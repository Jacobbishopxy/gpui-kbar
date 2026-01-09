use std::{path::Path, sync::Arc};

use super::super::{
    aggregation::AggregatedCandle,
    canvas::{chart_canvas, volume_canvas},
    footer::{chart_footer, range_button},
    header::chart_header,
};
use super::context::format_price_range;
use super::sections::body::chart_body;
use super::sections::header::header_controls;
use super::sections::layout::{
    build_body_layout, build_interval_menu, build_layered_view, build_loading_overlay,
    build_sidebar_panels,
};
use super::state::QUICK_RANGE_WINDOWS;
use super::widgets::{header_chip, header_icon};
use super::{ChartView, INTERVAL_TRIGGER_WIDTH, padded_bounds};
use crate::components::loading_sand::loading_sand;
use core::{Candle, Interval};
use gpui::{
    Context, Div, MouseButton, MouseDownEvent, Render, SharedString, Window, div, prelude::*, px,
    rgb,
};

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

// While the blocking loading overlay is visible, avoid expensive per-frame chart rendering
// so the spinner animation can stay smooth.
const SKIP_CHART_RENDER_WHILE_LOADING: bool = true;

pub(crate) struct RenderState {
    pub(crate) interval_label: SharedString,
    pub(crate) playback_label: SharedString,
    pub(crate) timezone_label: SharedString,
    pub(crate) candles: Arc<[Candle]>,
    pub(crate) visible_start: usize,
    pub(crate) visible_end: usize,
    pub(crate) aggregated: Option<Arc<[AggregatedCandle]>>,
    pub(crate) volume_max: Option<f64>,
    pub(crate) candle_count: usize,
    pub(crate) price_labels: [String; 3],
    pub(crate) start_label: String,
    pub(crate) mid_label: String,
    pub(crate) end_label: String,
    pub(crate) price_min: f64,
    pub(crate) price_max: f64,
    pub(crate) range_text: SharedString,
    pub(crate) hover_local: Option<usize>,
    pub(crate) hover_x: Option<f32>,
    pub(crate) hover_y: Option<f32>,
    pub(crate) change_display: String,
    pub(crate) change_color: u32,
    pub(crate) symbol_label: String,
    pub(crate) price_display: String,
    pub(crate) debug_loading: bool,
    pub(crate) tooltip: Option<Div>,
}

