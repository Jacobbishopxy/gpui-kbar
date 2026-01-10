use gpui::{Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};

use crate::chart::view::ChartView;
use crate::components::button_effect;
use crate::components::loading_sand::loading_sand;
use crate::components::remove_button::remove_button;

pub fn watchlist_panel(view: &mut ChartView, cx: &mut Context<ChartView>) -> Div {
    let open_watchlist_search =
        cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.hover_index = None;
            this.hover_position = None;
            let should_close = this.symbol_search_open && this.symbol_search_add_to_watchlist;
            this.symbol_search_add_to_watchlist = true;
            this.symbol_search_open = !should_close;
            this.interval_select_open = false;
            if this.symbol_search_open {
                this.focus_handle.focus(window);
            }
            window.refresh();
        });
    let watchlist_list = watchlist_list(view, cx);

    let mut watchlist_panel = div()
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .max_h(px(420.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().text_color(gpui::white()).child("Watchlist"))
                .child(button_effect::apply(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(0x111827))
                        .text_xs()
                        .text_color(rgb(0x9ca3af))
                        .on_mouse_down(MouseButton::Left, open_watchlist_search)
                        .child("+ Add")
                        .id("watchlist-add"),
                    0x111827,
                )),
        )
        .child(watchlist_list);
    if let Some(err) = view.load_error.clone() {
        watchlist_panel =
            watchlist_panel.child(div().text_xs().text_color(rgb(0xef4444)).child(err));
    }
    watchlist_panel
}

