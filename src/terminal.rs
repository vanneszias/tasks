use gpui::{prelude::*, *};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    ffi::{CString, c_void},
    io::{Read, Write},
    ops::Range,
    sync::mpsc,
    time::Duration,
};

type Emit =
    unsafe extern "C" fn(*mut c_void, u16, u16, *const u32, u32, u32, u32, bool, bool, bool);

fn uses_native_text_input(key: &Keystroke) -> bool {
    // macOS also supplies key_char for Enter and Tab; those need Ghostty key encoding.
    key.key_char
        .as_deref()
        .is_some_and(|text| !text.is_empty() && !text.chars().any(char::is_control))
        && !key.modifiers.control
        && !key.modifiers.platform
        && !key.modifiers.alt
}
unsafe extern "C" {
    fn tasks_vt_new(
        cols: u16,
        rows: u16,
        write: unsafe extern "C" fn(*mut c_void, *const u8, usize),
        data: *mut c_void,
    ) -> *mut c_void;
    fn tasks_vt_free(host: *mut c_void);
    fn tasks_vt_write(host: *mut c_void, bytes: *const u8, len: usize);
    fn tasks_vt_resize(host: *mut c_void, cols: u16, rows: u16) -> i32;
    fn tasks_vt_scroll(host: *mut c_void, delta: i64);
    fn tasks_vt_key(
        host: *mut c_void,
        name: *const i8,
        text: *const i8,
        len: usize,
        mods: u32,
    ) -> i32;
    fn tasks_vt_frame(host: *mut c_void, emit: Emit, data: *mut c_void) -> i32;
    fn tasks_vt_paste(host: *mut c_void, data: *const u8, len: usize) -> i32;
    fn tasks_vt_select(host: *mut c_void, x1: u16, y1: u16, x2: u16, y2: u16) -> i32;
    fn tasks_vt_copy(
        host: *mut c_void,
        emit: unsafe extern "C" fn(*mut c_void, *const u8, usize),
        data: *mut c_void,
    ) -> i32;
}

#[derive(Clone)]
struct Cell {
    x: u16,
    y: u16,
    text: String,
    fg: u32,
    bg: u32,
    bold: bool,
    cursor: bool,
    selected: bool,
}

unsafe extern "C" fn collect(
    data: *mut c_void,
    x: u16,
    y: u16,
    points: *const u32,
    len: u32,
    fg: u32,
    bg: u32,
    bold: bool,
    cursor: bool,
    selected: bool,
) {
    // The C bridge lends exactly len codepoints for the duration of this callback.
    let points = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(points, len as usize) }
    };
    let cells = unsafe { &mut *(data as *mut Vec<Cell>) };
    cells.push(Cell {
        x,
        y,
        text: points.iter().filter_map(|p| char::from_u32(*p)).collect(),
        fg,
        bg,
        bold,
        cursor,
        selected,
    });
}

unsafe extern "C" fn copy_bytes(data: *mut c_void, bytes: *const u8, len: usize) {
    let result = unsafe { &mut *(data as *mut Vec<u8>) };
    result.extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, len) });
}

struct Output {
    writer: Box<dyn Write + Send>,
    error: Option<String>,
}
unsafe extern "C" fn write_pty(data: *mut c_void, bytes: *const u8, len: usize) {
    let output = unsafe { &mut *(data as *mut Output) };
    let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
    if let Err(error) = output
        .writer
        .write_all(bytes)
        .and_then(|_| output.writer.flush())
    {
        output.error = Some(error.to_string());
    }
}

pub struct Terminal {
    vt: *mut c_void,
    output: Box<Output>,
    master: Box<dyn MasterPty + Send>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    incoming: mpsc::Receiver<Vec<u8>>,
    focus: FocusHandle,
    dimensions: (u16, u16),
    error: Option<String>,
    bounds: Option<Bounds<Pixels>>,
    selection_start: Option<(u16, u16)>,
    marked: String,
    _tick: Task<()>,
}

