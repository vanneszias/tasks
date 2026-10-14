#include <ghostty/vt.h>
#include <stdlib.h>
#include <string.h>

// Keep Ghostty's evolving structs behind a C boundary compiled against pinned headers.
typedef void (*CellFn)(void *, uint16_t, uint16_t, const uint32_t *, uint32_t,
                       uint32_t, uint32_t, bool, bool, bool);
typedef void (*WriteFn)(void *, const uint8_t *, size_t);
typedef struct {
    GhosttyTerminal terminal;
    GhosttyRenderState render;
    GhosttyRenderStateRowIterator rows;
    GhosttyRenderStateRowCells cells;
    GhosttyMouseEncoder mouse;
    uint16_t mouse_cols, mouse_rows;
    WriteFn write;
    void *userdata;
} Host;

static void write_pty(GhosttyTerminal terminal, void *userdata, const uint8_t *data, size_t len) {
    (void)terminal;
    Host *host = userdata;
    host->write(host->userdata, data, len);
}

void tasks_vt_free(Host *host) {
    if (!host) return;
    ghostty_render_state_row_cells_free(host->cells);
    ghostty_render_state_row_iterator_free(host->rows);
    ghostty_render_state_free(host->render);
    ghostty_mouse_encoder_free(host->mouse);
    ghostty_terminal_free(host->terminal);
    free(host);
}

Host *tasks_vt_new(uint16_t cols, uint16_t rows, WriteFn write, void *userdata) {
    Host *host = calloc(1, sizeof(Host));
    if (!host) return NULL;
    if (ghostty_terminal_new(NULL, &host->terminal, cols, rows) != GHOSTTY_SUCCESS ||
        ghostty_render_state_new(NULL, &host->render) != GHOSTTY_SUCCESS ||
        ghostty_render_state_row_iterator_new(NULL, &host->rows) != GHOSTTY_SUCCESS ||
        ghostty_render_state_row_cells_new(NULL, &host->cells) != GHOSTTY_SUCCESS ||
        ghostty_mouse_encoder_new(NULL, &host->mouse) != GHOSTTY_SUCCESS) {
        tasks_vt_free(host);
        return NULL;
    }
    host->write = write;
    host->userdata = userdata;
    bool track_cell = true;
    ghostty_mouse_encoder_setopt(host->mouse, GHOSTTY_MOUSE_ENCODER_OPT_TRACK_LAST_CELL, &track_cell);
    ghostty_terminal_set(host->terminal, GHOSTTY_TERMINAL_OPT_USERDATA, host);
    ghostty_terminal_set(host->terminal, GHOSTTY_TERMINAL_OPT_WRITE_PTY, (const void *)write_pty);
    return host;
}

void tasks_vt_write(Host *host, const uint8_t *bytes, size_t len) {
    ghostty_terminal_vt_write(host->terminal, bytes, len);
    ghostty_mouse_encoder_setopt_from_terminal(host->mouse, host->terminal);
}

int tasks_vt_resize(Host *host, uint16_t cols, uint16_t rows) {
    return ghostty_terminal_resize(host->terminal, cols, rows, 9, 18);
}

// Return whether the application owns mouse input, even when its tracking mode
// intentionally suppresses this particular event (e.g. X10 releases).
bool tasks_vt_mouse_tracking(Host *host) {
    bool tracking = false;
    ghostty_terminal_get(host->terminal, GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING, &tracking);
    return tracking;
}

int tasks_vt_mouse(Host *host, unsigned action, unsigned button, float x, float y,
                   unsigned mods, uint16_t cols, uint16_t rows) {
    GhosttyMouseEvent event = NULL;
    int result = ghostty_mouse_event_new(NULL, &event);
    if (result) return result;
    GhosttyMouseEncoderSize size = GHOSTTY_INIT_SIZED(GhosttyMouseEncoderSize);
    size.screen_width = cols * 9;
    size.screen_height = rows * 18;
    size.cell_width = 9;
    size.cell_height = 18;
    if (host->mouse_cols != cols || host->mouse_rows != rows) {
        ghostty_mouse_encoder_setopt(host->mouse, GHOSTTY_MOUSE_ENCODER_OPT_SIZE, &size);
        host->mouse_cols = cols;
        host->mouse_rows = rows;
    }
    bool pressed = button != 0;
    ghostty_mouse_encoder_setopt(host->mouse, GHOSTTY_MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED, &pressed);
    ghostty_mouse_event_set_action(event, (GhosttyMouseAction)action);
    if (button) ghostty_mouse_event_set_button(event, (GhosttyMouseButton)button);
    else ghostty_mouse_event_clear_button(event);
    GhosttyMods flags = 0;
    if (mods & 1) flags |= GHOSTTY_MODS_SHIFT;
    if (mods & 2) flags |= GHOSTTY_MODS_CTRL;
    if (mods & 4) flags |= GHOSTTY_MODS_ALT;
    if (mods & 8) flags |= GHOSTTY_MODS_SUPER;
    ghostty_mouse_event_set_mods(event, flags);
    ghostty_mouse_event_set_position(event, (GhosttyMousePosition){x, y});
    char buffer[128];
    size_t written = 0;
    result = ghostty_mouse_encoder_encode(host->mouse, event, buffer, sizeof(buffer), &written);
    if (!result && written) host->write(host->userdata, (uint8_t *)buffer, written);
    ghostty_mouse_event_free(event);
    return result;
}

