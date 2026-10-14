use gpui::{prelude::*, *};
mod terminal;
use terminal::Terminal;

struct Spike {
    terminals: Vec<Entity<Terminal>>,
    selected: usize,
}

actions!(spike, [Quit]);

impl Render for Spike {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x17191d))
            .text_color(rgb(0xe4e4e4))
            .font_family("Menlo")
            .text_size(px(14.))
            .child(
                div()
                    .id("quit")
                    .p_2()
                    .child("Quit spike")
                    .cursor_pointer()
                    .on_click(|_, _, cx| cx.quit()),
            )
            .child(div().flex().gap_4().p_2().children((0..2).map(|index| {
                div()
                    .id(index)
                    .child(format!("Terminal {}", index + 1))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected = index;
                        this.terminals[index].focus_handle(cx).focus(window);
                        cx.notify();
                    }))
            })))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.terminals[self.selected].clone()),
            )
    }
}

fn main() {
    eprintln!(
        "Ghostty VT revision: {}",
        tasks_foundation::GHOSTTY_REVISION
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(
        !args.is_empty(),
        "Usage: terminal-spike DISPOSABLE_CWD [COMMAND ARGS...]"
    );
    let cwd = args[0].clone();
    let command = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        vec!["/bin/zsh".into(), "-f".into()]
    };
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(120.), px(120.)),
                    size(px(1000.), px(650.)),
                ))),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    let first = cx.new(|cx| Terminal::new(command, &cwd, true, cx));
                    let second = cx.new(|cx| {
                        Terminal::new(vec!["/bin/zsh".into(), "-f".into()], &cwd, false, cx)
                    });
                    Spike {
                        terminals: vec![first, second],
                        selected: 0,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
        if let Ok(seconds) = std::env::var("TASKS_SPIKE_QUIT_AFTER") {
            let seconds: u64 = seconds.parse().expect("quit-after seconds");
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(seconds))
                    .await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}