impl Terminal {
    pub fn new(
        command: Vec<String>,
        cwd: &str,
        fixture_input: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 720,
                pixel_height: 432,
            })
            .expect("open PTY");
        let mut builder = CommandBuilder::new(&command[0]);
        builder.args(&command[1..]);
        builder.cwd(cwd);
        builder.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(builder)
            .expect("launch fixture command");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("PTY reader");
        let (send, incoming) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            let mut buffer = [0; 16384];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if send.send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut output = Box::new(Output {
            writer: pair.master.take_writer().expect("PTY writer"),
            error: None,
        });
        let vt = unsafe { tasks_vt_new(80, 24, write_pty, (&mut *output as *mut Output).cast()) };
        assert!(!vt.is_null(), "create Ghostty VT");
        let input = fixture_input
            .then(|| std::env::var("TASKS_SPIKE_TEST_INPUT").ok())
            .flatten();
        let tick = cx.spawn(async move |this, cx| {
            let started = std::time::Instant::now();
            let mut input = input;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this
                    .update(cx, |terminal, cx| {
                        if started.elapsed() > Duration::from_secs(3) {
                            if let Some(input) = input.take() {
                                let _ = terminal.output.writer.write_all(input.as_bytes());
                                let _ = terminal.output.writer.flush();
                                eprintln!("explicit test fixture input sent");
                            }
                        }
                        let mut changed = false;
                        // Bound each UI update; backpressure prevents unbounded heavy-output memory.
                        for _ in 0..32 {
                            let Ok(bytes) = terminal.incoming.try_recv() else {
                                break;
                            };
                            unsafe {
                                tasks_vt_write(terminal.vt, bytes.as_ptr(), bytes.len());
                            }
                            changed = true;
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        cx.on_app_quit(|this, cx| {
            let child = this.child.take();
            cx.background_executor().spawn(async move {
                if let Some(mut child) = child {
                    let pid = child.process_id();
                    match child.kill().and_then(|_| child.wait()) {
                        Ok(status) => eprintln!("owned PTY child {pid:?} exited: {status:?}"),
                        Err(error) => eprintln!("owned PTY shutdown failed: {error}"),
                    }
                }
            })
        })
        .detach();
        Self {
            vt,
            output,
            master: pair.master,
            child: Some(child),
            incoming,
            focus: cx.focus_handle(),
            dimensions: (80, 24),
            error: None,
            bounds: None,
            selection_start: None,
            marked: String::new(),
            _tick: tick,
        }
    }

    fn resize(&mut self, bounds: Bounds<Pixels>) {
        self.bounds = Some(bounds);
        let dimensions = (
            ((f32::from(bounds.size.width) / 9.) as u16).max(2),
            ((f32::from(bounds.size.height) / 18.) as u16).max(2),
        );
        if dimensions != self.dimensions {
            let result = unsafe { tasks_vt_resize(self.vt, dimensions.0, dimensions.1) };
            let resize = self.master.resize(PtySize {
                cols: dimensions.0,
                rows: dimensions.1,
                pixel_width: dimensions.0.saturating_mul(9),
                pixel_height: dimensions.1.saturating_mul(18),
            });
            if result != 0 || resize.is_err() {
                self.error = Some("Terminal resize failed".into());
            }
            self.dimensions = dimensions;
        }
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.platform && key.key == "q" {
            return;
        }
        if !self.marked.is_empty() {
            return;
        }
        if uses_native_text_input(key) {
            return; // GPUI's native input handler delivers committed text/IME below.
        }
        if key.modifiers.platform && key.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                let result = unsafe { tasks_vt_paste(self.vt, text.as_ptr(), text.len()) };
                if result != 0 {
                    self.error = Some(format!("Ghostty paste failed: {result}"));
                }
            }
        } else if key.modifiers.platform && key.key == "c" {
            let mut bytes: Vec<u8> = Vec::new();
            let result =
                unsafe { tasks_vt_copy(self.vt, copy_bytes, (&mut bytes as *mut Vec<u8>).cast()) };
            if result == 0 {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
            }
        } else {
            let name = CString::new(key.key.as_str()).unwrap();
            let text = key.key_char.as_deref().unwrap_or("");
            let mods = u32::from(key.modifiers.shift)
                | (u32::from(key.modifiers.control) << 1)
                | (u32::from(key.modifiers.alt) << 2)
                | (u32::from(key.modifiers.platform) << 3);
            let result = unsafe {
                tasks_vt_key(
                    self.vt,
                    name.as_ptr(),
                    text.as_ptr().cast(),
                    text.len(),
                    mods,
                )
            };
            if result != 0 {
                self.error = Some(format!("Ghostty key encoding failed: {result}"));
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn coordinate(&self, position: Point<Pixels>) -> (u16, u16) {
        let origin = self.bounds.map(|b| b.origin).unwrap_or_default();
        let x = ((f32::from(position.x - origin.x) / 9.).max(0.) as u16).min(self.dimensions.0 - 1);
        let y =
            ((f32::from(position.y - origin.y) / 18.).max(0.) as u16).min(self.dimensions.1 - 1);
        (x, y)
    }
}

impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let units: Vec<u16> = self.marked.encode_utf16().collect();
        let range = range.start.min(units.len())..range.end.min(units.len());
        *actual = Some(range.clone());
        Some(String::from_utf16_lossy(&units[range]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.marked.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..self.marked.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked.clear();
        let result = unsafe {
            tasks_vt_key(
                self.vt,
                c"unidentified".as_ptr(),
                text.as_ptr().cast(),
                text.len(),
                0,
            )
        };
        if result != 0 {
            self.error = Some(format!("Ghostty committed text failed: {result}"));
        }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = text.into();
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.bounds
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Focusable for Terminal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let paint_entity = entity.clone();
        div()
            .id("terminal")
            .size_full()
            .track_focus(&self.focus)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.selection_start = Some(this.coordinate(event.position));
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if let Some((x1, y1)) = this.selection_start {
                    let (x2, y2) = this.coordinate(event.position);
                    unsafe {
                        tasks_vt_select(this.vt, x1, y1, x2, y2);
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selection_start = None),
            )
            .on_key_down(cx.listener(Self::key))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(18.));
                unsafe {
                    tasks_vt_scroll(this.vt, -(f32::from(delta.y) / 18.).round() as i64);
                }
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            this.resize(bounds);
                            let mut cells: Vec<Cell> = Vec::new();
                            let result = unsafe {
                                tasks_vt_frame(
                                    this.vt,
                                    collect,
                                    (&mut cells as *mut Vec<Cell>).cast(),
                                )
                            };
                            if result != 0 {
                                this.error =
                                    Some(format!("Ghostty render snapshot failed: {result}"));
                            }
                            cells
                        })
                    },
                    move |bounds, cells, window, cx| {
                        let focus = paint_entity.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, paint_entity.clone()),
                            cx,
                        );
                        for cell in &cells {
                            let origin = bounds.origin
                                + point(px(f32::from(cell.x) * 9.), px(f32::from(cell.y) * 18.));
                            window.paint_quad(fill(
                                Bounds::new(origin, size(px(9.), px(18.))),
                                rgb(if cell.selected { 0x375574 } else { cell.bg }),
                            ));
                            if cell.cursor {
                                window.paint_quad(fill(
                                    Bounds::new(origin, size(px(2.), px(18.))),
                                    rgb(0xffffff),
                                ));
                            }
                        }
                        for cell in cells {
                            let origin = bounds.origin
                                + point(px(f32::from(cell.x) * 9.), px(f32::from(cell.y) * 18.));
                            if !cell.text.is_empty() {
                                let mut font = font("Menlo");
                                if cell.bold {
                                    font.weight = FontWeight::BOLD;
                                }
                                let text: SharedString = cell.text.into();
                                let run = TextRun {
                                    len: text.len(),
                                    font,
                                    color: rgb(cell.fg).into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                let line =
                                    window.text_system().shape_line(text, px(14.), &[run], None);
                                if let Err(error) = line.paint(origin, px(18.), window, cx) {
                                    eprintln!("GPUI text paint failed: {error}");
                                }
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // PTY closure is not proof of OpenCode interruption. Milestone-0 reports that separately.
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        unsafe {
            tasks_vt_free(self.vt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::uses_native_text_input;
    use gpui::Keystroke;

    #[::core::prelude::v1::test]
    fn macos_enter_and_tab_are_encoded_keys_not_committed_text() {
        for (name, text, native_text) in [
            ("enter", "\n", false),
            ("tab", "\t", false),
            ("space", " ", true),
            ("é", "é", true),
        ] {
            let mut key = Keystroke::parse(name).unwrap();
            key.key_char = Some(text.into());
            assert_eq!(uses_native_text_input(&key), native_text, "{name}");
        }
    }
}
