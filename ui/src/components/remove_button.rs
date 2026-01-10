use gpui::{Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb, rgba, svg};

pub fn remove_button(
    handler: impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    div()
        .w(px(24.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgba(0x00000000))
        .on_mouse_down(MouseButton::Left, handler)
        .child(
            svg()
                .path("delete-2.svg")
                .w(px(24.))
                .h(px(24.))
                .text_color(rgb(0x9ca3af)),
        )
}
