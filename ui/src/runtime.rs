use core::Candle;
use gpui::{App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size};
use std::{cell::RefCell, rc::Rc};

use crate::store::default_store;
use crate::{ChartMeta, ChartView, application_with_assets};

pub fn launch_runtime() {
    application_with_assets().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(RuntimeView::new),
        )
        .expect("failed to open runtime window");
        cx.activate(true);
    });
}

struct RuntimeView {
    chart: gpui::Entity<ChartView>,
    store: Option<Rc<RefCell<core::DuckDbStore>>>,
    restored: bool,
}

impl RuntimeView {
    fn new(cx: &mut Context<Self>) -> Self {
        let store = default_store();
        let store_rc = store.map(|s| Rc::new(RefCell::new(s)));

        let default_source = ChartView::default_watchlist()
            .first()
            .cloned()
            .unwrap_or_else(|| "Data".to_string());
        let chart = cx.new(|_| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: default_source,
                    initial_interval: None,
                },
                store_rc.clone(),
            )
        });
        Self {
            chart,
            store: store_rc,
            restored: false,
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
        if persist_session {
            if let Some(store) = &self.store {
                let _ = store.borrow_mut().write_candles(&source, &candles);
                let _ = store
                    .borrow_mut()
                    .set_session_value("active_source", &source);
                let interval = self.chart.update(cx, |chart, _| {
                    ChartView::interval_label(chart.current_interval())
                });
                let _ = store
                    .borrow_mut()
                    .set_session_value("interval", interval.as_str());
                let range = self
                    .chart
                    .update(cx, |chart, _| chart.current_range_index().to_string());
                let _ = store.borrow_mut().set_session_value("range_index", &range);
                let replay = self.chart.update(cx, |chart, _| chart.replay_enabled());
                let _ = store
                    .borrow_mut()
                    .set_session_value("replay_mode", if replay { "true" } else { "false" });
            }
        }
        let _ = self.chart.update(cx, |chart, cx| {
            chart.replace_data(candles, source, persist_session);
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

        let session = store.borrow().load_user_session().ok();
        let cached = session.as_ref().and_then(|session| {
            session
                .active_source
                .as_deref()
                .and_then(|source| {
                    store
                        .borrow()
                        .load_candles(source, None)
                        .ok()
                        .filter(|c| !c.is_empty())
                        .map(|candles| (source.to_string(), candles))
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
        let _ = self.chart.update(cx, |chart, _| chart.hydrate_from_store());

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
