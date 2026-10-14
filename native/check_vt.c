#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

typedef void Host;
typedef void (*WriteFn)(void *, const uint8_t *, size_t);
typedef void (*CellFn)(void *, uint16_t, uint16_t, const uint32_t *, uint32_t, uint32_t, uint32_t, bool, bool, bool);
Host *tasks_vt_new(uint16_t, uint16_t, WriteFn, void *);
void tasks_vt_free(Host *);
void tasks_vt_write(Host *, const uint8_t *, size_t);
int tasks_vt_resize(Host *, uint16_t, uint16_t);
void tasks_vt_scroll(Host *, int64_t);
int tasks_vt_frame(Host *, CellFn, void *);
int tasks_vt_key(Host *, const char *, const char *, size_t, unsigned);
int tasks_vt_paste(Host *, const uint8_t *, size_t);
int tasks_vt_select(Host *, uint16_t, uint16_t, uint16_t, uint16_t);
int tasks_vt_copy(Host *, WriteFn, void *);
bool tasks_vt_mouse_tracking(Host *);
int tasks_vt_mouse(Host *, unsigned, unsigned, float, float, unsigned, uint16_t, uint16_t);

typedef struct { uint8_t bytes[8192]; size_t len; bool wide, combining, green, selected; unsigned cells; } Capture;
static void output(void *userdata, const uint8_t *bytes, size_t len) {
    Capture *capture = userdata;
    assert(capture->len + len <= sizeof(capture->bytes));
    memcpy(capture->bytes + capture->len, bytes, len);
    capture->len += len;
}
static void cell(void *userdata, uint16_t x, uint16_t y, const uint32_t *points, uint32_t len,
                 uint32_t fg, uint32_t bg, bool bold, bool cursor, bool selected) {
    (void)x; (void)y; (void)bg; (void)bold; (void)cursor;
    Capture *capture = userdata;
    capture->cells++;
    for (uint32_t i = 0; i < len; i++) {
        capture->wide |= points[i] == 0x754c;
        capture->combining |= points[i] == 0x301 && len >= 2;
    }
    capture->green |= fg == 0x00ff00;
    capture->selected |= selected;
}
static void feed(Host *host, const char *text) { tasks_vt_write(host, (const uint8_t *)text, strlen(text)); }

static void mouse_protocols(Host *host, Capture *capture) {
    capture->len = 0;
    assert(!tasks_vt_mouse_tracking(host));
    assert(tasks_vt_mouse(host, 0, 1, 9, 18, 0, 80, 24) == 0 && capture->len == 0);
    feed(host, "\033[?1000h\033[?1006h");
    assert(tasks_vt_mouse_tracking(host));
    assert(tasks_vt_mouse(host, 0, 1, 9, 18, 2, 80, 24) == 0);
    assert(capture->len == strlen("\033[<16;2;2M") && !memcmp(capture->bytes, "\033[<16;2;2M", capture->len));
    capture->len = 0;
    assert(tasks_vt_mouse(host, 1, 1, 9, 18, 0, 80, 24) == 0);
    assert(capture->len == strlen("\033[<0;2;2m") && !memcmp(capture->bytes, "\033[<0;2;2m", capture->len));
    capture->len = 0;
    assert(tasks_vt_mouse(host, 2, 0, 18, 36, 0, 80, 24) == 0 && capture->len == 0);
    feed(host, "\033[?1002h");
    assert(tasks_vt_mouse(host, 2, 1, 18, 36, 0, 80, 24) == 0);
    assert(capture->len == strlen("\033[<32;3;3M") && !memcmp(capture->bytes, "\033[<32;3;3M", capture->len));
    capture->len = 0;
    assert(tasks_vt_mouse(host, 0, 4, 9, 18, 0, 80, 24) == 0);
    assert(capture->len == strlen("\033[<64;2;2M") && !memcmp(capture->bytes, "\033[<64;2;2M", capture->len));
    capture->len = 0;
    feed(host, "\033[?1006l\033[?1002l\033[?9h");
    assert(tasks_vt_mouse(host, 0, 1, 9, 18, 0, 80, 24) == 0);
    const char x10[] = {27, '[', 'M', 32, 34, 34};
    assert(capture->len == sizeof(x10) && !memcmp(capture->bytes, x10, sizeof(x10)));
    capture->len = 0;
    assert(tasks_vt_mouse(host, 1, 1, 9, 18, 0, 80, 24) == 0 && capture->len == 0);
    feed(host, "\033[?9l\033[?1003h\033[?1006h");
    assert(tasks_vt_mouse(host, 2, 0, 27, 54, 0, 80, 24) == 0);
    assert(capture->len == strlen("\033[<35;4;4M") && !memcmp(capture->bytes, "\033[<35;4;4M", capture->len));
    capture->len = 0;
    assert(tasks_vt_mouse(host, 2, 0, 28, 55, 0, 80, 24) == 0 && capture->len == 0);
    feed(host, "\033[?1006l\033[?1015h");
    assert(tasks_vt_mouse(host, 0, 2, 9, 18, 0, 80, 24) == 0);
    assert(capture->len == strlen("\033[34;2;2M") && !memcmp(capture->bytes, "\033[34;2;2M", capture->len));
    capture->len = 0;
    feed(host, "\033[?1015l\033[?1005h");
    assert(tasks_vt_mouse(host, 0, 1, 9, 18, 0, 80, 24) == 0);
    assert(capture->len == sizeof(x10) && !memcmp(capture->bytes, x10, sizeof(x10)));
    capture->len = 0;
    feed(host, "\033[?1005l\033[?1006h\033[?1016h");
    assert(tasks_vt_mouse(host, 0, 1, 9, 18, 0, 80, 24) == 0);
    // Pinned Ghostty emits surface pixels directly for mode 1016.
    assert(capture->len == strlen("\033[<0;9;18M") && !memcmp(capture->bytes, "\033[<0;9;18M", capture->len));
    capture->len = 0;
    feed(host, "\033[?1016l\033[?1006l\033[?1003l\033[?1000l");
    assert(!tasks_vt_mouse_tracking(host));
}

