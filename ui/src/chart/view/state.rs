use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use core::{Candle, Interval, LoadOptions, bounds, load_csv, resample};
use gpui::{Bounds, Context, EventEmitter, Pixels, SharedString, Subscription, Window};
use time::Duration;
use time::macros::format_description;

use super::super::ChartMeta;
use crate::data::{
    symbols::{SymbolMeta, load_symbols},
    universe::{SymbolSearchEntry, load_universe},
};
use crate::perf::{PerfSpec, generate_perf_candles, parse_perf_source, perf_label};
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

#[derive(Clone)]
struct LoadResult {
    symbol: String,
    base: Arc<[Candle]>,
    resamples: Vec<(Option<Interval>, Arc<[Candle]>)>,
}

#[derive(Clone)]
enum LoadMsg {
    Start {
        symbol: String,
        add_to_watchlist: bool,
    },
    Finished {
        load_id: u64,
        add_to_watchlist: bool,
        result: Result<LoadResult, String>,
    },
}

struct PersistSnapshot {
    store: Option<Arc<Mutex<DuckDbStore>>>,
    source: String,
    interval_label: String,
    range_index: usize,
    view_offset: f32,
    zoom: f32,
    watchlist: Vec<String>,
}

fn collect_resample_intervals(
    interval_at_start: Option<Interval>,
    cache: &[(Option<Interval>, Arc<[Candle]>)],
) -> Vec<Option<Interval>> {
    let mut seen = HashSet::new();
    let mut intervals = Vec::new();

    if seen.insert(None) {
        intervals.push(None);
    }

    if let Some(interval) = interval_at_start {
        if seen.insert(Some(interval)) {
            intervals.push(Some(interval));
        }
    }

    for (interval, _) in cache {
        if seen.insert(*interval) {
            intervals.push(*interval);
        }
    }

    intervals
}

fn build_resamples(
    base: &Arc<[Candle]>,
    intervals: &[Option<Interval>],
) -> Vec<(Option<Interval>, Arc<[Candle]>)> {
    let mut seen = HashSet::new();
    let mut resamples = Vec::new();

    for interval in intervals {
        if !seen.insert(*interval) {
            continue;
        }
        match interval {
            Some(interval) => {
                resamples.push((Some(*interval), Arc::from(resample(base, *interval))))
            }
            None => resamples.push((None, base.clone())),
        }
    }

    resamples
}

fn normalize_resamples(
    base: &Arc<[Candle]>,
    resamples: Vec<(Option<Interval>, Arc<[Candle]>)>,
) -> Vec<(Option<Interval>, Arc<[Candle]>)> {
    let mut seen = HashSet::new();
    let mut cache = Vec::new();

    for (interval, arc) in resamples {
        if seen.insert(interval) {
            cache.push((interval, arc));
        }
    }

    if seen.insert(None) {
        cache.push((None, base.clone()));
    }

    cache
}

pub struct ChartView {
    pub(super) base_candles: Arc<[Candle]>,
    pub(super) candles: Arc<[Candle]>,
    pub(super) price_min: f64,
    pub(super) price_max: f64,
    interval: Option<Interval>,
    pub(super) source: String,
    pub(super) settings_open: bool,
    pub(super) perf_mode: bool,
    pub(super) perf_n: usize,
    pub(super) perf_step_secs: i64,
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
    force_symbol_reload: bool,
    active_range_index: usize,
    replay_mode: bool,
    pub loading_symbol: Option<String>,
    pub load_error: Option<String>,
    pub store: Option<Arc<Mutex<DuckDbStore>>>,
    pub watchlist: Vec<String>,
    load_events: Option<Subscription>,
    pub active_load_seq: u64,
    hydrated: bool,
    symbols: HashMap<String, SymbolMeta>,
    symbol_search_filter: String,
    universe: Vec<SymbolSearchEntry>,
    resample_cache: Vec<(Option<Interval>, Arc<[Candle]>)>,
    render_cache_revision: u64,
    render_cache: Option<RenderCache>,
    time_axis_cache: Option<TimeAxisCache>,
}

pub(super) struct RenderCache {
    pub(super) revision: u64,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) columns: usize,
    pub(super) aggregated: Arc<[crate::chart::aggregation::AggregatedCandle]>,
    pub(super) padded_min: f64,
    pub(super) padded_max: f64,
    pub(super) max_volume: f64,
}

