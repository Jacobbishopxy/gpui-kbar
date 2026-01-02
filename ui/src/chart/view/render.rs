use std::path::Path;

use core::Interval;
use gpui::{
    Context, MouseButton, MouseDownEvent, Render, SharedString, Window, div, prelude::*, px, rgb,
    svg,
};
use time::macros::format_description;

use super::super::{
    canvas::{chart_canvas, volume_canvas},
    footer::{chart_footer, range_button},
    header::chart_header,
};
use super::context::format_price_range;
use super::sections::body::chart_body;
use super::sections::header::header_controls;
use super::sections::sidebar::sidebar;
use super::state::QUICK_RANGE_WINDOWS;
use super::widgets::{header_chip, stat_row, toolbar_button};
use super::{ChartView, INTERVAL_TRIGGER_WIDTH, TOOLBAR_WIDTH, padded_bounds};

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
impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let interval_label = ChartView::interval_label(self.current_interval());
        let playback_label = SharedString::from(if self.replay_enabled() {
            "Replay"
        } else {
            "Live"
        });
        let timezone_label = SharedString::from(
            self.candles
                .last()
                .map(|c| {
                    let offset = c.timestamp.offset();
                    format!("UTC{offset}")
                })
                .unwrap_or_else(|| "UTC".to_string()),
        );
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
        let range_text = SharedString::from(format_price_range(price_min, price_max));
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

        let chart = chart_canvas(candles, price_min, price_max, hover_local, hover_y)
            .flex_1()
            .w_full()
            .h_full();
        let volume = volume_canvas(volume_candles, hover_local)
            .flex_1()
            .w_full()
            .h_full();

        let start_label = start_label.unwrap_or_else(|| "---".into());
        let mid_label = mid_label.unwrap_or_else(|| "---".into());
        let end_label = end_label.unwrap_or_else(|| "---".into());

        let chart_area = chart_body(
            self,
            _cx,
            price_labels,
            chart,
            volume,
            start_label,
            mid_label,
            end_label,
            candle_count,
        );

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
            .w(px(INTERVAL_TRIGGER_WIDTH))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x1f2937))
            .bg(rgb(0x111827))
            .text_sm()
            .text_color(gpui::white())
            .on_mouse_down(MouseButton::Left, toggle_interval_select)
            .child(select_label.clone());

        let (header_controls, search_overlay) = header_controls(self, _cx, interval_trigger);

        let toggle_replay = _cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
            let next = !this.replay_enabled();
            this.set_replay_mode(next);
            window.refresh();
        });
        let replay_chip = {
            let active = self.replay_enabled();
            let bg = if active { rgb(0x1f2937) } else { rgb(0x111827) };
            let border = if active { rgb(0x2563eb) } else { rgb(0x1f2937) };
            let text = if active { rgb(0xffffff) } else { rgb(0xe5e7eb) };
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(bg)
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(text)
                .on_mouse_down(MouseButton::Left, toggle_replay)
                .child("Replay")
        };

        let header_left = div()
            .flex()
            .items_center()
            .gap_3()
            .child(header_controls)
            .child(header_chip("Indicators"))
            .child(header_chip("Compare"))
            .child(header_chip("Alerts"))
            .child(replay_chip);

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
        let mut quick_ranges = div().flex().items_center().gap_2();
        for (idx, (label, _)) in QUICK_RANGE_WINDOWS.iter().enumerate() {
            let is_active = self.current_range_index() == idx;
            let handle = _cx.listener(move |this: &mut Self, _: &MouseDownEvent, window, _| {
                this.apply_range_index(idx, true);
                window.refresh();
            });
            quick_ranges = quick_ranges
                .child(range_button(*label, is_active).on_mouse_down(MouseButton::Left, handle));
        }

        let footer = chart_footer(
            quick_ranges,
            interval_label.clone(),
            candle_count,
            range_text.clone(),
            !self.replay_enabled(),
            playback_label.clone(),
            timezone_label.clone(),
        );

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

        let mut watchlist_list = div()
            .flex()
            .flex_col()
            .gap_2()
            .min_w(px(0.))
            .max_h(px(320.))
            .pr_1();
        watchlist_list.style().overflow.y = Some(gpui::Overflow::Scroll);
        let symbols = self.watchlist_symbols();
        if symbols.is_empty() {
            watchlist_list = watchlist_list.child(
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
        } else {
            for symbol in symbols.into_iter() {
                let is_loading = self.loading_symbol.as_deref() == Some(&symbol);
                let active = self.source == symbol;
                let bg = if active || is_loading {
                    rgb(0x111827)
                } else {
                    rgb(0x0f172a)
                };
                let symbol_label = if is_loading {
                    format!("{symbol} - loading")
                } else {
                    symbol.clone()
                };
                let meta = self.symbol_meta(&symbol);
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
                let handler =
                    _cx.listener(move |this: &mut Self, _: &MouseDownEvent, window, cx| {
                        this.start_symbol_load(symbol_for_load.clone(), window, cx);
                        this.add_to_watchlist(symbol_for_load.clone());
                    });
                let remove_handler =
                    _cx.listener(move |this: &mut Self, _: &MouseDownEvent, window, cx| {
                        this.remove_from_watchlist(&symbol_for_remove);
                        cx.stop_propagation();
                        window.refresh();
                    });
                let left = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.))
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
                let remove_button = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, remove_handler)
                    .child(
                        svg()
                            .path("cross-circle.svg")
                            .w(px(24.))
                            .h(px(24.))
                            .text_color(rgb(0x9ca3af)),
                    );
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
                        .gap_2()
                        .min_w(px(0.))
                        .on_mouse_down(MouseButton::Left, handler)
                        .child(left)
                        .child(right),
                );
            }
        }

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
        if let Some(err) = self.load_error.clone() {
            watchlist_panel =
                watchlist_panel.child(div().text_xs().text_color(rgb(0xef4444)).child(err));
        }

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

        let sidebar = sidebar(watchlist_panel, instrument_card, trading_stub);

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

        let root = div()
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

        let track_root = _cx.processor(
            |this: &mut Self, bounds: Vec<gpui::Bounds<gpui::Pixels>>, _, _| {
                if let Some(root_bounds) = bounds.first() {
                    this.root_origin = (
                        f32::from(root_bounds.origin.x),
                        f32::from(root_bounds.origin.y),
                    );
                }
            },
        );

        let mut layered = div()
            .relative()
            .w_full()
            .h_full()
            .on_children_prepainted(track_root)
            .child(root);

        if let Some(overlay) = search_overlay {
            layered = layered.child(overlay);
        }

        if let Some(menu) = if self.interval_select_open {
            let origin = (
                self.interval_trigger_origin.0 - self.root_origin.0,
                self.interval_trigger_origin.1 - self.root_origin.1,
            );
            super::overlays::interval_menu::interval_menu(
                self,
                _cx,
                INTERVAL_OPTIONS,
                origin,
                self.interval_trigger_height,
                INTERVAL_TRIGGER_WIDTH,
            )
        } else {
            None
        } {
            layered = layered.child(menu);
        }

        if let Some(tip) = tooltip {
            layered = layered.child(tip);
        }

        layered
    }
}
