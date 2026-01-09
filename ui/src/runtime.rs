use core::Candle;
use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use std::sync::{Arc, Mutex};

use crate::perf::{PerfSpec, generate_perf_candles, perf_label, perf_source};
use crate::store::default_store;
use crate::{ChartMeta, ChartView, application_with_assets};

#[derive(Clone, Default)]
pub struct RuntimeOptions {
    pub initial_symbol: Option<String>,
    pub perf: Option<PerfOptions>,
}

#[derive(Clone)]
pub struct PerfOptions {
    pub n: usize,
    pub step_secs: i64,
}

pub fn launch_runtime() {
    launch_runtime_with_options(RuntimeOptions::default());
}

pub fn launch_runtime_with_options(options: RuntimeOptions) {
    application_with_assets().run(move |cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        let options = options.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            move |_, cx| {
                let options = options.clone();
                cx.new(|cx| RuntimeView::new_with_options(options, cx))
            },
        )
        .expect("failed to open runtime window");
        cx.activate(true);
    });
}

struct RuntimeView {
    chart: gpui::Entity<ChartView>,
    store: Option<Arc<Mutex<core::DuckDbStore>>>,
    restored: bool,
    options: RuntimeOptions,
}

impl RuntimeView {
    fn new_with_options(options: RuntimeOptions, cx: &mut Context<Self>) -> Self {
        let store = default_store();
        let store_arc = store;

        let default_source = options
            .initial_symbol
            .clone()
            .unwrap_or_else(|| "AAPL".to_string());
        let chart = cx.new(|_| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: default_source,
                    initial_interval: None,
                },
                store_arc.clone(),
            )
        });
        Self {
            chart,
            store: store_arc,
            restored: false,
            options,
        }
    }

    fn apply_loaded(
        &mut self,
        source: String,
        candles: Vec<Candle>,
        window: &mut Window,
        cx: &mut Context<Self>,
        persist_session: bool,
    ) {
        if persist_session
            && let Some(store) = &self.store
            && let Ok(guard) = store.lock()
        {
            let _ = guard.write_candles(&source, &candles);
            let _ = guard.set_session_value("active_source", &source);
            let interval = self.chart.update(cx, |chart, _| {
                ChartView::interval_label(chart.current_interval())
            });
            let _ = guard.set_session_value("interval", interval.as_str());
            let range = self
                .chart
                .update(cx, |chart, _| chart.current_range_index().to_string());
            let _ = guard.set_session_value("range_index", &range);
            let replay = self.chart.update(cx, |chart, _| chart.replay_enabled());
            let _ = guard.set_session_value("replay_mode", if replay { "true" } else { "false" });
        }
        self.chart.update(cx, |chart, cx| {
            chart.replace_data(candles, source, persist_session, persist_session);
            cx.notify();
        });
        window.refresh();
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.restored {
            return;
        }
        self.restored = true;
        let Some(store) = &self.store else {
            return;
        };

        let session = store.lock().ok().and_then(|s| s.load_user_session().ok());
        let perf_override = self.options.perf.clone();
        let perf_from_session = session.as_ref().and_then(|s| {
            if s.perf_mode.unwrap_or(false) {
                Some(PerfOptions {
                    n: s.perf_n.unwrap_or(200_000),
                    step_secs: s.perf_step_secs.unwrap_or(60),
                })
            } else {
                None
            }
        });
        let perf = perf_override.or(perf_from_session);

        if let Some(perf) = perf {
            let n = perf.n.max(1);
            let step_secs = perf.step_secs.max(1);
            let spec = PerfSpec { n, step_secs };
            let load_id = self.chart.update(cx, |chart, _| {
                chart.set_perf_n(n);
                chart.set_perf_step_secs(step_secs);
                chart.set_perf_mode_flag_only(true);
                chart.begin_external_loading(perf_label(spec))
            });

            let chart_entity = self.chart.clone();
            window
                .spawn(cx, async move |async_cx| {
                    let task = async_cx
                        .background_executor()
                        .spawn(async move { generate_perf_candles(spec) });
                    let candles = task.await;
                    let source = perf_source(spec);
                    async_cx
                        .update(|window, app| {
                            chart_entity.update(app, |chart, cx| {
                                chart.apply_external_loaded(load_id, candles, source);
                                cx.notify();
                            });
                            window.refresh();
                        })
                        .ok();
                })
                .detach();
            return;
        }

        let cached = session.as_ref().and_then(|session| {
            session
                .active_source
                .as_deref()
                .filter(|source| !source.starts_with("__PERF__"))
                .and_then(|source| {
                    store.lock().ok().and_then(|s| {
                        s.load_candles(source, None)
                            .ok()
                            .filter(|c| !c.is_empty())
                            .map(|candles| (source.to_string(), candles))
                    })
                })
        });

        if let Some((source, candles)) = cached {
            // On restore, avoid overwriting persisted session settings (interval/range)
            // before the chart hydrates from the store.
            self.apply_loaded(source, candles, window, cx, false);
        }
    }
}

impl Render for RuntimeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.restore_session(_window, cx);
        // Hydrate per-view state from store once per render cycle if not loaded yet.
        self.chart.update(cx, |chart, _| chart.hydrate_from_store());

        let chart_area = div()
            .flex_1()
            .h_full()
            .bg(rgb(0x0b1220))
            .child(self.chart.clone());

        div()
            .flex()
            .w_full()
            .h_full()
            .relative()
            .bg(rgb(0x0b1220))
            .child(chart_area)
    }
}
