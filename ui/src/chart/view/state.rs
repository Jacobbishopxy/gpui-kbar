use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use core::{Candle, Interval, LoadOptions, bounds, load_csv, resample};
use gpui::{Bounds, Context, Pixels, SharedString, Window};
use time::Duration;

use super::super::ChartMeta;
use crate::data::{
    symbols::{SymbolMeta, load_symbols},
    universe::{SymbolSearchEntry, load_universe},
};
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
    pub(super) base_candles: Arc<[Candle]>,
    pub(super) candles: Arc<[Candle]>,
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
    pub(super) symbol_search_add_to_watchlist: bool,
    active_range_index: usize,
    replay_mode: bool,
    pub loading_symbol: Option<String>,
    pub load_error: Option<String>,
    pub store: Option<Rc<RefCell<DuckDbStore>>>,
    pub watchlist: Vec<String>,
    hydrated: bool,
    symbols: HashMap<String, SymbolMeta>,
    symbol_search_filter: String,
    universe: Vec<SymbolSearchEntry>,
    resample_cache: Vec<(Option<Interval>, Arc<[Candle]>)>,
}

impl ChartView {
    pub(crate) fn new(
        base_candles: Vec<Candle>,
        meta: ChartMeta,
        store: Option<Rc<RefCell<DuckDbStore>>>,
    ) -> Self {
        let base_arc: Arc<[Candle]> = base_candles.into();
        let (candles, interval) = match meta.initial_interval {
            Some(i) => (Arc::from(resample(&base_arc, i)), Some(i)),
            None => (base_arc.clone(), None),
        };
        let (price_min, price_max) = padded_bounds(&candles);
        Self {
            base_candles: base_arc.clone(),
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
            symbol_search_add_to_watchlist: false,
            active_range_index: QUICK_RANGE_WINDOWS.len().saturating_sub(1),
            replay_mode: false,
            loading_symbol: None,
            load_error: None,
            store,
            watchlist: Vec::new(),
            hydrated: false,
            symbols: HashMap::new(),
            symbol_search_filter: "All".to_string(),
            universe: Vec::new(),
            resample_cache: vec![(None, base_arc)],
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

    pub fn symbol_search_filter(&self) -> &str {
        &self.symbol_search_filter
    }

    pub fn set_symbol_search_filter(&mut self, filter: &str) {
        self.symbol_search_filter = filter.to_string();
    }

    pub fn symbol_universe(&mut self) -> &[SymbolSearchEntry] {
        self.ensure_symbol_universe();
        &self.universe
    }

    pub fn start_symbol_load(
        &mut self,
        symbol: String,
        add_to_watchlist: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_symbol_catalog();
        if self.loading_symbol.as_deref() == Some(&symbol) {
            return;
        }
        if self.source == symbol {
            self.symbol_search_open = false;
            window.refresh();
            return;
        }

        let source = self.resolve_symbol_source(&symbol);
        let resolved = resolve_source_path(&source);

        if let Some(store_rc) = self.store.clone() {
            let cached = {
                let store_ref = store_rc.borrow();
                store_ref
                    .load_candles(&symbol, None)
                    .ok()
                    .filter(|c| !c.is_empty())
            };
            if let Some(cached) = cached {
                self.replace_data(cached, symbol.clone(), true, add_to_watchlist);
                self.loading_symbol = None;
                self.load_error = None;
                self.symbol_search_open = false;
                let _ = store_rc
                    .borrow_mut()
                    .set_session_value("active_source", &symbol);
                window.refresh();
                return;
            }
        }

        self.loading_symbol = Some(symbol.clone());
        self.load_error = None;
        self.symbol_search_open = false;
        window.refresh();

        let entity = cx.entity();
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            let candles = load_csv(&resolved, LoadOptions::default())
                .map_err(|e| format!("failed to load {symbol} from {}: {e}", resolved.display()))?;

            if candles.is_empty() {
                Err(format!("no candles loaded for {symbol}"))
            } else {
                Ok((symbol, candles))
            }
        });

        window
            .spawn(cx, async move |async_cx| {
                let result = task.await;
                async_cx
                    .update(|window, app| {
                        let _ = entity.update(app, |view, cx| {
                            view.loading_symbol = None;
                            match result {
                                Ok((symbol, candles)) => {
                                    view.load_error = None;
                                    if let Some(store) = store.as_ref() {
                                        let _ = store.borrow_mut().write_candles(&symbol, &candles);
                                        let _ = store
                                            .borrow_mut()
                                            .set_session_value("active_source", &symbol);
                                    }
                                    view.replace_data(
                                        candles,
                                        symbol.clone(),
                                        true,
                                        add_to_watchlist,
                                    );
                                    cx.notify();
                                }
                                Err(msg) => {
                                    view.load_error = Some(msg);
                                }
                            }
                            window.refresh();
                        });
                    })
                    .ok();
            })
            .detach();
    }