impl RenderState {
    fn from_view(view: &mut ChartView) -> Self {
        let interval_label = ChartView::interval_label(view.current_interval());
        let playback_label = SharedString::from(if view.replay_enabled() {
            "Replay"
        } else {
            "Live"
        });
        let timezone_label = SharedString::from(
            view.candles
                .last()
                .map(|c| {
                    let offset = c.timestamp.offset();
                    format!("UTC{offset}")
                })
                .unwrap_or_else(|| "UTC".to_string()),
        );

        let (mut start, mut end) = view.visible_range();
        end = end.min(view.candles.len());
        start = start.min(end);
        if start >= end {
            start = 0;
            end = view.candles.len();
        }

        let columns = view
            .chart_bounds
            .map(|b| f32::from(b.size.width).floor().max(1.0) as usize)
            .unwrap_or(0);
        let (cached_min, cached_max, aggregated, volume_max) = if columns > 0 {
            match view.render_cache(start, end, columns) {
                Some(cache) => (
                    Some(cache.padded_min),
                    Some(cache.padded_max),
                    Some(cache.aggregated.clone()),
                    Some(cache.max_volume),
                ),
                None => (None, None, None, None),
            }
        } else {
            (None, None, None, None)
        };

        let visible = if start < end {
            &view.candles[start..end]
        } else {
            &view.candles[..]
        };
        let candle_count = visible.len();

        let (price_min, price_max) = match (cached_min, cached_max) {
            (Some(min), Some(max)) => (min, max),
            _ => padded_bounds(visible),
        };
        view.price_min = price_min;
        view.price_max = price_max;
        let range_text = SharedString::from(format_price_range(price_min, price_max));
        let tooltip = view.tooltip_overlay(start, end);
        let price_labels = [
            format!("{price_max:.4}"),
            format!("{:.4}", (price_min + price_max) * 0.5),
            format!("{price_min:.4}"),
        ];

        let (start_label, mid_label, end_label) = view.time_axis_labels(start, end);

        let candles = view.candles.clone();
        let hover_local = view.hover_index.and_then(|idx| {
            if start <= idx && idx < end {
                Some(idx - start)
            } else {
                None
            }
        });
        let (hover_x, hover_y) = if hover_local.is_some() {
            (
                view.hover_position.map(|(x, _)| x),
                view.hover_position.map(|(_, y)| y),
            )
        } else {
            (None, None)
        };
        let last_close = view.candles.last().map(|c| c.close);
        let prev_close = view.candles.iter().rev().nth(1).map(|c| c.close);
        let (change_display, change_color) = match (last_close, prev_close) {
            (Some(latest), Some(prev)) if prev.abs() > f64::EPSILON => {
                let diff = latest - prev;
                let pct = diff / prev * 100.0;
                let sign = if diff >= 0.0 { "+" } else { "-" };
                (
                    format!("{sign}{:.2} ({sign}{:.2}%)", diff.abs(), pct.abs()),
                    if diff >= 0.0 { 0x22c55e } else { 0xef4444 },
                )
            }
            _ => ("--".to_string(), 0x9ca3af),
        };
        let symbol_label = Path::new(&view.source)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(view.source.as_str())
            .to_string();
        let price_display = last_close
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "--".to_string());
        let debug_loading = view.debug_loading_active();

        Self {
            interval_label,
            playback_label,
            timezone_label,
            candles,
            visible_start: start,
            visible_end: end,
            aggregated,
            volume_max,
            candle_count,
            price_labels,
            start_label,
            mid_label,
            end_label,
            price_min,
            price_max,
            range_text,
            hover_local,
            hover_x,
            hover_y,
            change_display,
            change_color,
            symbol_label,
            price_display,
            debug_loading,
            tooltip,
        }
    }
}

impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Keep animation frames flowing while the blocking overlay is visible.
        if self.loading_symbol.is_some() {
            _window.request_animation_frame();
            _window.refresh();

            if SKIP_CHART_RENDER_WHILE_LOADING {
                let loading_overlay = build_loading_overlay(self, _cx);
                return div()
                    .relative()
                    .w_full()
                    .h_full()
                    .bg(rgb(0x0b1220))
                    .child(loading_overlay.unwrap_or_else(|| div()));
            }
        }

        let state = RenderState::from_view(self);
        let chart_area = build_chart_area(self, _cx, &state);
        let (header, search_overlay) = build_header_bar(self, _cx, &state);
        let footer = build_footer_bar(self, _cx, &state);
        let sidebar = build_sidebar_panels(self, _cx, &state);
        let body = build_body_layout(chart_area, sidebar);
        let interval_menu = build_interval_menu(self, _cx, INTERVAL_OPTIONS);
        let loading_overlay = build_loading_overlay(self, _cx);
        let tooltip = state.tooltip;
        build_layered_view(
            self,
            _cx,
            header,
            body,
            footer,
            search_overlay,
            interval_menu,
            tooltip,
            loading_overlay,
        )
    }
}

fn build_chart_area(view: &mut ChartView, cx: &mut Context<ChartView>, state: &RenderState) -> Div {
    let chart = chart_canvas(
        state.candles.clone(),
        state.visible_start,
        state.visible_end,
        state.price_min,
        state.price_max,
        state.hover_local,
        state.hover_x,
        state.hover_y,
        state.aggregated.clone(),
    )
    .flex_1()
    .w_full()
    .h_full();
    let volume = volume_canvas(
        state.candles.clone(),
        state.visible_start,
        state.visible_end,
        state.hover_local,
        state.hover_x,
        state.aggregated.clone(),
        state.volume_max,
    )
    .flex_1()
    .w_full()
    .h_full();

    chart_body(
        view,
        cx,
        state.price_labels.clone(),
        chart,
        volume,
        state.start_label.clone(),
        state.mid_label.clone(),
        state.end_label.clone(),
        state.candle_count,
    )
}

