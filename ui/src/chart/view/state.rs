use core::{Candle, Interval, bounds, resample};
use gpui::{Bounds, Pixels, SharedString};

use super::super::ChartMeta;

pub(crate) struct ChartView {
    pub(super) candles: Vec<Candle>,
    pub(super) price_min: f64,
    pub(super) price_max: f64,
    pub(super) interval: Option<Interval>,
    pub(super) source: String,
    pub(super) view_offset: f32,
    pub(super) zoom: f32,
    pub(super) chart_bounds: Option<Bounds<Pixels>>,
    pub(super) last_drag_position: Option<(f32, f32)>,
    pub(super) dragging: bool,
    pub(super) hover_index: Option<usize>,
    pub(super) hover_position: Option<(f32, f32)>,
}

impl ChartView {
    pub(crate) fn new(base_candles: Vec<Candle>, meta: ChartMeta) -> Self {
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

    pub(super) fn interval_label(interval: Option<Interval>) -> SharedString {
        let label = match interval {
            Some(Interval::Minute(n)) => format!("{n}m"),
            Some(Interval::Hour(n)) => format!("{n}h"),
            Some(Interval::Day(n)) => format!("{n}d"),
            None => "raw".to_string(),
        };
        SharedString::from(label)
    }

    pub(super) fn visible_len(&self) -> f32 {
        if self.candles.is_empty() {
            return 0.0;
        }
        let zoom = self.zoom.max(1.0).min(self.candles.len() as f32);
        (self.candles.len() as f32 / zoom).max(1.0)
    }

    pub(super) fn clamp_offset(&self, offset: f32, visible_count: usize) -> f32 {
        if self.candles.is_empty() {
            return 0.0;
        }
        let max_start = self.candles.len().saturating_sub(visible_count);
        offset.clamp(0.0, max_start as f32)
    }

    pub(super) fn visible_range(&self) -> (usize, usize) {
        if self.candles.is_empty() {
            return (0, 0);
        }
        let visible = self.visible_len().round().max(1.0) as usize;
        let start = self.clamp_offset(self.view_offset, visible).round() as usize;
        let end = (start + visible).min(self.candles.len());
        (start, end)
    }
}

pub(crate) fn padded_bounds(candles: &[Candle]) -> (f64, f64) {
    let (min, mut max) = bounds(candles).unwrap_or((0.0, 1.0));
    if min == max {
        max = min + 1.0;
    }
    let pad = ((max - min) * 0.01).max(0.0);
    (min - pad, max + pad)
}