void tasks_vt_scroll(Host *host, int64_t delta) {
    GhosttyTerminalScrollViewport scroll = {.tag = GHOSTTY_SCROLL_VIEWPORT_DELTA, .value = {.delta = delta}};
    ghostty_terminal_scroll_viewport(host->terminal, scroll);
}

int tasks_vt_key(Host *host, const char *name, const char *text, size_t len, unsigned mods) {
    GhosttyKey key = GHOSTTY_KEY_UNIDENTIFIED;
    if (strlen(name) == 1 && name[0] >= 'a' && name[0] <= 'z') key = GHOSTTY_KEY_A + name[0] - 'a';
    if (strlen(name) == 1 && name[0] >= '0' && name[0] <= '9') key = GHOSTTY_KEY_DIGIT_0 + name[0] - '0';
    struct { const char *name; GhosttyKey key; } keys[] = {
        {"enter", GHOSTTY_KEY_ENTER}, {"backspace", GHOSTTY_KEY_BACKSPACE},
        {"escape", GHOSTTY_KEY_ESCAPE}, {"tab", GHOSTTY_KEY_TAB}, {"space", GHOSTTY_KEY_SPACE},
        {"up", GHOSTTY_KEY_ARROW_UP}, {"down", GHOSTTY_KEY_ARROW_DOWN},
        {"left", GHOSTTY_KEY_ARROW_LEFT}, {"right", GHOSTTY_KEY_ARROW_RIGHT},
        {"home", GHOSTTY_KEY_HOME}, {"end", GHOSTTY_KEY_END}, {"delete", GHOSTTY_KEY_DELETE},
        {"pageup", GHOSTTY_KEY_PAGE_UP}, {"pagedown", GHOSTTY_KEY_PAGE_DOWN}
    };
    for (size_t i = 0; i < sizeof(keys)/sizeof(keys[0]); i++) if (!strcmp(name, keys[i].name)) key = keys[i].key;
    GhosttyKeyEncoder encoder = NULL;
    GhosttyKeyEvent event = NULL;
    int result = ghostty_key_encoder_new(NULL, &encoder);
    if (result) return result;
    result = ghostty_key_event_new(NULL, &event);
    if (result) { ghostty_key_encoder_free(encoder); return result; }
    ghostty_key_encoder_setopt_from_terminal(encoder, host->terminal);
    ghostty_key_event_set_action(event, GHOSTTY_KEY_ACTION_PRESS);
    ghostty_key_event_set_key(event, key);
    GhosttyMods flags = 0;
    if (mods & 1) flags |= GHOSTTY_MODS_SHIFT;
    if (mods & 2) flags |= GHOSTTY_MODS_CTRL;
    if (mods & 4) flags |= GHOSTTY_MODS_ALT;
    if (mods & 8) flags |= GHOSTTY_MODS_SUPER;
    ghostty_key_event_set_mods(event, flags);
    ghostty_key_event_set_utf8(event, text, len);
    if (strlen(name) == 1) ghostty_key_event_set_unshifted_codepoint(event, (unsigned char)name[0]);
    char buffer[256];
    size_t written = 0;
    result = ghostty_key_encoder_encode(encoder, event, buffer, sizeof(buffer), &written);
    if (!result) host->write(host->userdata, (uint8_t *)buffer, written);
    ghostty_key_event_free(event);
    ghostty_key_encoder_free(encoder);
    return result;
}

typedef struct { const uint8_t *data; size_t len; } PasteData;
static bool paste_read(void *userdata, GhosttyString mime, GhosttyWriter writer) {
    (void)mime;
    PasteData *data = userdata;
    return writer.write(writer.userdata, data->data, data->len);
}

int tasks_vt_paste(Host *host, const uint8_t *data, size_t len) {
    PasteData source = {data, len};
    GhosttyString mime = {.ptr = (const uint8_t *)"text/plain", .len = 10};
    GhosttyPaste paste = GHOSTTY_INIT_SIZED(GhosttyPaste);
    paste.location = GHOSTTY_CLIPBOARD_LOCATION_STANDARD;
    paste.source = GHOSTTY_PASTE_SOURCE_CLIPBOARD;
    paste.mimes = &mime;
    paste.mimes_len = 1;
    paste.reader = (GhosttyMimeReader){.read = paste_read, .userdata = &source};
    // The spike's explicit Cmd+V action is the user's paste request.
    paste.allow_unsafe = true;
    bool written = false;
    return ghostty_terminal_paste(host->terminal, &paste, &written);
}

