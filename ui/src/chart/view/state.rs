use std::{cell::RefCell, rc::Rc};

use core::{Candle, Interval, bounds, resample};
use gpui::{Bounds, Pixels, SharedString};
use time::Duration;

use super::super::ChartMeta;
use core::DuckDbStore;

pub const QUICK_RANGE_WINDOWS: [(&str, Option<Duration>); 8] = [
    ("1D", Some(Duration::days(1))),
    ("5D", Some(Duration::days(5))),
    ("1M", Some(Duration::days(30))),
    ("3M", Some(Duration::days(90))),
    ("6M", Some(Duration::days(180))),
    ("1Y", Some(Duration::days(365))),
    ("5Y", Some(Duration::days(365 * 5))),
    ("ALL", None),
];

pub struct ChartView {
    pub(super) base_candles: Vec<Candle>,
    pub(super) candles: Vec<Candle>,
    pub(super) price_min: f64,
    pub(super) price_max: f64,
    interval: Option<Interval>,
    pub(super) source: String,
    pub(super) view_offset: f32,
    pub(super) zoom: f32,
    pub(super) root_origin: (f32, f32),
    pub(super) chart_bounds: Option<Bounds<Pixels>>,
    pub(super) interval_trigger_origin: (f32, f32),
    pub(super) interval_trigger_height: f32,
    pub(super) last_drag_position: Option<(f32, f32)>,
    pub(super) dragging: bool,
    pub(super) hover_index: Option<usize>,
    pub(super) hover_position: Option<(f32, f32)>,
    pub(super) interval_select_open: bool,
    pub(super) symbol_search_open: bool,
    active_range_index: usize,
    replay_mode: bool,
    pub loading_symbol: Option<String>,
    pub load_error: Option<String>,
    pub store: Option<Rc<RefCell<DuckDbStore>>>,
}

impl ChartView {
    pub(crate) fn new(
        base_candles: Vec<Candle>,
        meta: ChartMeta,
        store: Option<Rc<RefCell<DuckDbStore>>>,
    ) -> Self {
        let base = base_candles;
        let (candles, interval) = match meta.initial_interval {
            Some(i) => (resample(&base, i), Some(i)),
            None => (base.clone(), None),
        };
        let (price_min, price_max) = padded_bounds(&candles);
        Self {
            base_candles: base,
            candles,
            price_min,
            price_max,
            interval,
            source: meta.source,
            view_offset: 0.0,
            zoom: 1.0,
            root_origin: (0.0, 0.0),
            chart_bounds: None,
            interval_trigger_origin: (0.0, 0.0),
            interval_trigger_height: 40.0,
            last_drag_position: None,
            dragging: false,
            hover_index: None,
            hover_position: None,
            interval_select_open: false,
            symbol_search_open: false,
            active_range_index: QUICK_RANGE_WINDOWS.len().saturating_sub(1),
            replay_mode: false,
            loading_symbol: None,
            load_error: None,
            store,
        }
    }

    pub fn interval_label(interval: Option<Interval>) -> SharedString {
        let label = match interval {
            Some(Interval::Second(n)) => format!("{n}s"),
            Some(Interval::Minute(n)) => format!("{n}m"),
            Some(Interval::Hour(n)) => format!("{n}h"),
            Some(Interval::Day(n)) => format!("{n}d"),
            None => "raw".to_string(),
        };
        SharedString::from(label)
    }

    pub fn current_interval(&self) -> Option<Interval> {
        self.interval
    }

    pub fn current_range_index(&self) -> usize {
        self.active_range_index
    }

    pub fn replay_enabled(&self) -> bool {
        self.replay_mode
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

    pub(super) fn apply_interval(&mut self, interval: Option<Interval>) {
        self.interval = interval;
        self.candles = match interval {
            Some(i) => resample(&self.base_candles, i),
            None => self.base_candles.clone(),
        };
        self.view_offset = 0.0;
        self.zoom = 1.0;
        self.hover_index = None;
        self.hover_position = None;
        self.interval_select_open = false;
        self.symbol_search_open = false;
        self.apply_range_index(self.active_range_index);
        let _ = self.persist_session("interval", &Self::interval_label(self.interval));
    }

    pub(crate) fn replace_data(&mut self, base: Vec<Candle>, source: String) {
        self.base_candles = base;
        let interval = self.interval;
        self.apply_interval(interval);
        self.source = source;
        self.load_error = None;
        self.loading_symbol = None;
        let _ = self.persist_session("active_source", &self.source);
    }

    pub(super) fn apply_range_index(&mut self, index: usize) {
        let clamped_index = index.min(QUICK_RANGE_WINDOWS.len().saturating_sub(1));
        self.active_range_index = clamped_index;
        self.interval_select_open = false;
        self.symbol_search_open = false;
        self.hover_index = None;
        self.hover_position = None;
        self.dragging = false;
        self.last_drag_position = None;

        if self.candles.is_empty() {
            return;
        }

        let (_, duration) = QUICK_RANGE_WINDOWS[clamped_index];
        match duration {
            Some(duration) => {
                let last_ts = self.candles.last().map(|c| c.timestamp);
                if let Some(end) = last_ts {
                    let start = end - duration;
                    let start_idx = match self.candles.binary_search_by(|c| c.timestamp.cmp(&start))
                    {
                        Ok(i) => i,
                        Err(i) => i,
                    };
                    let start_idx = start_idx.min(self.candles.len().saturating_sub(1));
                    let visible = self.candles.len().saturating_sub(start_idx).max(1);
                    self.zoom = (self.candles.len() as f32 / visible as f32)
                        .clamp(1.0, self.candles.len() as f32);
                    self.view_offset = self.clamp_offset(start_idx as f32, visible);
                }
            }
            None => {
                self.zoom = 1.0;
                self.view_offset = 0.0;
            }
        }
        let _ = self.persist_session("range_index", &self.active_range_index.to_string());
    }

    pub(super) fn set_replay_mode(&mut self, enabled: bool) {
        self.replay_mode = enabled;
        let _ = self.persist_session("replay_mode", if enabled { "true" } else { "false" });
    }

    fn persist_session(&self, key: &str, value: &str) -> Result<(), ()> {
        if let Some(store) = &self.store {
            store
                .borrow_mut()
                .set_session_value(key, value)
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

pub fn padded_bounds(candles: &[Candle]) -> (f64, f64) {
    let (min, mut max) = bounds(candles).unwrap_or((0.0, 1.0));
    if min == max {
        max = min + 1.0;
    }
    let pad = ((max - min) * 0.01).max(0.0);
    (min - pad, max + pad)
}
