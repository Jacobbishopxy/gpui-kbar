use gpui::{Bounds, Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};

use crate::chart::view::{ChartView, overlays::symbol_search::symbol_search_overlay};

pub fn header_controls(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    interval_trigger: Div,
) -> (Div, Option<Div>) {
    let toggle_symbol_search =
        cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.symbol_search_open = !this.symbol_search_open;
            this.interval_select_open = false;
            window.refresh();
        });

    let search_input = div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .w(px(220.))
        .rounded_md()
        .border_1()
        .border_color(rgb(0x1f2937))
        .bg(rgb(0x111827))
        .text_sm()
        .text_color(rgb(0x9ca3af))
        .on_mouse_down(MouseButton::Left, toggle_symbol_search)
        .child(div().text_color(gpui::white()).child("Search symbols"));

    let track_header_controls = cx.processor(
        |this: &mut ChartView, bounds: Vec<Bounds<gpui::Pixels>>, _, _| {
            this.interval_trigger_bounds = bounds.get(1).copied();
        },
    );

    let header_controls = div()
        .relative()
        .flex()
        .items_center()
        .gap_2()
        .child(search_input)
        .child(interval_trigger)
        .on_children_prepainted(track_header_controls);

    let search_overlay = symbol_search_overlay(view, cx);
    (header_controls, search_overlay)
}
