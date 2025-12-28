use core::Interval;
use gpui::{Div, MouseButton, SharedString, div, prelude::*, px, rgb};

use crate::chart::view::{ChartView, OVERLAY_GAP};

pub fn interval_menu(
    view: &mut ChartView,
    cx: &mut gpui::Context<ChartView>,
    options: &[(Option<Interval>, &str)],
    origin: (f32, f32),
    trigger_height: f32,
    trigger_width: f32,
) -> Option<Div> {
    let menu_width = if trigger_width > 0.0 {
        trigger_width
    } else {
        super::super::INTERVAL_TRIGGER_WIDTH
    };
    let menu_top = origin.1 + trigger_height.max(0.0) + OVERLAY_GAP;
    let menu_left = origin.0;

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
