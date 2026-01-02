use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use core::{Candle, Interval, bounds, resample};
use gpui::{Bounds, Pixels, SharedString};
use time::Duration;

use super::super::ChartMeta;
use crate::data::symbols::{SymbolMeta, load_symbols};
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
    pub watchlist: Vec<String>,
    hydrated: bool,
    symbols: HashMap<String, SymbolMeta>,
}

impl ChartView {
    pub fn default_watchlist() -> Vec<String> {
        vec![
            "AAPL".to_string(),
            "TSLA".to_string(),
            "NFLX".to_string(),
            "USOIL".to_string(),
        ]
    }
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
            watchlist: Vec::new(),
            hydrated: false,
            symbols: HashMap::new(),
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

    pub fn watchlist_symbols(&self) -> Vec<String> {
        self.watchlist.clone()
    }

    pub fn symbol_meta(&mut self, symbol: &str) -> Option<SymbolMeta> {
        self.ensure_symbol_catalog();
        self.symbols.get(symbol).cloned()
    }

    pub fn resolve_symbol_source(&mut self, symbol: &str) -> String {
        self.ensure_symbol_catalog();
        self.symbols
            .get(symbol)
            .map(|meta| meta.source.clone())
            .unwrap_or_else(|| "../data/sample.csv".to_string())
    }

    pub fn current_source(&self) -> String {
        self.source.clone()
    }

    pub fn hydrate_from_store(&mut self) {
        if self.hydrated {
            return;
        }
        if let Some(store_rc) = self.store.clone() {
            let (watchlist, interval_str, range_idx_str, replay_str) = {
                let store = store_rc.borrow();
                (
                    store.get_watchlist().unwrap_or_default(),
                    store.get_session_value("interval").ok().flatten(),
                    store.get_session_value("range_index").ok().flatten(),
                    store.get_session_value("replay_mode").ok().flatten(),
                )
            };

            if watchlist.is_empty() {
                self.watchlist = Self::default_watchlist();
                let _ = store_rc.borrow_mut().set_watchlist(&self.watchlist);
            } else {
                self.watchlist = watchlist;
            }

            if let Some(interval) = interval_str.and_then(|interval| match interval.as_str() {
                "raw" => Some(None),
                s if s.ends_with('s') => s
                    .trim_end_matches('s')
                    .parse()
                    .ok()
                    .map(Interval::Second)
                    .map(Some),
                s if s.ends_with('m') => s
                    .trim_end_matches('m')
                    .parse()
                    .ok()
                    .map(Interval::Minute)
                    .map(Some),
                s if s.ends_with('h') => s
                    .trim_end_matches('h')
                    .parse()
                    .ok()
                    .map(Interval::Hour)
                    .map(Some),
                s if s.ends_with('d') => s
                    .trim_end_matches('d')
                    .parse()
                    .ok()
                    .map(Interval::Day)
                    .map(Some),
                _ => None,
            }) {
                self.apply_interval(interval, false);
            }

            if let Some(idx) = range_idx_str.and_then(|r| r.parse::<usize>().ok()) {
                self.apply_range_index(idx, false);
            }

            if let Some(replay) = replay_str {
                self.set_replay_mode(replay == "true");
            }
            self.hydrated = true;
        }
    }

    pub fn add_to_watchlist(&mut self, symbol: String) {
        if !self.watchlist.iter().any(|s| s == &symbol) {
            self.watchlist.push(symbol.clone());
            if let Some(store) = &self.store {
                let _ = store.borrow_mut().set_watchlist(&self.watchlist);
            }
        }
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

    pub(super) fn apply_interval(&mut self, interval: Option<Interval>, persist: bool) {
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
        self.apply_range_index(self.active_range_index, persist);
        if persist {
            let _ = self.persist_session("interval", &Self::interval_label(self.interval));
        }
    }

    pub(crate) fn replace_data(&mut self, base: Vec<Candle>, source: String, persist: bool) {
        self.base_candles = base;
        let interval = self.interval;
        self.apply_interval(interval, persist);
        self.source = source;
        self.load_error = None;
        self.loading_symbol = None;
        if persist {
            self.add_to_watchlist(self.source.clone());
            let _ = self.persist_session("active_source", &self.source);
        }
    }

    pub(super) fn apply_range_index(&mut self, index: usize, persist: bool) {
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
        if persist {
            let _ = self.persist_session("range_index", &self.active_range_index.to_string());
        }
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

    fn ensure_symbol_catalog(&mut self) {
        if !self.symbols.is_empty() {
            return;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/symbols.csv");
        if let Ok(map) = load_symbols(path.to_str().unwrap_or_default()) {
            self.symbols = map;
        }
    }

    pub fn remove_from_watchlist(&mut self, symbol: &str) {
        let len_before = self.watchlist.len();
        self.watchlist.retain(|s| s != symbol);
        if len_before != self.watchlist.len() {
            if let Some(store) = &self.store {
                let _ = store.borrow_mut().set_watchlist(&self.watchlist);
            }
        }
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
