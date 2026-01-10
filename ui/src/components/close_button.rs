use gpui::{Div, MouseButton, MouseDownEvent, Stateful, div, prelude::*, px, rgb, svg};

use crate::components::button_effect;

pub fn close_button(
    id: impl Into<gpui::ElementId>,
    handler: impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    let mut button = div()
        .w(px(24.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    button = button
        .rounded_full()
        .bg(rgb(0x1f2937))
        .text_color(gpui::white());
    let button = button.id(id);
    let button = button_effect::apply(button, 0x1f2937);

    let icon_color = rgb(0xffffff);

    button.child(
        svg()
            .path("cross-circle.svg")
            .w(px(24.))
            .h(px(24.))
            .text_color(icon_color),
    )
}