fn build_header_bar(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    state: &RenderState,
) -> (Div, Option<Div>) {
    let toggle_interval_select =
        cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
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
        .child(state.interval_label.clone());

    let (header_controls, search_overlay) = header_controls(view, cx, interval_trigger);

    let toggle_replay = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
        let next = !this.replay_enabled();
        this.set_replay_mode(next);
        window.refresh();
    });
    let replay_chip = {
        let active = view.replay_enabled();
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

    let trigger_debug_loader =
        cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.start_debug_loading();
            window.refresh();
        });
    let mut debug_loader_button = div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if state.debug_loading {
            rgb(0xf59e0b)
        } else {
            rgb(0x1f2937)
        })
        .bg(rgb(0x111827))
        .text_sm()
        .text_color(gpui::white())
        .on_mouse_down(MouseButton::Left, trigger_debug_loader)
        .child("Debug load");
    if state.debug_loading {
        debug_loader_button = debug_loader_button.child(loading_sand(16.0, rgb(0xf59e0b)));
    }

    let perf_presets = view.is_perf_mode().then(|| {
        let active_n = view.perf_current_n();
        let preset = |label: &'static str, n: usize, cx: &mut Context<ChartView>| {
            let active = active_n == Some(n);
            let handle = cx.listener(move |this: &mut ChartView, _: &MouseDownEvent, window, cx| {
                this.start_perf_preset_load(n, window, cx);
            });
            header_chip(label)
                .border_color(if active { rgb(0x2563eb) } else { rgb(0x1f2937) })
                .text_color(if active { rgb(0xffffff) } else { rgb(0xe5e7eb) })
                .on_mouse_down(MouseButton::Left, handle)
        };

        div()
            .flex()
            .items_center()
            .gap_1()
            .child(preset("50k", 50_000, cx))
            .child(preset("200k", 200_000, cx))
            .child(preset("1M", 1_000_000, cx))
    });

    let header_left = div()
        .flex()
        .items_center()
        .gap_3()
        .child(header_controls)
        .child(header_icon("chart-create.svg", "Indicators"))
        .child(header_icon("compare.svg", "Compare"))
        .child(header_icon("alarm-clock.svg", "Alerts"))
        .child(replay_chip);

    let mut header_right = div()
        .flex()
        .items_center()
        .gap_2()
        .child(header_chip("Log"))
        .child(header_chip("Auto"));
    if let Some(perf_presets) = perf_presets {
        header_right = header_right.child(perf_presets);
    }
    header_right = header_right
        .child(debug_loader_button)
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

    (chart_header(header_left, header_right), search_overlay)
}

fn build_quick_ranges(view: &mut ChartView, cx: &mut Context<ChartView>) -> Div {
    let mut quick_ranges = div().flex().items_center().gap_2();
    for (idx, (label, _)) in QUICK_RANGE_WINDOWS.iter().enumerate() {
        let is_active = view.current_range_index() == idx;
        let handle = cx.listener(move |this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.apply_range_index(idx, true);
            window.refresh();
        });
        quick_ranges = quick_ranges
            .child(range_button(*label, is_active).on_mouse_down(MouseButton::Left, handle));
    }
    quick_ranges
}

fn build_footer_bar(view: &mut ChartView, cx: &mut Context<ChartView>, state: &RenderState) -> Div {
    let quick_ranges = build_quick_ranges(view, cx);
    chart_footer(
        quick_ranges,
        state.interval_label.clone(),
        state.candle_count,
        state.range_text.clone(),
        !view.replay_enabled(),
        state.playback_label.clone(),
        state.timezone_label.clone(),
    )
}
