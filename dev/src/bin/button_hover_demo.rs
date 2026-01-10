use gpui::{
    App, Bounds, Context as GpuiContext, MouseButton, Render, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};
use ui::application_with_assets;

const WINDOW_WIDTH: f32 = 420.0;
const WINDOW_HEIGHT: f32 = 220.0;

struct ButtonHoverDemo {
    hovered: bool,
    clicks: usize,
}

impl ButtonHoverDemo {
    fn new() -> Self {
        Self {
            hovered: false,
            clicks: 0,
        }
    }
}

impl Render for ButtonHoverDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let on_hover = cx.listener(|this: &mut ButtonHoverDemo, hovered: &bool, _, cx| {
            this.hovered = *hovered;
            cx.notify();
        });

        let on_click = cx.listener(|this: &mut ButtonHoverDemo, _, _, cx| {
            this.clicks += 1;
            cx.notify();
        });

        let button_bg = rgb(0x1f2937);
        let button_hover_bg = rgb(0x374151);
        let button_active_bg = rgb(0x111827);

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child("Hover the button to slightly highlight it."),
            )
            .child(
                div()
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .bg(button_bg)
                    .text_sm()
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .id("button-hover-demo")
                    .hover(move |s| s.bg(button_hover_bg))
                    .active(move |s| s.bg(button_active_bg))
                    .on_hover(on_hover)
                    .on_mouse_down(MouseButton::Left, on_click)
                    .child("Hover / Click me"),
            )
            .child(div().text_sm().text_color(gpui::white()).child(format!(
                "hovered: {}, clicks: {}",
                if self.hovered { "true" } else { "false" },
                self.clicks
            )))
    }
}

fn main() {
    application_with_assets().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| ButtonHoverDemo::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