int main(void) {
    Capture capture = {0}, second = {0};
    Host *a = tasks_vt_new(80, 24, output, &capture);
    Host *b = tasks_vt_new(80, 24, output, &second);
    assert(a && b);
    mouse_protocols(a, &capture);
    feed(a, "Hello \033[38;2;0;255;0m界e\xcc\x81\033[0m");
    feed(b, "retained second terminal");
    assert(tasks_vt_frame(a, cell, &capture) == 0);
    assert(capture.wide && capture.combining && capture.green && capture.cells == 80 * 24);
    assert(tasks_vt_select(a, 0, 0, 4, 0) == 0);
    assert(tasks_vt_copy(a, output, &capture) == 0);
    assert(capture.len == 5 && !memcmp(capture.bytes, "Hello", 5));
    assert(tasks_vt_frame(a, cell, &capture) == 0 && capture.selected);
    capture.len = 0;
    assert(tasks_vt_key(a, "c", "c", 1, 2) == 0);
    assert(capture.len == 1 && capture.bytes[0] == 3);
    capture.len = 0;
    // Match GPUI's macOS key_char payloads, not just the terminal key names.
    assert(tasks_vt_key(a, "enter", "\n", 1, 0) == 0);
    assert(capture.len == 1 && capture.bytes[0] == '\r');
    capture.len = 0;
    assert(tasks_vt_key(a, "tab", "\t", 1, 0) == 0);
    assert(capture.len == 1 && capture.bytes[0] == '\t');
    capture.len = 0;
    feed(a, "\033[?2004h");
    assert(tasks_vt_paste(a, (const uint8_t *)"paste\n界", strlen("paste\n界")) == 0);
    const char *paste = "\033[200~paste\n界\033[201~";
    assert(capture.len == strlen(paste) && !memcmp(capture.bytes, paste, capture.len));
    capture.len = 0;
    feed(a, "\033[>1u"); // Kitty disambiguation mode, using Ghostty's encoder.
    assert(tasks_vt_key(a, "c", "c", 1, 2) == 0);
    assert(capture.len > 1 && capture.bytes[0] == 27 && capture.bytes[capture.len - 1] == 'u');
    feed(a, "\033[?1049hALTERNATE\033[?1049l");
    Capture copy = {0};
    assert(tasks_vt_select(a, 0, 0, 4, 0) == 0);
    assert(tasks_vt_copy(a, output, &copy) == 0 && !memcmp(copy.bytes, "Hello", 5));
    assert(tasks_vt_resize(a, 100, 30) == 0);
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    for (unsigned i = 0; i < 10000; i++) feed(a, "large-output line 0123456789 界\r\n");
    clock_gettime(CLOCK_MONOTONIC, &end);
    tasks_vt_scroll(a, -20);
    Capture frame = {0};
    assert(tasks_vt_frame(a, cell, &frame) == 0 && frame.cells == 100 * 30);
    assert(tasks_vt_select(b, 0, 0, 7, 0) == 0);
    assert(tasks_vt_copy(b, output, &second) == 0);
    assert(second.len == 8 && !memcmp(second.bytes, "retained", 8));
    double ms = (end.tv_sec - start.tv_sec) * 1000. + (end.tv_nsec - start.tv_nsec) / 1000000.;
    printf("{\"mouse_protocols\":true,\"unicode_styles_selection\":true,\"enter_tab_encoding\":true,\"ctrl_c_kitty_paste\":true,\"alternate_resize_scrollback\":true,\"two_retained_vt_states\":true,\"output_lines\":10000,\"parse_ms\":%.3f}\n", ms);
    tasks_vt_free(a);
    tasks_vt_free(b);
    return 0;
}