struct TimeAxisCache {
    revision: u64,
    start: usize,
    end: usize,
    start_label: String,
    mid_label: String,
    end_label: String,
}

impl ChartView {
    pub(crate) fn new(
        base_candles: Vec<Candle>,
        meta: ChartMeta,
        store: Option<Arc<Mutex<DuckDbStore>>>,
    ) -> Self {
        let base_arc: Arc<[Candle]> = base_candles.into();
        let (candles, interval) = match meta.initial_interval {
            Some(i) => (Arc::from(resample(&base_arc, i)), Some(i)),
            None => (base_arc.clone(), None),
        };
        let (price_min, price_max) = padded_bounds(&candles);
        let perf_from_source = parse_perf_source(&meta.source);
        Self {
            base_candles: base_arc.clone(),
            candles,
            price_min,
            price_max,
            interval,
            source: meta.source,
            settings_open: false,
            perf_mode: perf_from_source.is_some(),
            perf_n: perf_from_source.map(|s| s.n).unwrap_or(200_000),
            perf_step_secs: perf_from_source.map(|s| s.step_secs).unwrap_or(60),
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
            force_symbol_reload: false,
            active_range_index: QUICK_RANGE_WINDOWS.len().saturating_sub(1),
            replay_mode: false,
            loading_symbol: None,
            load_error: None,
            store,
            watchlist: Vec::new(),
            load_events: None,
            active_load_seq: 0,
            hydrated: false,
            symbols: HashMap::new(),
            symbol_search_filter: "All".to_string(),
            universe: Vec::new(),
            resample_cache: vec![(None, base_arc)],
            render_cache_revision: 0,
            render_cache: None,
            time_axis_cache: None,
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

    pub(super) fn is_perf_mode(&self) -> bool {
        self.perf_mode || self.source.starts_with("__PERF__")
    }

    fn perf_step_secs(&self) -> i64 {
        if self.is_perf_mode() {
            self.perf_step_secs.max(1)
        } else if let Some(spec) = parse_perf_source(&self.source) {
            spec.step_secs.max(1)
        } else {
            60
        }
    }

    pub(crate) fn start_perf_preset_load(
        &mut self,
        n: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let step_secs = self.perf_step_secs();
        self.perf_mode = true;
        self.perf_n = n.max(1);
        self.perf_step_secs = step_secs.max(1);
        let _ = self.persist_session("perf_mode", "true");
        let _ = self.persist_session("perf_n", &self.perf_n.to_string());
        let _ = self.persist_session("perf_step_secs", &self.perf_step_secs.to_string());
        let load_id = self.active_load_seq.wrapping_add(1);
        self.active_load_seq = load_id;
        self.load_error = None;
        window.refresh();

        let entity = cx.entity();
        let spec = PerfSpec {
            n: self.perf_n,
            step_secs: self.perf_step_secs,
        };
        self.loading_symbol = Some(perf_label(spec));
        window
            .spawn(cx, async move |async_cx| {
                let task = async_cx
                    .background_executor()
                    .spawn(async move { generate_perf_candles(spec) });
                let candles = task.await;
                async_cx
                    .update(|window, app| {
                        entity.update(app, |this, cx| {
                            if this.active_load_seq != load_id {
                                return;
                            }
                            let keep_source = this.source.clone();
                            this.replace_data(candles, keep_source, false, false);
                            cx.notify();
                        });
                        window.refresh();
                    })
                    .ok();
            })
            .detach();
    }

    pub(super) fn toggle_settings_open(&mut self) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.interval_select_open = false;
            self.symbol_search_open = false;
        }
    }

    pub(super) fn close_settings(&mut self) {
        self.settings_open = false;
    }

    pub(crate) fn set_perf_step_secs(&mut self, step_secs: i64) {
        self.perf_step_secs = step_secs.max(1);
        let _ = self.persist_session("perf_step_secs", &self.perf_step_secs.to_string());
    }

    pub(crate) fn set_perf_n(&mut self, n: usize) {
        self.perf_n = n.max(1);
        let _ = self.persist_session("perf_n", &self.perf_n.to_string());
    }

    pub(crate) fn set_perf_mode_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.perf_mode = enabled;
        let _ = self.persist_session("perf_mode", if enabled { "true" } else { "false" });