int tasks_vt_select(Host *host, uint16_t x1, uint16_t y1, uint16_t x2, uint16_t y2) {
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    selection.start = (GhosttyGridRef)GHOSTTY_INIT_SIZED(GhosttyGridRef);
    selection.end = (GhosttyGridRef)GHOSTTY_INIT_SIZED(GhosttyGridRef);
    GhosttyPoint start = {.tag = GHOSTTY_POINT_TAG_VIEWPORT, .value = {.coordinate = {x1, y1}}};
    GhosttyPoint end = {.tag = GHOSTTY_POINT_TAG_VIEWPORT, .value = {.coordinate = {x2, y2}}};
    int result = ghostty_terminal_grid_ref(host->terminal, start, &selection.start);
    if (result) return result;
    result = ghostty_terminal_grid_ref(host->terminal, end, &selection.end);
    if (result) return result;
    return ghostty_terminal_set(host->terminal, GHOSTTY_TERMINAL_OPT_SELECTION, &selection);
}

typedef struct { WriteFn emit; void *userdata; } CopyWriter;
static bool copy_write(void *userdata, const uint8_t *data, size_t len) {
    CopyWriter *writer = userdata;
    writer->emit(writer->userdata, data, len);
    return true;
}

int tasks_vt_copy(Host *host, WriteFn emit, void *userdata) {
    GhosttySelection selection = GHOSTTY_INIT_SIZED(GhosttySelection);
    int result = ghostty_terminal_get(host->terminal, GHOSTTY_TERMINAL_DATA_SELECTION, &selection);
    if (result) return result;
    GhosttyFormatterTerminalOptions options = GHOSTTY_INIT_SIZED(GhosttyFormatterTerminalOptions);
    options.emit = GHOSTTY_FORMATTER_FORMAT_PLAIN;
    options.trim = true;
    options.selection = &selection;
    GhosttyFormatter formatter = NULL;
    result = ghostty_formatter_terminal_new(NULL, &formatter, host->terminal, options);
    if (result) return result;
    CopyWriter copy = {emit, userdata};
    result = ghostty_formatter_format(formatter, (GhosttyWriter){.write = copy_write, .userdata = &copy});
    ghostty_formatter_free(formatter);
    return result;
}

static uint32_t color(GhosttyStyleColor value, GhosttyRenderStateColors *palette, GhosttyColorRgb fallback) {
    GhosttyColorRgb rgb = fallback;
    if (value.tag == GHOSTTY_STYLE_COLOR_RGB) rgb = value.value.rgb;
    if (value.tag == GHOSTTY_STYLE_COLOR_PALETTE) rgb = palette->palette[value.value.palette];
    return ((uint32_t)rgb.r << 16) | ((uint32_t)rgb.g << 8) | rgb.b;
}

int tasks_vt_frame(Host *host, CellFn emit, void *userdata) {
    int result = ghostty_render_state_update(host->render, host->terminal);
    if (result != GHOSTTY_SUCCESS) return result;
    GhosttyRenderStateColors colors = GHOSTTY_INIT_SIZED(GhosttyRenderStateColors);
    GhosttyRenderStateCursor cursor = GHOSTTY_INIT_SIZED(GhosttyRenderStateCursor);
    ghostty_render_state_get(host->render, GHOSTTY_RENDER_STATE_DATA_COLORS, &colors);
    ghostty_render_state_get(host->render, GHOSTTY_RENDER_STATE_DATA_CURSOR, &cursor);
    ghostty_render_state_get(host->render, GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR, &host->rows);
    uint16_t y = 0;
    while (ghostty_render_state_row_iterator_next(host->rows)) {
        GhosttyRenderStateRowSelection selection = GHOSTTY_INIT_SIZED(GhosttyRenderStateRowSelection);
        bool selected = ghostty_render_state_row_get(host->rows, GHOSTTY_RENDER_STATE_ROW_DATA_SELECTION, &selection) == GHOSTTY_SUCCESS;
        ghostty_render_state_row_get(host->rows, GHOSTTY_RENDER_STATE_ROW_DATA_CELLS, &host->cells);
        uint16_t x = 0;
        while (ghostty_render_state_row_cells_next(host->cells)) {
            uint32_t len = 0;
            ghostty_render_state_row_cells_get(host->cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_LEN, &len);
            uint32_t *points = len ? malloc(len * sizeof(uint32_t)) : NULL;
            if (len && !points) return GHOSTTY_OUT_OF_MEMORY;
            if (len) ghostty_render_state_row_cells_get(host->cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_BUF, points);
            GhosttyStyle style = GHOSTTY_INIT_SIZED(GhosttyStyle);
            ghostty_render_state_row_cells_get(host->cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE, &style);
            uint32_t fg = color(style.fg_color, &colors, colors.foreground);
            uint32_t bg = color(style.bg_color, &colors, colors.background);
            if (style.inverse) { uint32_t tmp = fg; fg = bg; bg = tmp; }
            emit(userdata, x, y, points, len, fg, bg, style.bold,
                 cursor.visible && cursor.viewport_has_value && cursor.viewport_x == x && cursor.viewport_y == y,
                 selected && x >= selection.start_x && x <= selection.end_x);
            free(points);
            x++;
        }
        y++;
    }
    return ghostty_render_state_clean(host->render);
}
