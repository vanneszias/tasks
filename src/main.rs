use gpui::{prelude::*, *};
mod terminal;
use terminal::Terminal;

unsafe extern "C" {
    fn tasks_install_quit_guard() -> bool;
    fn tasks_quit_requested() -> bool;
    fn tasks_allow_quit();
}

struct Spike {
    terminals: Vec<Entity<Terminal>>,
    selected: usize,
    stopping: bool,
    canceled: bool,
    guard_installed: bool,
    shutdown_message: String,
    shutdown: Option<Task<()>>,
    quit_ready: bool,
}

actions!(spike, [Quit]);

impl Spike {
    fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.stopping {
            return;
        }
        if !self.guard_installed {
            self.shutdown_message = "Quit canceled: macOS Quit guard could not be installed. Restart the spike after checking its native bridge.".into();
            cx.notify();
            return;
        }
        self.stopping = true;
        self.canceled = false;
        self.shutdown_message =
            "Stopping both owned terminals before Quit… Input is disabled.".into();
        let children: Vec<_> = self
            .terminals
            .iter()
            .map(|terminal| {
                terminal.update(cx, |terminal, _| {
                    (terminal.begin_shutdown(), terminal.agent)
                })
            })
            .collect();
        cx.notify();
        self.shutdown = Some(cx.spawn(async move |this, cx| {
            let results = cx.background_executor().spawn(async move {
                children.into_iter().map(|(mut child, agent)| {
                    let result = child.as_mut().map(|child| child.stop()).transpose();
                    (child, agent, result)
                }).collect::<Vec<_>>()
            }).await;
            let _ = this.update(cx, |this, cx| {
                let mut failures = Vec::new();
                for (index, (child, agent, result)) in results.into_iter().enumerate() {
                    match result {
                        Err(error) => failures.push(format!("Terminal {}: {error}", index + 1)),
                        Ok(Some(report)) if agent && report.root_forced => failures.push(format!(
                            "Terminal {}: agent root did not exit cleanly; session persistence is unverified. Inspect the exact saved session/history before recovery. Retry cannot certify a forced exit.", index + 1)),
                        _ => {},
                    }
                    this.terminals[index].update(cx, |terminal, _| terminal.restore_after_shutdown(child));
                }
                this.stopping = false;
                if !failures.is_empty() {
                    this.shutdown_message = format!("Quit canceled. {} Input remains disabled; Retry Quit repeats bounded cleanup.", failures.join(" "));
                    eprintln!("{}", this.shutdown_message);
                } else if this.canceled {
                    this.shutdown_message = "Quit canceled by user. Owned execution is stopped; input remains disabled. Quit again to exit.".into();
                } else {
                    eprintln!("coordinated shutdown complete before cx.quit()");
                    this.quit_ready = true;
                    unsafe { tasks_allow_quit(); }
                    cx.quit();
                }
                cx.notify();
            });
        }));
    }
}

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
                    .child(if self.stopping {
                        "Cancel Quit"
                    } else {
                        "Quit spike / Retry Quit"
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.stopping {
                            this.canceled = true;
                            this.shutdown_message =
                                "Quit canceled; bounded cleanup is still stopping owned execution…"
                                    .into();
                            cx.notify();
                        } else {
                            this.request_quit(cx);
                        }
                    })),
            )
            .child(div().p_2().child(self.shutdown_message.clone()))
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
        let guard_installed = unsafe { tasks_install_quit_guard() };
        let mut spike = None;
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(120.), px(120.)),
                    size(px(1000.), px(650.)),
                ))),
                ..Default::default()
            },
            |window, cx| {
                let entity = cx.new(|cx| {
                    let first = cx.new(|cx| Terminal::new(command, &cwd, true, cx));
                    let second = cx.new(|cx| {
                        Terminal::new(vec!["/bin/zsh".into(), "-f".into()], &cwd, false, cx)
                    });
                    Spike {
                        terminals: vec![first, second],
                        selected: 0,
                        stopping: false,
                        canceled: false,
                        guard_installed,
                        shutdown_message: if guard_installed {
                            String::new()
                        } else {
                            "Native Quit guard installation failed".into()
                        },
                        shutdown: None,
                        quit_ready: false,
                    }
                });
                let weak = entity.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    weak.update(cx, |this, cx| {
                        if this.quit_ready {
                            return true;
                        }
                        this.request_quit(cx);
                        false // Retain the window and its visible failure result.
                    })
                    .unwrap_or(false)
                });
                spike = Some(entity.clone());
                entity
            },
        )
        .unwrap();
        let spike = spike.unwrap();
        let weak = spike.downgrade();
        cx.on_action(move |_: &Quit, cx| {
            let _ = weak.update(cx, |this, cx| this.request_quit(cx));
        });
        let poll = spike.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                if unsafe { tasks_quit_requested() } {
                    if cx
                        .update(|cx| poll.update(cx, |this, cx| this.request_quit(cx)))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        })
        .detach();
        cx.activate(true);
        if let Ok(seconds) = std::env::var("TASKS_SPIKE_QUIT_AFTER") {
            let seconds: u64 = seconds.parse().expect("quit-after seconds");
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(seconds))
                    .await;
                let _ = cx.update(|cx| spike.update(cx, |this, cx| this.request_quit(cx)));
            })
            .detach();
        }
    });
}