        if enabled {
            let _ = self.persist_session("perf_n", &self.perf_n.to_string());
            let _ = self.persist_session("perf_step_secs", &self.perf_step_secs.to_string());
            self.start_perf_preset_load(self.perf_n, window, cx);
            return;
        }

        let active_source = self
            .store
            .as_ref()
            .and_then(|store| store.lock().ok())
            .and_then(|guard| guard.get_session_value("active_source").ok().flatten());
        if let Some(symbol) = active_source {
            self.start_symbol_load(symbol, false, window, cx);
        } else {
            self.source = "Search".to_string();
            self.candles = Arc::from([]);
            self.base_candles = Arc::from([]);
            self.resample_cache = vec![(None, Arc::from([]))];
            self.invalidate_render_cache();
            window.refresh();
        }
    }

    pub(crate) fn set_perf_mode_flag_only(&mut self, enabled: bool) {
        self.perf_mode = enabled;
        let _ = self.persist_session("perf_mode", if enabled { "true" } else { "false" });
    }

    pub(crate) fn begin_external_loading(&mut self, label: String) -> u64 {
        let load_id = self.active_load_seq.wrapping_add(1);
        self.active_load_seq = load_id;
        self.loading_symbol = Some(label);
        self.load_error = None;
        load_id
    }

    pub(crate) fn apply_external_loaded(
        &mut self,
        load_id: u64,
        candles: Vec<Candle>,
        source: String,
    ) {
        if self.active_load_seq != load_id {
            return;
        }
        self.loading_symbol = None;
        self.load_error = None;
        if let Some(spec) = parse_perf_source(&source) {
            self.perf_mode = true;
            self.perf_n = spec.n;
            self.perf_step_secs = spec.step_secs;
            let keep_source = self.source.clone();
            self.replace_data(candles, keep_source, false, false);
        } else {
            self.perf_mode = false;
            self.replace_data(candles, source, false, false);
        }
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
        if self.perf_mode && self.source == symbol {
            self.force_symbol_reload = true;
        }
        if self.perf_mode {
            self.perf_mode = false;
            let _ = self.persist_session("perf_mode", "false");
        }
        self.ensure_load_subscription(window, cx);
        cx.emit(LoadMsg::Start {
            symbol,
            add_to_watchlist,
        });
    }

    fn ensure_load_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.load_events.is_some() {
            return;
        }
        let entity = cx.entity();
        let subscription =
            cx.subscribe_in(&entity, window, |this, _, event: &LoadMsg, window, cx| {
                this.handle_load_event(event, window, cx);
            });
        self.load_events = Some(subscription);
    }

    fn handle_load_event(&mut self, event: &LoadMsg, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            LoadMsg::Start {
                symbol,
                add_to_watchlist,
            } => {
                let force_reload = self.force_symbol_reload;
                self.force_symbol_reload = false;
                let interval_at_start = self.current_interval();
                let resample_intervals =
                    collect_resample_intervals(interval_at_start, &self.resample_cache);
                self.ensure_symbol_catalog();
                if self.loading_symbol.as_deref() == Some(symbol) {
                    return;
                }
                if self.source == *symbol && !force_reload {
                    self.symbol_search_open = false;
                    window.refresh();
                    return;
                }

                let source = self.resolve_symbol_source(symbol);
                let resolved = resolve_source_path(&source);

                let load_id = self.active_load_seq.wrapping_add(1);
                self.active_load_seq = load_id;
                self.loading_symbol = Some(symbol.clone());
                self.load_error = None;
                self.symbol_search_open = false;
                window.refresh();

                let entity = cx.entity();
                let store = self.store.clone();
                let resample_intervals = resample_intervals.clone();
                let symbol_for_task = symbol.clone();
                let resolved_path = resolved.clone();
                let add_to_watchlist = *add_to_watchlist;

                window
                    .spawn(cx, async move |async_cx| {
                        let task = async_cx.background_executor().spawn(async move {
                            if let Some(store_arc) = store.as_ref() {
                                let cached = store_arc
                                    .lock()
                                    .ok()
                                    .and_then(|guard| {
                                        guard.load_candles(&symbol_for_task, None).ok()
                                    })
                                    .filter(|c| !c.is_empty());

                                if let Some(cached) = cached {
                                    let base_arc: Arc<[Candle]> = Arc::from(cached);
                                    let resamples = build_resamples(&base_arc, &resample_intervals);
                                    if let Ok(guard) = store_arc.lock() {
                                        let _ = guard
                                            .set_session_value("active_source", &symbol_for_task);
                                    }
                                    return Ok(LoadResult {
                                        symbol: symbol_for_task.clone(),
                                        base: base_arc,
                                        resamples,
                                    });
                                }
                            }

                            let candles = load_csv(&resolved_path, LoadOptions::default())
                                .map_err(|e| {
                                    format!(
                                        "failed to load {symbol_for_task} from {}: {e}",
                                        resolved_path.display()
                                    )
                                })?;

                            if candles.is_empty() {
                                Err(format!("no candles loaded for {symbol_for_task}"))
                            } else {
                                let base_arc: Arc<[Candle]> = Arc::from(candles);
                                let resamples = build_resamples(&base_arc, &resample_intervals);
                                if let Some(store_arc) = store.as_ref() {
                                    if let Ok(guard) = store_arc.lock() {
                                        guard
                                            .write_candles(&symbol_for_task, base_arc.as_ref())
                                            .map_err(|e| {
                                                format!("failed to persist {symbol_for_task}: {e}")
                                            })?;
                                        let _ = guard
                                            .set_session_value("active_source", &symbol_for_task);
                                    }
                                }
                                Ok(LoadResult {
                                    symbol: symbol_for_task.clone(),
                                    base: base_arc,
                                    resamples,
                                })
                            }
                        });

                        let mut result = task.await;
                        let bg = async_cx.background_executor().clone();
                        let desired_interval = async_cx
                            .update(|_, app| entity.update(app, |view, _| view.current_interval()))
                            .ok()
                            .flatten();

                        if let Ok(ref mut loaded) = result {
                            if let Some(interval) = desired_interval {
                                let missing = !loaded
                                    .resamples
                                    .iter()
                                    .any(|(cached, _)| *cached == Some(interval));
                                if missing {
                                    let base = loaded.base.clone();
                                    let resample_task = bg.spawn(async move {
                                        let out = Arc::from(resample(&base, interval));
                                        (Some(interval), out)
                                    });
                                    let extra = resample_task.await;
                                    loaded.resamples.push(extra);
                                }
                            }
                        }

                        async_cx
                            .update(|window, app| {
                                entity.update(app, |_, cx| {
                                    cx.emit(LoadMsg::Finished {
                                        load_id,
                                        add_to_watchlist,
                                        result: result.clone(),
                                    });
                                });
                                window.refresh();
                            })
                            .ok();
                    })
                    .detach();
            }
            LoadMsg::Finished {
                load_id,
                add_to_watchlist,
                result,
            } => {
                if *load_id != self.active_load_seq {
                    return;
                }

                let mut persist_snapshot: Option<PersistSnapshot> = None;
                match result.clone() {
                    Ok(LoadResult {
                        symbol,
                        base,
                        resamples,
                    }) => {
                        self.load_error = None;
                        self.replace_data_from_load(
                            base,
                            resamples,
                            symbol,
                            false,
                            *add_to_watchlist,
                        );
                        persist_snapshot = Some(PersistSnapshot {
                            store: self.store.clone(),
                            source: self.source.clone(),
                            interval_label: ChartView::interval_label(self.interval).to_string(),
                            range_index: self.active_range_index,
                            view_offset: self.view_offset,
                            zoom: self.zoom,
                            watchlist: self.watchlist.clone(),
                        });
                        cx.notify();
                    }
                    Err(msg) => {
                        self.load_error = Some(msg);
                    }
                }
                self.loading_symbol = None;

                if let Some(snapshot) = persist_snapshot {
                    let bg = cx.background_executor().clone();
                    bg.spawn(async move {
                        if let Some(store) = snapshot.store {
                            if let Ok(guard) = store.lock() {
                                let _ = guard.set_session_value("active_source", &snapshot.source);
                                let _ =
                                    guard.set_session_value("interval", &snapshot.interval_label);
                                let _ = guard.set_session_value(
                                    "range_index",
                                    &snapshot.range_index.to_string(),
                                );
                                let _ = guard.set_session_value(
                                    "view_offset",
                                    &snapshot.view_offset.to_string(),
                                );
                                let _ = guard.set_session_value("zoom", &snapshot.zoom.to_string());
                                let _ = guard.set_watchlist(&snapshot.watchlist);
                            }
                        }
                    })
                    .detach();
                }
                window.refresh();
            }
        }
    }

    pub fn hydrate_from_store(&mut self) {
        if self.hydrated {
            return;
        }
        if let Some(store_arc) = self.store.clone() {
            let session = store_arc
                .lock()
                .ok()
                .and_then(|s| s.load_user_session().ok())
                .unwrap_or_default();

            self.watchlist = session.watchlist;
            if let Some(perf_mode) = session.perf_mode {
                self.perf_mode = perf_mode;
            }
            if let Some(perf_n) = session.perf_n {
                self.perf_n = perf_n.max(1);
            }
            if let Some(step) = session.perf_step_secs {
                self.perf_step_secs = step.max(1);
            }

            if session.perf_n.is_none() {
                let _ = self.persist_session("perf_n", &self.perf_n.to_string());
            }
            if session.perf_step_secs.is_none() {
                let _ = self.persist_session("perf_step_secs", &self.perf_step_secs.to_string());
            }

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
        if self.add_to_watchlist_local(symbol.clone()) {
            if let Some(store) = &self.store
                && let Ok(guard) = store.lock()
            {
                let _ = guard.set_watchlist(&self.watchlist);
            }
        }
    }

    fn add_to_watchlist_local(&mut self, symbol: String) -> bool {
        if self.watchlist.iter().any(|s| s == &symbol) {
            return false;
        }
        self.watchlist.push(symbol);
        true
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

    fn invalidate_render_cache(&mut self) {
        self.render_cache_revision = self.render_cache_revision.wrapping_add(1);
        self.render_cache = None;
        self.time_axis_cache = None;
    }

    pub(super) fn time_axis_labels(
        &mut self,
        start: usize,
        end: usize,
    ) -> (String, String, String) {
        if self.candles.is_empty() {
            self.time_axis_cache = None;
            return ("---".into(), "---".into(), "---".into());
        }

        let end = end.min(self.candles.len());
        let start = start.min(end);

        let needs_rebuild = match self.time_axis_cache.as_ref() {
            Some(cache) => {
                cache.revision != self.render_cache_revision
                    || cache.start != start
                    || cache.end != end
            }
            None => true,
        };
        if needs_rebuild {
            let visible = if start < end {
                &self.candles[start..end]
            } else {
                &self.candles[..]
            };

            let time_fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
            let start_label = visible
                .first()
                .map(|c| {
                    c.timestamp
                        .format(&time_fmt)
                        .unwrap_or_else(|_| c.timestamp.to_string())
                })
                .unwrap_or_else(|| "---".into());
            let mid_label = visible
                .get(visible.len().saturating_sub(1) / 2)
                .map(|c| {
                    c.timestamp
                        .format(&time_fmt)
                        .unwrap_or_else(|_| c.timestamp.to_string())
                })
                .unwrap_or_else(|| "---".into());
            let end_label = visible
                .last()
                .map(|c| {
                    c.timestamp
                        .format(&time_fmt)
                        .unwrap_or_else(|_| c.timestamp.to_string())
                })
                .unwrap_or_else(|| "---".into());

            self.time_axis_cache = Some(TimeAxisCache {
                revision: self.render_cache_revision,
                start,
                end,
                start_label,
                mid_label,
                end_label,
            });
        }

        let cache = self.time_axis_cache.as_ref();
        match cache {
            Some(cache) => (
                cache.start_label.clone(),
                cache.mid_label.clone(),
                cache.end_label.clone(),
            ),
            None => ("---".into(), "---".into(), "---".into()),
        }
    }

    pub(super) fn render_cache(
        &mut self,
        start: usize,
        end: usize,
        columns: usize,
    ) -> Option<&RenderCache> {
        if columns == 0 || start >= end || self.candles.is_empty() {
            self.render_cache = None;
            return None;
        }

        let end = end.min(self.candles.len());
        let start = start.min(end);
        let candle_count = end.saturating_sub(start);
        if candle_count == 0 || candle_count <= columns {
            self.render_cache = None;
            return None;
        }

        let needs_rebuild = match self.render_cache.as_ref() {
            Some(cache) => {
                cache.revision != self.render_cache_revision
                    || cache.start != start
                    || cache.end != end
                    || cache.columns != columns
            }
            None => true,
        };
        if needs_rebuild {
            let visible = &self.candles[start..end];
            let mut aggregated = Vec::with_capacity(columns);
            let mut min_low = f64::INFINITY;
            let mut max_high = f64::NEG_INFINITY;
            let mut max_volume = 0.0_f64;

            for col in 0..columns {
                let g_start = col * candle_count / columns;
                let g_end = ((col + 1) * candle_count / columns).max(g_start + 1);
                let group = &visible[g_start..g_end];
                if let Some(agg) = crate::chart::aggregation::AggregatedCandle::from_slice(group) {
                    min_low = min_low.min(agg.low);
                    max_high = max_high.max(agg.high);
                    max_volume = max_volume.max(agg.volume);
                    aggregated.push(agg);
                }
            }

            if aggregated.is_empty() {
                self.render_cache = None;
                return None;
            }

            let (padded_min, padded_max) = padded_bounds_from_min_max(min_low, max_high);
            self.render_cache = Some(RenderCache {
                revision: self.render_cache_revision,
                start,
                end,
                columns,
                aggregated: Arc::from(aggregated),
                padded_min,
                padded_max,
                max_volume,
            });
        }

        self.render_cache.as_ref()
    }

    pub(super) fn apply_interval(&mut self, interval: Option<Interval>, persist: bool) {
        self.interval = interval;
        self.candles = self.resampled_for(interval);
        self.invalidate_render_cache();
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
        self.replace_data_precomputed(base, None, source, persist_session, add_to_watchlist);
    }

    pub(crate) fn replace_data_from_load(
        &mut self,
        base: Arc<[Candle]>,
        resamples: Vec<(Option<Interval>, Arc<[Candle]>)>,
        source: String,
        persist_session: bool,
        add_to_watchlist: bool,
    ) {
        let cache = normalize_resamples(&base, resamples);
        let interval = self.interval;
        self.base_candles = base.clone();
        self.resample_cache = cache;
        let next_candles = self
            .resample_cache
            .iter()
            .find(|(cached_interval, _)| *cached_interval == interval)
            .map(|(_, arc)| arc.clone())
            .unwrap_or_else(|| self.resampled_for(interval));

        self.candles = next_candles;
        self.interval = interval;
        self.invalidate_render_cache();
        self.apply_range_index(self.active_range_index, persist_session);

        self.source = source;
        self.load_error = None;
        self.loading_symbol = None;

        if add_to_watchlist {
            if persist_session {
                self.add_to_watchlist(self.source.clone());
            } else {
                self.add_to_watchlist_local(self.source.clone());
            }
        }
        if persist_session {
            let _ = self.persist_session("active_source", &self.source);
            let _ = self.persist_session("interval", &Self::interval_label(self.interval));
        }
    }

    pub(crate) fn replace_data_precomputed(
        &mut self,
        base: Vec<Candle>,
        pre_resampled: Option<(Option<Interval>, Arc<[Candle]>)>,
        source: String,
        persist_session: bool,
        add_to_watchlist: bool,
    ) {
        let base_arc: Arc<[Candle]> = base.into();
        let mut resamples = vec![(None, base_arc.clone())];
        if let Some(pre) = pre_resampled {
            resamples.push(pre);
        }
        self.replace_data_from_load(
            base_arc,
            resamples,
            source,
            persist_session,
            add_to_watchlist,
        );
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
            let guard = store.lock().map_err(|_| ())?;
            guard.set_session_value(key, value).map_err(|_| ())?;
        }
        Ok(())
    }

    pub(super) fn persist_viewport(&self) -> Result<(), ()> {
        if let Some(store) = &self.store {
            let guard = store.lock().map_err(|_| ())?;
            guard
                .set_session_value("view_offset", &self.view_offset.to_string())
                .map_err(|_| ())?;
            guard
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
        if len_before != self.watchlist.len()
            && let Some(store) = &self.store
            && let Ok(guard) = store.lock()
        {
            let _ = guard.set_watchlist(&self.watchlist);
        }
    }
}

impl EventEmitter<LoadMsg> for ChartView {}

pub fn padded_bounds(candles: &[Candle]) -> (f64, f64) {
    let (min, mut max) = bounds(candles).unwrap_or((0.0, 1.0));
    if min == max {
        max = min + 1.0;
    }
    let pad = ((max - min) * 0.01).max(0.0);
    (min - pad, max + pad)
}

fn padded_bounds_from_min_max(min: f64, max: f64) -> (f64, f64) {
    let mut min = if min.is_finite() { min } else { 0.0 };
    let mut max = if max.is_finite() { max } else { 1.0 };
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
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
