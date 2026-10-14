//! Minimal native text reproduction, independent of the terminal renderer.
use gpui::{prelude::*, *};
use std::time::Duration;

struct TextSmoke;
impl Render for TextSmoke {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x505050))
            .text_color(rgb(0xffffff))
            .text_size(px(32.))
            .font_family("Menlo")
            .child("GPUI native text: Hello, 界 é")
            .child(div().size(px(50.)).bg(rgb(0xff0000)))
    }
}
fn main() {
    Application::new().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(120.), px(120.)),
                    size(px(1000.), px(650.)),
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| TextSmoke),
        )
        .unwrap();
        cx.activate(true);
        cx.spawn(async |cx| {
            cx.background_executor().timer(Duration::from_secs(8)).await;
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
