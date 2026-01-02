use gpui::{Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb, rgba};

use crate::chart::view::ChartView;
use crate::components::close_button::close_button;

const POPUP_WIDTH: f32 = 620.0;
const POPUP_HEIGHT: f32 = 620.0;

pub fn symbol_search_overlay(view: &mut ChartView, cx: &mut Context<ChartView>) -> Option<Div> {
    if !view.symbol_search_open {
        return None;
    }

    view.ensure_symbol_universe();

    let search_filters = [
        "All", "Stocks", "Funds", "Futures", "Forex", "Crypto", "Indices", "Bonds", "Economy",
        "Options",
    ];
    let active_filter = view.symbol_search_filter().to_string();
    let add_on_select = view.symbol_search_add_to_watchlist;

    let mut filters = div().flex().items_center().gap_2();
    for label in search_filters.iter() {
        let active = *label == active_filter;
        let bg = if active { rgb(0x1f2937) } else { rgb(0x111827) };
        let text = if active { rgb(0xffffff) } else { rgb(0x9ca3af) };
        let filter_label = label.to_string();
        let set_filter = cx.listener(move |this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.set_symbol_search_filter(&filter_label);
            window.refresh();
        });
        filters = filters.child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(bg)
                .text_xs()
                .text_color(text)
                .on_mouse_down(MouseButton::Left, set_filter)
                .child(*label),
        );
    }

    let filtered: Vec<&_> = view
        .symbol_universe()
        .iter()
        .filter(|entry| {
            active_filter == "All"
                || entry
                    .filters
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&active_filter))
        })
        .collect();

    let mut results_list = div()
        .flex()
        .flex_col()
        .flex_1()
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .h_full()
        .id("search-results")
        .overflow_y_scroll();
    if filtered.is_empty() {
        results_list = results_list.child(
            div()
                .p_4()
                .text_sm()
                .text_color(rgb(0x9ca3af))
                .child("No symbols match this filter."),
        );
    }
    for (idx, entry) in filtered.into_iter().enumerate() {
        let active = idx == 0;
        let row_bg = if active { rgb(0x0f172a) } else { rgb(0x0b1220) };
        let border_color = if active { rgb(0x2563eb) } else { rgb(0x1f2937) };
        let symbol = entry.symbol.clone();
        let on_select = cx.listener(
            move |this: &mut ChartView, _: &MouseDownEvent, window, cx| {
                this.start_symbol_load(symbol.clone(), add_on_select, window, cx);
            },
        );

        let mut row = div()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .bg(row_bg)
            .on_mouse_down(MouseButton::Left, on_select)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(32.))
                            .h(px(32.))
                            .rounded_full()
                            .bg(rgb(0x1f2937))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(gpui::white())
                            .child(entry.badge.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child(entry.symbol.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(0x1f2937))
                                            .text_xs()
                                            .text_color(rgb(0x9ca3af))
                                            .child(entry.market.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x9ca3af))
                                    .child(entry.name.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child(entry.market.clone())
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x1f2937))
                            .text_xs()
                            .text_color(gpui::white())
                            .child(entry.venue.clone()),
                    ),
            );

        row = if active {
            row.border_1().border_color(border_color)
        } else {
            row.border_b_1().border_color(border_color)
        };
        results_list = results_list.child(row);
    }

    let close_overlay = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
        this.symbol_search_open = false;
        window.refresh();
    });

    Some(
        div()
            .absolute()
            .left(px(0.))
            .top(px(0.))
            .w_full()
            .h_full()
            .bg(rgba(0x0b122080))
            .flex()
            .items_center()
            .justify_center()
            .p_3()
            .child(
                div()
                    .w(px(POPUP_WIDTH))
                    .h(px(POPUP_HEIGHT))
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .rounded_md()
                    .shadow_lg()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .h_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child("Symbol Search"),
                                    )
                                    .child(close_button(close_overlay)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0x1f2937))
                                    .bg(rgb(0x111827))
                                    .child(
                                        div().text_sm().text_color(rgb(0x9ca3af)).child("Search"),
                                    )
                                    .child(div().text_sm().text_color(gpui::white()).child("NDQ")),
                            )
                            .child(filters)
                            .child(results_list)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x6b7280))
                                    .child("Search using ISIN and CUSIP codes"),
                            ),
                    ),
            ),
    )
}
