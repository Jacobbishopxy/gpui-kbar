use std::path::Path;

use core::Interval;
use gpui::{
    Bounds, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Render, ScrollWheelEvent, SharedString, Window, div, prelude::*, px, rgb, rgba,
};
use time::macros::format_description;

use super::super::{
    canvas::{chart_canvas, volume_canvas},
    footer::chart_footer,
    header::chart_header,
};
use super::{ChartView, padded_bounds};

const INTERVAL_OPTIONS: &[(Option<Interval>, &str)] = &[
    (None, "raw"),
    (Some(Interval::Second(3)), "3s"),
    (Some(Interval::Second(10)), "10s"),
    (Some(Interval::Second(30)), "30s"),
    (Some(Interval::Minute(1)), "1m"),
    (Some(Interval::Minute(5)), "5m"),
    (Some(Interval::Minute(10)), "10m"),
    (Some(Interval::Minute(15)), "15m"),
    (Some(Interval::Minute(30)), "30m"),
    (Some(Interval::Hour(1)), "1h"),
    (Some(Interval::Day(1)), "1d"),
];
const SIDEBAR_WIDTH: f32 = 320.0;
const TOOLBAR_WIDTH: f32 = 56.0;
const OVERLAY_GAP: f32 = 8.0;

fn toolbar_button(label: impl Into<SharedString>, active: bool) -> Div {
    let label = label.into();
    let bg = if active { rgb(0x111827) } else { rgb(0x0f172a) };
    div()
        .w(px(36.))
        .h(px(36.))
        .rounded_md()
        .bg(bg)
        .border_1()
        .border_color(rgb(0x1f2937))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .text_color(rgb(0xe5e7eb))
        .child(label)
}

fn header_chip(label: impl Into<SharedString>) -> Div {
    let label = label.into();
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0x111827))
        .border_1()
        .border_color(rgb(0x1f2937))
        .text_sm()
        .text_color(rgb(0xe5e7eb))
        .child(label)
}

fn stat_row(label: impl Into<SharedString>, value: impl Into<String>) -> Div {
    let label = label.into();
    div()
        .flex()
        .items_center()
        .justify_between()
        .text_xs()
        .text_color(rgb(0x9ca3af))
        .child(label)
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xe5e7eb))
                .child(value.into()),
        )
}

impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let interval_label = ChartView::interval_label(self.interval);
        let select_label = interval_label.clone();
        let (start, end) = self.visible_range();
        let visible = if start < end {
            &self.candles[start..end]
        } else {
            &self.candles[..]
        };
        let candle_count = visible.len();
        let (price_min, price_max) = padded_bounds(visible);
        self.price_min = price_min;
        self.price_max = price_max;
        let range_text = SharedString::from(format!("{:.4} - {:.4}", price_min, price_max));
        let tooltip = self.tooltip_overlay(start, end);
        let price_labels = [
            format!("{price_max:.4}"),
            format!("{:.4}", (price_min + price_max) * 0.5),
            format!("{price_min:.4}"),
        ];

        let time_fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
        let start_label = visible.first().map(|c| {
            c.timestamp
                .format(&time_fmt)
                .unwrap_or_else(|_| c.timestamp.to_string())
        });
        let mid_label = visible.get(candle_count.saturating_sub(1) / 2).map(|c| {
            c.timestamp
                .format(&time_fmt)
                .unwrap_or_else(|_| c.timestamp.to_string())
        });
        let end_label = visible.last().map(|c| {
            c.timestamp
                .format(&time_fmt)
                .unwrap_or_else(|_| c.timestamp.to_string())
        });

        let candles = visible.to_vec();
        let volume_candles = candles.clone();
        let price_min = self.price_min;
        let price_max = self.price_max;
        let hover_local = self.hover_index.and_then(|idx| {
            if start <= idx && idx < end {
                Some(idx - start)
            } else {
                None
            }
        });
        let hover_y = if hover_local.is_some() {
            self.hover_position.map(|(_, y)| y)
        } else {
            None
        };
        let last_close = self.candles.last().map(|c| c.close);
        let prev_close = self.candles.iter().rev().nth(1).map(|c| c.close);
        let (change_display, change_color) = match (last_close, prev_close) {
            (Some(latest), Some(prev)) if prev.abs() > f64::EPSILON => {
                let diff = latest - prev;
                let pct = diff / prev * 100.0;
                let sign = if diff >= 0.0 { "+" } else { "-" };
                (
                    format!("{sign}{:.2} ({sign}{:.2}%)", diff.abs(), pct.abs()),
                    if diff >= 0.0 {
                        rgb(0x22c55e)
                    } else {
                        rgb(0xef4444)
                    },
                )
            }
            _ => ("--".to_string(), rgb(0x9ca3af)),
        };
        let symbol_label = Path::new(&self.source)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.source.as_str())
            .to_string();
        let price_display = last_close
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "--".to_string());

        let track_chart_bounds =
            _cx.processor(|this: &mut Self, bounds: Vec<Bounds<Pixels>>, _, _| {
                if let Some(canvas_bounds) = bounds.first() {
                    this.chart_bounds = Some(*canvas_bounds);
                }
            });

        let handle_scroll = _cx.listener(|this: &mut Self, event: &ScrollWheelEvent, window, _| {
            this.handle_scroll(event, window);
        });

        let handle_mouse_down =
            _cx.listener(|this: &mut Self, event: &MouseDownEvent, window, _| {
                if event.button == MouseButton::Left {
                    this.dragging = true;
                    this.last_drag_position =
                        Some((f32::from(event.position.x), f32::from(event.position.y)));
                    window.refresh();
                }
            });

        let handle_mouse_up = _cx.listener(|this: &mut Self, _: &MouseUpEvent, window, _| {
            this.dragging = false;
            this.last_drag_position = None;
            window.refresh();
        });

        let handle_mouse_move =
            _cx.listener(move |this: &mut Self, event: &MouseMoveEvent, window, _| {
                this.handle_hover(event, candle_count);
                this.handle_drag(event, window);
            });

        let chart = chart_canvas(candles, price_min, price_max, hover_local, hover_y)
            .flex_1()
            .w_full()
            .h_full();

        let canvas_region = div()
            .flex_1()
            .w_full()
            .h_full()
            .relative()
            .on_children_prepainted(track_chart_bounds)
            .child(chart);

        let price_axis = div()
            .w(px(82.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .items_end()
            .px_2()
            .bg(rgb(0x0f172a))
            .border_r_1()
            .border_color(rgb(0x1f2937))
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .relative()
            .child(price_labels[0].clone())
            .child(price_labels[1].clone())
            .child(price_labels[2].clone());

        let hover_price_label =
            if let (Some((_, y)), Some(bounds)) = (self.hover_position, self.chart_bounds) {
                let height = f32::from(bounds.size.height);
                if height <= 0.0 {
                    None
                } else {
                    let oy = f32::from(bounds.origin.y);
                    let frac = ((y - oy) / height).clamp(0.0, 1.0);
                    let price = self.price_max - (self.price_max - self.price_min) * frac as f64;
                    let label_h = 18.0;
                    let mut top = frac * height - label_h * 0.5;
                    top = top.clamp(0.0, height - label_h);

                    Some(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(top))
                            .w(px(82.))
                            .h(px(label_h))
                            .px_1()
                            .bg(rgba(0x1f293780))
                            .border_1()
                            .border_color(rgba(0x37415180))
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .justify_end()
                            .text_xs()
                            .text_color(gpui::white())
                            .child(format!("{price:.4}")),
                    )
                }
            } else {
                None
            };

        let price_axis = if let Some(label) = hover_price_label {
            price_axis.child(label)
        } else {
            price_axis
        };

        let chart_row = div()
            .flex_1()
            .flex()
            .w_full()
            .h_full()
            .min_h(px(320.))
            .on_mouse_down(MouseButton::Left, handle_mouse_down)
            .on_mouse_move(handle_mouse_move)
            .on_mouse_up(MouseButton::Left, handle_mouse_up)
            .on_scroll_wheel(handle_scroll)
            .child(price_axis)
            .child(canvas_region);

        let time_axis = div()
            .h(px(28.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .bg(rgb(0x0f172a))
            .border_t_1()
            .border_color(rgb(0x1f2937))
            .child(start_label.unwrap_or_else(|| "---".into()))
            .child(mid_label.unwrap_or_else(|| "---".into()))
            .child(end_label.unwrap_or_else(|| "---".into()));

        let toggle_interval_select =
            _cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
                this.interval_select_open = !this.interval_select_open;
                this.symbol_search_open = false;
                window.refresh();
            });

        let interval_trigger = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .w(px(128.))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x1f2937))
            .bg(rgb(0x111827))
            .text_sm()
            .text_color(gpui::white())
            .on_mouse_down(MouseButton::Left, toggle_interval_select)
            .child(select_label.clone());

        let toggle_symbol_search =
            _cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
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

        let search_filters = [
            "All", "Stocks", "Funds", "Futures", "Forex", "Crypto", "Indices", "Bonds", "Economy",
            "Options",
        ];
        let search_results = [
            ("100", "NDQ", "US 100 Index", "index cfd", "TVC"),
            ("ETF", "NDQ", "BetaShares NASDAQ 100 ETF", "fund etf", "ASX"),
            (
                "ETF",
                "NDQ",
                "Invesco QQQ Trust Series I",
                "fund etf",
                "TRADEGATE",
            ),
            (
                "ETF",
                "NDQ",
                "Invesco QQQ Trust Series I",
                "fund etf",
                "BER",
            ),
            (
                "ETF",
                "NDQ",
                "Invesco QQQ Trust Series I",
                "fund etf",
                "HAM",
            ),
            (
                "100",
                "NDQM",
                "NASDAQ 100 Index (NDX)",
                "index cfd",
                "FXOpen",
            ),
            ("CASH", "NDQ100", "Nasdaq Cash", "index cfd", "Eightcap"),
            (
                "CW",
                "NDQCC",
                "Cititwarrants 36.2423 NDQ 07-Jun-35 Instal Mini",
                "warrant",
                "CHIXAU",
            ),
            ("CR", "NDQUSD", "Nasdaq666", "spot crypto", "CRYPTO"),
            (
                "3L",
                "NDQ3L",
                "SG Issuer SA Exchange Traded Product 2022-03-18",
                "fund etf",
                "Euronext Paris",
            ),
            (
                "3S",
                "NDQ3S",
                "SG Issuer SA War 2022- Without fixed mat on ...",
                "fund etf",
                "Euronext Paris",
            ),
            (
                "USD",
                "NDQUSD",
                "US Tech (NDQ) / US Dollar",
                "index cfd",
                "easyMarkets",
            ),
        ];
        let search_overlay = if self.symbol_search_open {
            let mut filters = div().flex().items_center().gap_2();
            for (idx, label) in search_filters.iter().enumerate() {
                let active = idx == 0;
                let bg = if active { rgb(0x1f2937) } else { rgb(0x111827) };
                let text = if active { rgb(0xffffff) } else { rgb(0x9ca3af) };
                filters = filters.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(bg)
                        .text_xs()
                        .text_color(text)
                        .child(*label),
                );
            }

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
            for (idx, (badge, symbol, name, market, venue)) in search_results.iter().enumerate() {
                let active = idx == 0;
                let row_bg = if active { rgb(0x0f172a) } else { rgb(0x0b1220) };
                let border_color = if active { rgb(0x2563eb) } else { rgb(0x1f2937) };
                let close_row = _cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
                    this.symbol_search_open = false;
                    window.refresh();
                });

                let mut row = div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(row_bg)
                    .on_mouse_down(MouseButton::Left, close_row)
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
                                    .child(*badge),
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
                                                    .child(*symbol),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_sm()
                                                    .bg(rgb(0x1f2937))
                                                    .text_xs()
                                                    .text_color(rgb(0x9ca3af))
                                                    .child(*market),
                                            ),
                                    )
                                    .child(div().text_xs().text_color(rgb(0x9ca3af)).child(*name)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(rgb(0x9ca3af))
                            .child(*market)
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x1f2937))
                                    .text_xs()
                                    .text_color(gpui::white())
                                    .child(*venue),
                            ),
                    );

                row = if active {
                    row.border_1().border_color(border_color)
                } else {
                    row.border_b_1().border_color(border_color)
                };
                results_list = results_list.child(row);
            }

            let close_overlay = _cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
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
                            .w(px(920.))
                            .h(px(630.))
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
                                            .child(
                                                div()
                                                    .w(px(24.))
                                                    .h(px(24.))
                                                    .rounded_full()
                                                    .bg(rgb(0x1f2937))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_xs()
                                                    .text_color(gpui::white())
                                                    .on_mouse_down(MouseButton::Left, close_overlay)
                                                    .child("X"),
                                            ),
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
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(0x9ca3af))
                                                    .child("Search"),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(gpui::white())
                                                    .child("NDQ"),
                                            ),
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
        } else {
            None
        };

        let track_header_controls =
            _cx.processor(|this: &mut Self, bounds: Vec<Bounds<Pixels>>, _, _| {
                this.interval_trigger_bounds = bounds.get(1).copied();
            });

        let header_controls = div()
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .child(search_input)
            .child(interval_trigger)
            .on_children_prepainted(track_header_controls);

        let interval_menu = if self.interval_select_open {
            let (menu_left, menu_top, menu_width) =
                if let Some(bounds) = self.interval_trigger_bounds {
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

            for (option, label) in INTERVAL_OPTIONS {
                let is_active = self.interval == *option;
                let handler =
                    _cx.listener(move |this: &mut Self, _: &MouseDownEvent, window, _| {
                        this.apply_interval(*option);
                        window.refresh();
                    });
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
        } else {
            None
        };

        let header_left = div()
            .flex()
            .items_center()
            .gap_3()
            .child(header_controls)
            .child(header_chip("Indicators"))
            .child(header_chip("Compare"))
            .child(header_chip("Alerts"))
            .child(header_chip("Replay"));

        let header_right = div()
            .flex()
            .items_center()
            .gap_2()
            .child(header_chip("Log"))
            .child(header_chip("Auto"))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2563eb))
                    .text_sm()
                    .text_color(gpui::white())
                    .child("Publish"),
            )
            .child(
                div()
                    .w(px(32.))
                    .h(px(32.))
                    .rounded_full()
                    .bg(rgb(0x1f2937))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(0xe5e7eb))
                    .child("U"),
            );

        let header = chart_header(header_left, header_right);
        let footer = chart_footer(div(), interval_label, candle_count, range_text.clone());

        let chart_area = div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(420.))
            .bg(rgb(0x0b1220))
            .border_1()
            .border_color(rgb(0x1f2937))
            .rounded_md()
            .overflow_hidden()
            .child(chart_row)
            .child(
                div()
                    .flex()
                    .w_full()
                    .h(px(120.))
                    .min_h(px(100.))
                    .child(
                        div()
                            .w(px(82.))
                            .h_full()
                            .bg(rgb(0x0f172a))
                            .border_r_1()
                            .border_color(rgb(0x1f2937)),
                    )
                    .child(
                        volume_canvas(volume_candles, hover_local)
                            .flex_1()
                            .w_full()
                            .h_full(),
                    ),
            )
            .child(time_axis);

        let toolbar_items = [
            "Cursor", "Trend", "Fib", "Brush", "Text", "Measure", "Zoom", "Cross",
        ];
        let mut left_toolbar = div()
            .w(px(TOOLBAR_WIDTH))
            .bg(rgb(0x0f172a))
            .border_r_1()
            .border_color(rgb(0x1f2937))
            .py_3()
            .flex()
            .flex_col()
            .items_center()
            .gap_2();
        for (idx, item) in toolbar_items.iter().enumerate() {
            left_toolbar = left_toolbar.child(toolbar_button(*item, idx == 0));
        }

        let watchlist_items = [
            ("AAPL", "273.81", "+0.53%"),
            ("TSLA", "485.40", "-0.03%"),
            ("NFLX", "93.64", "+0.15%"),
            ("USOIL", "58.38", "-0.02%"),
        ];
        let mut watchlist_list = div().flex().flex_col().gap_2();
        for (idx, (sym, price, change)) in watchlist_items.iter().enumerate() {
            let active = idx == 0;
            let bg = if active { rgb(0x111827) } else { rgb(0x0f172a) };
            let change_color = if change.starts_with('-') {
                rgb(0xef4444)
            } else {
                rgb(0x22c55e)
            };
            watchlist_list = watchlist_list.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(bg)
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().text_color(gpui::white()).child(*sym))
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x1f2937))
                                    .text_xs()
                                    .text_color(rgb(0x9ca3af))
                                    .child("Stock"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().child(*price))
                            .child(div().text_xs().text_color(change_color).child(*change)),
                    ),
            );
        }

        let watchlist_panel = div()
            .bg(rgb(0x0b1220))
            .border_1()
            .border_color(rgb(0x1f2937))
            .rounded_md()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().text_color(gpui::white()).child("Watchlist"))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x111827))
                            .text_xs()
                            .text_color(rgb(0x9ca3af))
                            .child("+ Add"),
                    ),
            )
            .child(watchlist_list);

        let instrument_card = div()
            .bg(rgb(0x0b1220))
            .border_1()
            .border_color(rgb(0x1f2937))
            .rounded_md()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child("Instrument"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_2xl()
                            .text_color(gpui::white())
                            .child(price_display),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(change_color)
                            .child(change_display),
                    ),
            )
            .child(stat_row("Symbol", symbol_label.clone()))
            .child(stat_row("Interval", select_label.to_string()))
            .child(stat_row("Candles", candle_count.to_string()))
            .child(stat_row("Range", range_text.to_string()));

        let trading_stub = div()
            .bg(rgb(0x0b1220))
            .border_1()
            .border_color(rgb(0x1f2937))
            .rounded_md()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::white())
                    .child("Trading panel"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child("Order ticket and positions will appear here."),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2563eb))
                    .text_sm()
                    .text_color(gpui::white())
                    .child("Open panel"),
            );

        let sidebar = div()
            .w(px(SIDEBAR_WIDTH))
            .bg(rgb(0x0b1220))
            .border_l_1()
            .border_color(rgb(0x1f2937))
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(watchlist_panel)
            .child(instrument_card)
            .child(trading_stub);

        let main_column = div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_3()
            .p_3()
            .child(chart_area);

        let body = div()
            .flex()
            .flex_1()
            .w_full()
            .min_h(px(560.))
            .child(left_toolbar)
            .child(main_column)
            .child(sidebar);

        let mut root = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .relative()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(header)
            .child(body)
            .child(footer);

        if let Some(overlay) = search_overlay {
            root = root.child(overlay);
        }

        if let Some(menu) = interval_menu {
            root = root.child(menu);
        }

        if let Some(tip) = tooltip {
            root = root.child(tip);
        }

        root
    }
}
