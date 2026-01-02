use std::path::Path;

use gpui::{Bounds, Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb, svg};

use crate::chart::view::{ChartView, overlays::symbol_search::symbol_search_overlay};

pub fn header_controls(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    interval_trigger: Div,
) -> (Div, Option<Div>) {
    let toggle_symbol_search =
        cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
            let was_open = this.symbol_search_open && !this.symbol_search_add_to_watchlist;
            this.hover_index = None;
            this.hover_position = None;
            this.symbol_search_add_to_watchlist = false;
            this.symbol_search_open = !was_open;
            this.interval_select_open = false;
            window.refresh();
        });

    let search_label = if view.candles.is_empty() {
        "Search symbols".to_string()
    } else {
        Path::new(&view.source)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| view.source.as_str())
            .to_string()
    };

    let search_icon = svg()
        .path("search.svg")
        .w(px(16.))
        .h(px(16.))
        .text_color(rgb(0x9ca3af));

    let search_input = div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .w(px(120.))
        .rounded_md()
        .border_1()
        .border_color(rgb(0x1f2937))
        .bg(rgb(0x111827))
        .text_sm()
        .text_color(rgb(0x9ca3af))
        .on_mouse_down(MouseButton::Left, toggle_symbol_search)
        .child(search_icon)
        .child(div().text_color(gpui::white()).child(search_label));

    let track_header_controls = cx.processor(
        |this: &mut ChartView, bounds: Vec<Bounds<gpui::Pixels>>, _, _| {
            if let Some(trigger_bounds) = bounds.get(1) {
                this.interval_trigger_origin = (
                    f32::from(trigger_bounds.origin.x),
                    f32::from(trigger_bounds.origin.y),
                );
                this.interval_trigger_height = f32::from(trigger_bounds.size.height);
            }
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
