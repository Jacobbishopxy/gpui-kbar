use core::{Candle, Interval, bounds, resample};
use gpui::{
    Bounds, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Render, ScrollWheelEvent, SharedString, Window, div, prelude::*, px, rgb,
};

use super::{canvas::chart_canvas, header::chart_header};

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
    hover_index: Option<usize>,
    hover_position: Option<(f32, f32)>,
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
            hover_index: None,
            hover_position: None,
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

    fn handle_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window) {
        if self.candles.is_empty() {
            return;
        }
        let delta = event.delta.pixel_delta(px(16.0));
        let scroll_y = f32::from(delta.y);
        if scroll_y.abs() < f32::EPSILON {
            return;
        }
        let center = self.view_offset + self.visible_len() * 0.5;
        let zoom_factor = if scroll_y < 0.0 { 1.1 } else { 0.9 };
        self.zoom = (self.zoom * zoom_factor).clamp(1.0, self.candles.len() as f32);
        let new_visible = self.visible_len();
        let new_offset = center - new_visible * 0.5;
        let visible_count = new_visible.round().max(1.0) as usize;
        self.view_offset = self.clamp_offset(new_offset, visible_count);
        window.refresh();
    }

    fn handle_hover(&mut self, event: &MouseMoveEvent, candle_count: usize) {
        if let (Some(bounds), true) = (
            self.chart_bounds,
            !self.candles.is_empty() && candle_count > 0,
        ) {
            let bx = f32::from(bounds.origin.x);
            let by = f32::from(bounds.origin.y);
            let bw = f32::from(bounds.size.width);
            let bh = f32::from(bounds.size.height);
            let px = f32::from(event.position.x);
            let py = f32::from(event.position.y);
            if px >= bx && px <= bx + bw && py >= by && py <= by + bh {
                let candle_width = (bw / candle_count as f32).max(1.0);
                let local_x = (px - bx).max(0.0);
                let local_idx = (local_x / candle_width).floor() as usize;
                let start_idx = self.visible_range().0;
                let idx = (start_idx + local_idx).min(self.candles.len().saturating_sub(1));
                self.hover_index = Some(idx);
                self.hover_position = Some((px, py));
            } else {
                self.hover_index = None;
                self.hover_position = None;
            }
        }
    }

    fn handle_drag(&mut self, event: &MouseMoveEvent, window: &mut Window) {
        if !self.dragging || self.candles.is_empty() {
            window.refresh();
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            self.dragging = false;
            self.last_drag_position = None;
            window.refresh();
            return;
        }

        if let Some((last_x, _)) = self.last_drag_position {
            let dx = f32::from(event.position.x) - last_x;
            let width = self
                .chart_bounds
                .map(|b| f32::from(b.size.width).max(1.0))
                .unwrap_or(1.0);
            let visible = self.visible_len();
            if visible > 0.0 {
                let candles_per_px = visible / width.max(1e-3);
                let new_offset = self.view_offset - dx * candles_per_px;
                let visible_count = visible.round().max(1.0) as usize;
                self.view_offset = self.clamp_offset(new_offset, visible_count);
            }
        }

        self.last_drag_position = Some((f32::from(event.position.x), f32::from(event.position.y)));
        window.refresh();
    }

    fn tooltip_overlay(&self, start: usize, end: usize) -> Option<Div> {
        let (idx, (mx, my), bounds) = (self.hover_index?, self.hover_position?, self.chart_bounds?);
        let candle = self.candles.get(idx)?;
        if idx < start || idx >= end {
            return None;
        }

        let origin_x = f32::from(bounds.origin.x);
        let origin_y = f32::from(bounds.origin.y);
        let max_x = origin_x + f32::from(bounds.size.width);
        let max_y = origin_y + f32::from(bounds.size.height);
        let mut x = mx + 12.0;
        let mut y = my + 12.0;
        let tip_width = 180.0;
        let tip_height = 88.0;
        if x + tip_width > max_x {
            x = (max_x - tip_width).max(origin_x);
        }
        if y + tip_height > max_y {
            y = (max_y - tip_height).max(origin_y);
        }

        let ts = candle.timestamp;
        let idx_line = format!("#{idx}");
        let o_line = format!("O: {:.4}", candle.open);
        let h_line = format!("H: {:.4}", candle.high);
        let l_line = format!("L: {:.4}", candle.low);
        let c_line = format!("C: {:.4}", candle.close);
        let v_line = format!("V: {:.2}", candle.volume);

        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .bg(rgb(0x111827))
                .border_1()
                .border_color(rgb(0x1f2937))
                .rounded_md()
                .shadow_lg()
                .p_2()
                .text_xs()
                .flex()
                .flex_col()
                .gap_1()
                .child(ts.to_string())
                .child(idx_line)
                .child(o_line)
                .child(h_line)
                .child(l_line)
                .child(c_line)
                .child(v_line),
        )
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
        let tooltip = self.tooltip_overlay(start, end);

        let candles = visible.to_vec();
        let price_min = self.price_min;
        let price_max = self.price_max;
        let hover_local = self.hover_index.and_then(|idx| {
            if start <= idx && idx < end {
                Some(idx - start)
            } else {
                None
            }
        });

        let track_bounds = _cx.processor(|this: &mut Self, bounds: Vec<Bounds<Pixels>>, _, _| {
            if let Some(chart_bounds) = bounds.get(1) {
                this.chart_bounds = Some(*chart_bounds);
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

        let chart = chart_canvas(candles, price_min, price_max, hover_local)
            .flex_1()
            .w_full();

        let mut chart_area = div()
            .flex_1()
            .flex()
            .w_full()
            .on_mouse_down(MouseButton::Left, handle_mouse_down)
            .on_mouse_move(handle_mouse_move)
            .on_mouse_up(MouseButton::Left, handle_mouse_up)
            .on_scroll_wheel(handle_scroll)
            .child(chart);
        if let Some(tip) = tooltip {
            chart_area = chart_area.child(tip);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(chart_header(
                &self.source,
                interval_label,
                candle_count,
                range_text,
            ))
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
