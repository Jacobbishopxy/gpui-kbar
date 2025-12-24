use core::{bounds, resample, Candle, Interval};
use gpui::{
    BorderStyle, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathBuilder, Pixels, Render, ScrollWheelEvent, SharedString, Window, canvas, div, point,
    prelude::*, px, quad, rgb, size, transparent_black,
};

use super::ChartMeta;

pub(super) struct ChartView {
    candles: Vec<Candle>,
    price_min: f64,
    price_max: f64,
    interval: Option<Interval>,
    source: String,
    view_offset: f32,
    zoom: f32,
    chart_bounds: Option<Bounds<Pixels>>,
    last_drag_position: Option<(f32, f32)>,
    dragging: bool,
}

impl ChartView {
    pub(super) fn new(base_candles: Vec<Candle>, meta: ChartMeta) -> Self {
        let (candles, interval) = match meta.initial_interval {
            Some(i) => (resample(&base_candles, i), Some(i)),
            None => (base_candles, None),
        };
        let (price_min, price_max) = padded_bounds(&candles);
        Self {
            candles,
            price_min,
            price_max,
            interval,
            source: meta.source,
            view_offset: 0.0,
            zoom: 1.0,
            chart_bounds: None,
            last_drag_position: None,
            dragging: false,
        }
    }

    fn interval_label(interval: Option<Interval>) -> SharedString {
        let label = match interval {
            Some(Interval::Minute(n)) => format!("{n}m"),
            Some(Interval::Hour(n)) => format!("{n}h"),
            Some(Interval::Day(n)) => format!("{n}d"),
            None => "raw".to_string(),
        };
        SharedString::from(label)
    }

    fn visible_len(&self) -> f32 {
        if self.candles.is_empty() {
            return 0.0;
        }
        let zoom = self.zoom.max(1.0).min(self.candles.len() as f32);
        (self.candles.len() as f32 / zoom).max(1.0)
    }

    fn clamp_offset(&self, offset: f32, visible_count: usize) -> f32 {
        if self.candles.is_empty() {
            return 0.0;
        }
        let max_start = self.candles.len().saturating_sub(visible_count);
        offset.clamp(0.0, max_start as f32)
    }

    fn visible_range(&self) -> (usize, usize) {
        if self.candles.is_empty() {
            return (0, 0);
        }
        let visible = self.visible_len().round().max(1.0) as usize;
        let start = self.clamp_offset(self.view_offset, visible).round() as usize;
        let end = (start + visible).min(self.candles.len());
        (start, end)
    }
}

impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let interval_label = Self::interval_label(self.interval);
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

        let candles = visible.to_vec();
        let price_min = self.price_min;
        let price_max = self.price_max;

        let track_bounds =
            _cx.processor(|this: &mut Self, bounds: Vec<Bounds<Pixels>>, _, _| {
                if let Some(chart_bounds) = bounds.get(1) {
                    this.chart_bounds = Some(*chart_bounds);
                }
            });

        let handle_scroll = _cx.listener(|this: &mut Self, event: &ScrollWheelEvent, window, _| {
            if this.candles.is_empty() {
                return;
            }
            let delta = event.delta.pixel_delta(px(16.0));
            let scroll_y = f32::from(delta.y);
            if scroll_y.abs() < f32::EPSILON {
                return;
            }
            let center = this.view_offset + this.visible_len() * 0.5;
            let zoom_factor = if scroll_y < 0.0 { 1.1 } else { 0.9 };
            this.zoom = (this.zoom * zoom_factor).clamp(1.0, this.candles.len() as f32);
            let new_visible = this.visible_len();
            let new_offset = center - new_visible * 0.5;
            let visible_count = new_visible.round().max(1.0) as usize;
            this.view_offset = this.clamp_offset(new_offset, visible_count);
            window.refresh();
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
            _cx.listener(|this: &mut Self, event: &MouseMoveEvent, window, _| {
                if !this.dragging || this.candles.is_empty() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    this.dragging = false;
                    this.last_drag_position = None;
                    return;
                }

                if let Some((last_x, _)) = this.last_drag_position {
                    let dx = f32::from(event.position.x) - last_x;
                    let width = this
                        .chart_bounds
                        .map(|b| f32::from(b.size.width).max(1.0))
                        .unwrap_or(1.0);
                    let visible = this.visible_len();
                    if visible > 0.0 {
                        let candles_per_px = visible / width.max(1e-3);
                        let new_offset =
                            this.view_offset - dx * candles_per_px;
                        let visible_count = visible.round().max(1.0) as usize;
                        this.view_offset = this.clamp_offset(new_offset, visible_count);
                    }
                }

                this.last_drag_position =
                    Some((f32::from(event.position.x), f32::from(event.position.y)));
                window.refresh();
            });

        let chart = canvas(
            move |_, _, _| candles.clone(),
            move |bounds, candles, window, _| {
                window.paint_quad(quad(
                    bounds,
                    px(0.),
                    rgb(0x0b1220),
                    px(0.),
                    transparent_black(),
                    BorderStyle::default(),
                ));

                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                let ox = f32::from(bounds.origin.x);
                let oy = f32::from(bounds.origin.y);
                if candles.is_empty() || height <= 0.0 || width <= 0.0 {
                    return;
                }

                let range = (price_max - price_min).max(1e-9);
                let candle_width = (width / candles.len() as f32).max(1.0);
                let body_width = (candle_width * 0.6).max(1.0);

                let price_to_y = |price: f64| -> f32 {
                    let normalized = ((price - price_min) / range).clamp(0.0, 1.0);
                    oy + (1.0 - normalized as f32) * height
                };

                // gridlines (min/mid/max)
                for frac in [0.0f32, 0.5, 1.0] {
                    let y = oy + height * (1.0 - frac);
                    let mut builder = PathBuilder::stroke(px(1.));
                    builder.move_to(point(px(ox), px(y)));
                    builder.line_to(point(px(ox + width), px(y)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, rgb(0x1f2937));
                    }
                }

                for (idx, candle) in candles.iter().enumerate() {
                    let x = ox + idx as f32 * candle_width + candle_width * 0.5;
                    let open_y = price_to_y(candle.open);
                    let close_y = price_to_y(candle.close);
                    let high_y = price_to_y(candle.high);
                    let low_y = price_to_y(candle.low);

                    let body_top = open_y.min(close_y);
                    let body_height = (open_y - close_y).abs().max(1.0);
                    let color = if candle.close >= candle.open {
                        rgb(0x22c55e)
                    } else {
                        rgb(0xef4444)
                    };

                    let mut builder = PathBuilder::stroke(px(1.));
                    builder.move_to(point(px(x), px(high_y)));
                    builder.line_to(point(px(x), px(low_y)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, rgb(0xe5e7eb));
                    }

                    let body_bounds = Bounds {
                        origin: point(px(x - body_width * 0.5), px(body_top)),
                        size: size(px(body_width), px(body_height)),
                    };
                    window.paint_quad(quad(
                        body_bounds,
                        px(2.),
                        color,
                        px(0.),
                        color,
                        BorderStyle::default(),
                    ));
                }
            },
        )
        .flex_1()
        .w_full();

        let chart_area = div()
            .flex_1()
            .flex()
            .w_full()
            .on_mouse_down(MouseButton::Left, handle_mouse_down)
            .on_mouse_move(handle_mouse_move)
            .on_mouse_up(MouseButton::Left, handle_mouse_up)
            .on_scroll_wheel(handle_scroll)
            .child(chart);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .p_3()
                    .bg(rgb(0x111827))
                    .border_b_1()
                    .border_color(rgb(0x1f2937))
                    .child(
                        div()
                            .text_sm()
                            .child(SharedString::from(self.source.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .text_sm()
                            .child(format!("interval: {interval_label}"))
                            .child(format!("candles: {candle_count}"))
                            .child(format!("range: {range_text}")),
                    ),
            )
            .on_children_prepainted(track_bounds)
            .child(chart_area)
    }
}

fn padded_bounds(candles: &[Candle]) -> (f64, f64) {
    let (min, mut max) = bounds(candles).unwrap_or((0.0, 1.0));
    if min == max {
        max = min + 1.0;
    }
    let pad = ((max - min) * 0.01).max(0.0);
    (min - pad, max + pad)
}