fn watchlist_list(view: &mut ChartView, cx: &mut Context<ChartView>) -> gpui::Stateful<Div> {
    let mut watchlist_list = div()
        .flex()
        .flex_col()
        .gap_2()
        .min_w(px(0.))
        .max_h(px(320.))
        .pr_1()
        .id("watchlist-list")
        .overflow_y_scroll();
    let symbols = view.watchlist_symbols();
    if symbols.is_empty() {
        return watchlist_list.child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x0f172a))
                .border_1()
                .border_color(rgb(0x1f2937))
                .text_sm()
                .text_color(rgb(0x9ca3af))
                .child("Watchlist is empty. Add a symbol to get started."),
        );
    }

    for symbol in symbols.into_iter() {
        let is_loading = view.loading_symbol.as_deref() == Some(&symbol);
        let active = view.source == symbol;
        let bg_hex = if active || is_loading {
            0x111827
        } else {
            0x0f172a
        };
        let hover_bg_hex = if active || is_loading {
            0x1f2937
        } else {
            0x111827
        };
        let active_bg_hex = if active || is_loading {
            0x0f172a
        } else {
            0x0b1220
        };
        let symbol_label = if is_loading {
            format!("{symbol} - loading")
        } else {
            symbol.clone()
        };
        let meta = view.symbol_meta(&symbol);
        let label = meta
            .as_ref()
            .map(|m| m.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| symbol.clone());
        let exchange = meta
            .as_ref()
            .map(|m| m.exchange.clone())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "Symbol".to_string());
        let symbol_for_load = symbol.clone();
        let symbol_for_remove = symbol.clone();
        let row_id: gpui::SharedString = format!("watchlist-row-{symbol}").into();
        let remove_id: gpui::SharedString = format!("watchlist-remove-{symbol}").into();
        let handler = cx.listener(
            move |this: &mut ChartView, _: &MouseDownEvent, window, cx| {
                this.start_symbol_load(symbol_for_load.clone(), true, window, cx);
            },
        );
        let remove_handler = cx.listener(
            move |this: &mut ChartView, _: &MouseDownEvent, window, cx| {
                this.remove_from_watchlist(&symbol_for_remove);
                cx.stop_propagation();
                window.refresh();
            },
        );
        let mut left = div().flex().items_center().gap_2().min_w(px(0.));
        if is_loading {
            left = left.child(loading_sand(18.0, rgb(0xf59e0b)));
        }
        left = left
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::white())
                    .truncate()
                    .child(symbol_label),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x1f2937))
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child(exchange),
            );
        let remove_button = button_effect::apply_custom(
            remove_button(remove_handler).id(remove_id),
            0x1f2937,
            0x0f172a,
        )
        .debug_selector(|| format!("watchlist-remove-{symbol}"));
        let right = div()
            .flex()
            .items_center()
            .gap_2()
            .min_w(px(0.))
            .flex_1()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_sm()
                    .text_color(gpui::white())
                    .truncate()
                    .child(label),
            )
            .child(remove_button);
        watchlist_list = watchlist_list.child(button_effect::apply_custom(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(bg_hex))
                .border_1()
                .border_color(rgb(0x1f2937))
                .flex()
                .items_center()
                .gap_2()
                .min_w(px(0.))
                .on_mouse_down(MouseButton::Left, handler)
                .child(left)
                .child(right)
                .id(row_id)
                .debug_selector(|| format!("watchlist-row-{symbol}")),
            hover_bg_hex,
            active_bg_hex,
        ));
    }

    watchlist_list
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::Candle;
    use gpui::{Modifiers, TestAppContext};

    use crate::ChartMeta;

    #[gpui::test]
    async fn watchlist_remove_buttons_remove_the_clicked_symbol(cx: &TestAppContext) {
        let mut cx = cx.clone();
        let (chart, cx) = cx.add_window_view(|_, cx| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: "AAPL".to_string(),
                    initial_interval: None,
                },
                None,
                cx,
            )
        });

        let set_watchlist = |cx: &mut gpui::VisualTestContext, symbols: &[&str]| {
            chart.update(cx, |chart, cx| {
                chart.watchlist = symbols.iter().map(|s| (*s).to_string()).collect();
                cx.notify();
            });
            cx.refresh().expect("refresh");
            cx.run_until_parked();
        };

        let read_watchlist =
            |cx: &mut gpui::VisualTestContext| chart.update(cx, |chart, _| chart.watchlist.clone());

        set_watchlist(cx, &["AAPL", "MSFT", "TSLA"]);
        let remove_aapl_bounds = cx
            .debug_bounds("watchlist-remove-AAPL")
            .expect("watchlist-remove-AAPL bounds");
        cx.simulate_click(remove_aapl_bounds.center(), Modifiers::none());
        let watchlist = read_watchlist(cx);
        assert_eq!(watchlist, vec!["MSFT".to_string(), "TSLA".to_string()]);

        set_watchlist(cx, &["AAPL", "MSFT", "TSLA"]);
        let remove_msft_bounds = cx
            .debug_bounds("watchlist-remove-MSFT")
            .expect("watchlist-remove-MSFT bounds");
        cx.simulate_click(remove_msft_bounds.center(), Modifiers::none());
        let watchlist = read_watchlist(cx);
        assert_eq!(watchlist, vec!["AAPL".to_string(), "TSLA".to_string()]);
    }

    #[gpui::test]
    async fn watchlist_row_click_loads_the_clicked_symbol(cx: &TestAppContext) {
        let mut cx = cx.clone();
        let (chart, cx) = cx.add_window_view(|_, cx| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: "AAPL".to_string(),
                    initial_interval: None,
                },
                None,
                cx,
            )
        });

        chart.update(cx, |chart, cx| {
            chart.watchlist = vec!["AAPL".to_string(), "MSFT".to_string(), "TSLA".to_string()];
            cx.notify();
        });
        cx.refresh().expect("refresh");
        cx.run_until_parked();

        let initial_source = chart.update(cx, |chart, _| chart.current_source());
        assert_eq!(initial_source, "AAPL");

        let msft_row_bounds = cx
            .debug_bounds("watchlist-row-MSFT")
            .expect("watchlist-row-MSFT bounds");
        cx.simulate_click(msft_row_bounds.center(), Modifiers::none());
        cx.run_until_parked();

        let source = chart.update(cx, |chart, _| chart.current_source());
        assert_eq!(source, "MSFT");
        let load_error = chart.update(cx, |chart, _| chart.load_error.clone());
        assert_eq!(load_error, None);
    }

    #[gpui::test]
    async fn watchlist_buttons_work_after_adding_new_items(cx: &TestAppContext) {
        let mut cx = cx.clone();
        let (chart, cx) = cx.add_window_view(|_, cx| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: "AAPL".to_string(),
                    initial_interval: None,
                },
                None,
                cx,
            )
        });

        chart.update(cx, |chart, cx| {
            chart.watchlist = vec!["AAPL".to_string(), "MSFT".to_string()];
            cx.notify();
        });
        cx.refresh().expect("refresh");
        cx.run_until_parked();

        chart.update(cx, |chart, cx| {
            chart.watchlist.push("TSLA".to_string());
            cx.notify();
        });
        cx.refresh().expect("refresh");
        cx.run_until_parked();

        let remove_aapl_bounds = cx
            .debug_bounds("watchlist-remove-AAPL")
            .expect("watchlist-remove-AAPL bounds");
        cx.simulate_click(remove_aapl_bounds.center(), Modifiers::none());

        let watchlist = chart.update(cx, |chart, _| chart.watchlist.clone());
        assert_eq!(watchlist, vec!["MSFT".to_string(), "TSLA".to_string()]);
    }
}
