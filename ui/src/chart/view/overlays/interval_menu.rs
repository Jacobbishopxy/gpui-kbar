use core::Interval;
use gpui::{Div, MouseButton, SharedString, div, prelude::*, px, rgb};

use crate::chart::view::{ChartView, OVERLAY_GAP};

pub fn interval_menu(
    view: &mut ChartView,
    cx: &mut gpui::Context<ChartView>,
    options: &[(Option<Interval>, &str)],
) -> Option<Div> {
    let (menu_left, menu_top, menu_width) = if let Some(bounds) = view.interval_trigger_bounds {
        (
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y + bounds.size.height) + OVERLAY_GAP,
            f32::from(bounds.size.width),
        )
    } else {
        (0.0, 148.0, 128.0)
    };

    let mut menu = div()
        .absolute()
        .left(px(menu_left))
        .top(px(menu_top))
        .flex()
        .flex_col()
        .bg(rgb(0x0f172a))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md();

    for (option, label) in options.iter().cloned() {
        let is_active = view.interval == option;
        let handler = cx.listener(
            move |this: &mut ChartView, _: &gpui::MouseDownEvent, window, _| {
                this.apply_interval(option);
                window.refresh();
            },
        );
        let bg = if is_active {
            rgb(0x1f2937)
        } else {
            rgb(0x0f172a)
        };
        let text = SharedString::from(label.to_string());

        menu = menu.child(
            div()
                .px_3()
                .py_2()
                .w(px(menu_width))
                .bg(bg)
                .text_sm()
                .text_color(gpui::white())
                .on_mouse_down(MouseButton::Left, handler)
                .child(text),
        );
    }

    Some(menu)
}