    pub fn hydrate_from_store(&mut self) {
        if self.hydrated {
            return;
        }
        if let Some(store_rc) = self.store.clone() {
            let session = store_rc.borrow().load_user_session().unwrap_or_default();

            self.watchlist = session.watchlist;

            if let Some(interval) = session
                .interval
                .and_then(|interval| match interval.as_str() {
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
                })
            {
                self.apply_interval(interval, false);
            }

            if let Some(idx) = session.range_index {
                self.apply_range_index(idx, false);
            }

            if let Some(replay) = session.replay_mode {
                self.set_replay_mode(replay);
            }

            if let Some(zoom) = session.zoom {
                let max_zoom = self.candles.len().max(1) as f32;
                self.zoom = zoom.clamp(1.0, max_zoom);
            }

            if let Some(offset) = session.view_offset {
                let visible_count = self.visible_len().round().max(1.0) as usize;
                self.view_offset = self.clamp_offset(offset, visible_count);
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
        self.candles = self.resampled_for(interval);
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

    fn resampled_for(&mut self, interval: Option<Interval>) -> Arc<[Candle]> {
        if let Some((_, cached)) = self
            .resample_cache
            .iter()
            .find(|(cached_interval, _)| *cached_interval == interval)
        {
            return cached.clone();
        }

        let arc = match interval {
            Some(i) => Arc::from(resample(&self.base_candles, i)),
            None => self.base_candles.clone(),
        };
        self.resample_cache.push((interval, arc.clone()));
        arc
    }

    pub(crate) fn replace_data(
        &mut self,
        base: Vec<Candle>,
        source: String,
        persist_session: bool,
        add_to_watchlist: bool,
    ) {
        let base_arc: Arc<[Candle]> = base.into();
        self.base_candles = base_arc.clone();
        self.resample_cache.clear();
        self.resample_cache.push((None, base_arc.clone()));
        let interval = self.interval;
        self.apply_interval(interval, persist_session);
        self.source = source;
        self.load_error = None;
        self.loading_symbol = None;
        if add_to_watchlist {
            self.add_to_watchlist(self.source.clone());
        }
        if persist_session {
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
            let _ = self.persist_viewport();
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

    pub(super) fn persist_viewport(&self) -> Result<(), ()> {
        if let Some(store) = &self.store {
            let store = store.borrow_mut();
            store
                .set_session_value("view_offset", &self.view_offset.to_string())
                .map_err(|_| ())?;
            store
                .set_session_value("zoom", &self.zoom.to_string())
                .map_err(|_| ())?;
        }
        Ok(())
    }

    fn ensure_symbol_catalog(&mut self) {
        if !self.symbols.is_empty() {
            return;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/mapping.csv");
        if let Ok(map) = load_symbols(path.to_str().unwrap_or_default()) {
            self.symbols = map;
        }
    }

    pub fn ensure_symbol_universe(&mut self) {
        if !self.universe.is_empty() {
            return;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/universe.csv");
        if let Ok(entries) = load_universe(path.to_str().unwrap_or_default()) {
            self.universe = entries;
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

fn resolve_source_path(relative: &str) -> PathBuf {
    let path = Path::new(relative);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }
}
