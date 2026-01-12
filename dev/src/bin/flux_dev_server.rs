use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use anyhow::{Context as _, Result};
use csv::ReaderBuilder;
use flux_schema::{WIRE_SCHEMA_VERSION, fb};
use gpui::{
    App, Bounds, Context as GpuiContext, MouseButton, MouseDownEvent, Render, SharedString, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_component::scroll::ScrollableElement;
use kbar_core::{DuckDbStore, StorageMode, UniverseRow};
use tokio::sync::{RwLock, watch};
use zeromq::{Socket, SocketRecv, SocketSend};

use ui::application_with_assets;
use ui::components::button_effect;

const WINDOW_WIDTH: f32 = 920.0;
const WINDOW_HEIGHT: f32 = 640.0;

fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StreamKeyOwned {
    source_id: String,
    symbol: String,
    interval: String,
}

impl StreamKeyOwned {
    fn topic(&self) -> String {
        format!(
            "candles.{}.{}.{}",
            self.source_id, self.symbol, self.interval
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct CandleWire {
    ts_ms: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug)]
struct StreamState {
    next_sequence: u64,
    next_ts_ms: i64,
    last_close: f64,
    history: Vec<CandleWire>,
}

#[derive(Debug, Clone)]
struct FaultConfig {
    drop_percent: u8,
    gap_every: u64,
    jitter_ms: u64,
}

#[derive(Debug, Clone)]
struct RunConfig {
    live_pub: String,
    chunk_rep: String,
    source_id: String,
    interval: String,
    tick_ms: u64,
    batch_size: usize,
    symbols: Vec<String>,
    fault: FaultConfig,
}

struct ServerHandle {
    shutdown_tx: watch::Sender<bool>,
    published_candles: Arc<AtomicU64>,
    stored_candles: Arc<AtomicU64>,
}

struct DevServerView {
    universe: Vec<UniverseRow>,
    selected: HashSet<String>,
    running: bool,
    status: SharedString,

    live_pub: String,
    chunk_rep: String,
    source_id: String,
    interval: String,
    tick_ms: u64,
    batch_size: usize,

    drop_percent: u8,
    gap_every: u64,
    jitter_ms: u64,

    server: Option<ServerHandle>,
}

impl DevServerView {
    fn new() -> Self {
        let universe = load_universe_rows().unwrap_or_default();
        let status = if universe.is_empty() {
            "No symbols loaded (check data/universe.csv)".to_string()
        } else {
            format!("Loaded {} symbols", universe.len())
        };
        let mut selected = HashSet::new();
        if let Some(first) = universe.first() {
            selected.insert(first.symbol.clone());
        }

        Self {
            universe,
            selected,
            running: false,
            status: SharedString::from(status),
            live_pub: "tcp://127.0.0.1:5556".to_string(),
            chunk_rep: "tcp://127.0.0.1:5557".to_string(),
            source_id: "SIM".to_string(),
            interval: "1s".to_string(),
            tick_ms: 250,
            batch_size: 50,
            drop_percent: 0,
            gap_every: 0,
            jitter_ms: 0,
            server: None,
        }
    }

    fn start(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        if self.running {
            return;
        }
        if self.selected.is_empty() {
            self.status = SharedString::from("Select at least 1 symbol.");
            window.refresh();
            return;
        }

        let cfg = RunConfig {
            live_pub: self.live_pub.clone(),
            chunk_rep: self.chunk_rep.clone(),
            source_id: self.source_id.clone(),
            interval: self.interval.clone(),
            tick_ms: self.tick_ms.max(1),
            batch_size: self.batch_size.max(1),
            symbols: self.selected.iter().cloned().collect(),
            fault: FaultConfig {
                drop_percent: self.drop_percent.min(100),
                gap_every: self.gap_every,
                jitter_ms: self.jitter_ms,
            },
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let published_candles = Arc::new(AtomicU64::new(0));
        let stored_candles = Arc::new(AtomicU64::new(0));

        let streams = Arc::new(RwLock::new(HashMap::<StreamKeyOwned, StreamState>::new()));

        let cfg_for_tasks = cfg.clone();
        let streams_for_rep = streams.clone();
        let streams_for_pub = streams.clone();
        let published_for_tasks = published_candles.clone();
        let stored_for_tasks = stored_candles.clone();

        tokio_runtime().spawn(async move {
            let _ = tokio::join!(
                rep_task(cfg_for_tasks.clone(), streams_for_rep, shutdown_rx.clone()),
                pub_task(
                    cfg_for_tasks,
                    streams_for_pub,
                    shutdown_rx,
                    published_for_tasks,
                    stored_for_tasks,
                ),
            );
        });

        self.server = Some(ServerHandle {
            shutdown_tx,
            published_candles,
            stored_candles,
        });
        self.running = true;
        self.status = SharedString::from(format!(
            "Running: PUB={} REP={} symbols={}",
            cfg.live_pub,
            cfg.chunk_rep,
            cfg.symbols.len()
        ));
        start_status_pump(window, cx.entity());
        window.refresh();
    }

    fn stop(&mut self, window: &mut Window) {
        if let Some(server) = self.server.take() {
            let _ = server.shutdown_tx.send(true);
        }
        self.running = false;
        self.status = SharedString::from("Stopped");
        window.refresh();
    }

    fn toggle_symbol(&mut self, symbol: &str) {
        if self.running {
            return;
        }
        if !self.selected.insert(symbol.to_string()) {
            self.selected.remove(symbol);
        }
    }
}

impl Render for DevServerView {
    fn render(&mut self, _window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let on_start = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, cx| {
            this.start(window, cx);
        });
        let on_stop = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            this.stop(window);
        });

        let cycle_interval =
            cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
                if this.running {
                    return;
                }
                let options = ["1s", "5s", "15s", "1m"];
                let idx = options
                    .iter()
                    .position(|v| *v == this.interval)
                    .unwrap_or(0);
                let next = options[(idx + 1) % options.len()].to_string();
                this.interval = next;
                window.refresh();
            });

        let dec_tick = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.tick_ms = this.tick_ms.saturating_sub(25).max(1);
            window.refresh();
        });
        let inc_tick = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.tick_ms = this.tick_ms.saturating_add(25).min(60_000);
            window.refresh();
        });

        let dec_batch = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.batch_size = this.batch_size.saturating_sub(10).max(1);
            window.refresh();
        });
        let inc_batch = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.batch_size = (this.batch_size.saturating_add(10)).min(10_000);
            window.refresh();
        });

        let dec_drop = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.drop_percent = this.drop_percent.saturating_sub(5);
            window.refresh();
        });
        let inc_drop = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.drop_percent = (this.drop_percent.saturating_add(5)).min(100);
            window.refresh();
        });

        let dec_gap = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.gap_every = this.gap_every.saturating_sub(10);
            window.refresh();
        });
        let inc_gap = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.gap_every = this.gap_every.saturating_add(10).min(1_000_000);
            window.refresh();
        });
        let clear_gap = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.gap_every = 0;
            window.refresh();
        });

        let dec_jitter = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.jitter_ms = this.jitter_ms.saturating_sub(10);
            window.refresh();
        });
        let inc_jitter = cx.listener(|this: &mut DevServerView, _: &MouseDownEvent, window, _| {
            if this.running {
                return;
            }
            this.jitter_ms = this.jitter_ms.saturating_add(10).min(10_000);
            window.refresh();
        });

        let (pub_count, stored_count) = self
            .server
            .as_ref()
            .map(|s| {
                (
                    s.published_candles.load(Ordering::Relaxed),
                    s.stored_candles.load(Ordering::Relaxed),
                )
            })
            .unwrap_or((0, 0));

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::white())
                            .child("Flux Dev Server"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9ca3af))
                            .child(self.status.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button_effect::apply(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x2563eb))
                            .text_sm()
                            .text_color(gpui::white())
                            .on_mouse_down(MouseButton::Left, on_start)
                            .child("Start")
                            .id("dev-server-start"),
                        0x2563eb,
                    ))
                    .child(button_effect::apply(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x111827))
                            .border_1()
                            .border_color(rgb(0x1f2937))
                            .text_sm()
                            .text_color(rgb(0xe5e7eb))
                            .on_mouse_down(MouseButton::Left, on_stop)
                            .child("Stop")
                            .id("dev-server-stop"),
                        0x111827,
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9ca3af))
                            .child(format!("published={pub_count} stored={stored_count}")),
                    ),
            );

        let cfg_row = |label: &'static str, value: String| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x111827))
                .border_1()
                .border_color(rgb(0x1f2937))
                .child(div().text_xs().text_color(rgb(0x9ca3af)).child(label))
                .child(div().text_sm().text_color(gpui::white()).child(value))
        };

        let row_shell = |label: &'static str, right: gpui::Div| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x111827))
                .border_1()
                .border_color(rgb(0x1f2937))
                .child(div().text_xs().text_color(rgb(0x9ca3af)).child(label))
                .child(right)
        };

        let interval_row = row_shell(
            "interval",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.interval.clone()),
                )
                .child(button_effect::apply(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("cycle")
                        .on_mouse_down(MouseButton::Left, cycle_interval)
                        .id("dev-server-interval-cycle"),
                    0x0f172a,
                )),
        );

        let tick_row = row_shell(
            "tick_ms",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("-")
                        .on_mouse_down(MouseButton::Left, dec_tick)
                        .id("dev-server-tick-dec"),
                    0x0f172a,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.tick_ms.to_string()),
                )
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("+")
                        .on_mouse_down(MouseButton::Left, inc_tick)
                        .id("dev-server-tick-inc"),
                    0x0f172a,
                )),
        );

        let batch_row = row_shell(
            "batch_size",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("-")
                        .on_mouse_down(MouseButton::Left, dec_batch)
                        .id("dev-server-batch-dec"),
                    0x0f172a,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.batch_size.to_string()),
                )
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("+")
                        .on_mouse_down(MouseButton::Left, inc_batch)
                        .id("dev-server-batch-inc"),
                    0x0f172a,
                )),
        );

        let drop_row = row_shell(
            "drop_%",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("-")
                        .on_mouse_down(MouseButton::Left, dec_drop)
                        .id("dev-server-drop-dec"),
                    0x0f172a,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.drop_percent.to_string()),
                )
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("+")
                        .on_mouse_down(MouseButton::Left, inc_drop)
                        .id("dev-server-drop-inc"),
                    0x0f172a,
                )),
        );

        let gap_row = row_shell(
            "gap_every",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("-")
                        .on_mouse_down(MouseButton::Left, dec_gap)
                        .id("dev-server-gap-dec"),
                    0x0f172a,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.gap_every.to_string()),
                )
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("+")
                        .on_mouse_down(MouseButton::Left, inc_gap)
                        .id("dev-server-gap-inc"),
                    0x0f172a,
                ))
                .child(button_effect::apply(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(0x111827))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .text_xs()
                        .text_color(rgb(0x9ca3af))
                        .child("clear")
                        .on_mouse_down(MouseButton::Left, clear_gap)
                        .id("dev-server-gap-clear"),
                    0x111827,
                )),
        );

        let jitter_row = row_shell(
            "jitter_ms",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("-")
                        .on_mouse_down(MouseButton::Left, dec_jitter)
                        .id("dev-server-jitter-dec"),
                    0x0f172a,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::white())
                        .child(self.jitter_ms.to_string()),
                )
                .child(button_effect::apply(
                    div()
                        .w(px(26.))
                        .h(px(24.))
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0xe5e7eb))
                        .child("+")
                        .on_mouse_down(MouseButton::Left, inc_jitter)
                        .id("dev-server-jitter-inc"),
                    0x0f172a,
                )),
        );

        let stats = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(cfg_row("PUB", self.live_pub.clone()))
            .child(cfg_row("REP", self.chunk_rep.clone()))
            .child(cfg_row("source_id", self.source_id.clone()))
            .child(interval_row)
            .child(tick_row)
            .child(batch_row)
            .child(drop_row)
            .child(gap_row)
            .child(jitter_row);

        let mut symbol_list = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .rounded_md()
            .bg(rgb(0x0b1220))
            .border_1()
            .border_color(rgb(0x1f2937))
            .id("dev-server-symbols-inner");

        for row in self.universe.iter() {
            let symbol = row.symbol.clone();
            let row_id: gpui::SharedString = format!("dev-server-symbol-{symbol}").into();
            let symbol_for_toggle = symbol.clone();
            let is_selected = self.selected.contains(&symbol);
            let bg_hex = if is_selected { 0x0f172a } else { 0x0b1220 };
            let border = if is_selected {
                rgb(0x2563eb)
            } else {
                rgb(0x1f2937)
            };

            let on_toggle = cx.listener(
                move |this: &mut DevServerView, _: &MouseDownEvent, window, _| {
                    this.toggle_symbol(&symbol_for_toggle);
                    window.refresh();
                },
            );

            symbol_list = symbol_list.child(button_effect::apply(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(bg_hex))
                    .border_b_1()
                    .border_color(border)
                    .on_mouse_down(MouseButton::Left, on_toggle)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(28.))
                                    .h(px(28.))
                                    .rounded_full()
                                    .bg(rgb(0x1f2937))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(gpui::white())
                                    .child(row.badge.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child(symbol.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x9ca3af))
                                            .child(row.name.clone()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9ca3af))
                            .child(format!("{} • {}", row.market, row.venue)),
                    )
                    .id(row_id),
                bg_hex,
            ));
        }

        let symbol_list = symbol_list.overflow_y_scrollbar();

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .p_4()
            .gap_3()
            .child(header)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w(px(320.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child("Config (read-only; restart to change)"),
                            )
                            .child(stats),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child("Symbols (click to select)"),
                            )
                            .child(symbol_list),
                    ),
            )
    }
}

fn start_status_pump(window: &mut Window, entity: gpui::Entity<DevServerView>) {
    window.on_next_frame(move |window, app| {
        let mut keep = false;
        entity.update(app, |view, _| keep = view.running);
        if keep {
            window.refresh();
            start_status_pump(window, entity.clone());
        }
    });
}

fn load_universe_rows() -> Result<Vec<UniverseRow>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_db = manifest.join("../data/config.duckdb");
    let data_db = manifest.join("../data/data.duckdb");
    let universe_csv = manifest.join("../data/universe.csv");

    // Prefer DuckDB as the source of truth, but fall back to parsing CSV if DuckDB is unavailable
    // (or if the universe table is empty).
    let store = match DuckDbStore::new_split(&config_db, &data_db, StorageMode::Disk) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("open duckdb failed: {err}");
            return load_universe_rows_from_csv(&universe_csv);
        }
    };
    if let Err(err) = store.ensure_universe_loaded(&universe_csv) {
        eprintln!("ensure_universe_loaded failed: {err}");
    }
    let rows = store.load_universe_rows().unwrap_or_default();
    if !rows.is_empty() {
        return Ok(rows);
    }
    load_universe_rows_from_csv(&universe_csv)
}

fn load_universe_rows_from_csv(path: &Path) -> Result<Vec<UniverseRow>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("open universe csv {}", path.display()))?;
    let headers = reader.headers().context("read csv headers")?.clone();

    let idx = |name: &str| -> Result<usize> {
        headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("missing column {name}"))
    };

    let filters_idx = idx("filters")?;
    let badge_idx = idx("badge")?;
    let symbol_idx = idx("symbol")?;
    let name_idx = idx("name")?;
    let market_idx = idx("market")?;
    let venue_idx = idx("venue")?;

    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.context("read universe csv record")?;
        let symbol = record.get(symbol_idx).unwrap_or("").to_string();
        if symbol.is_empty() {
            continue;
        }
        out.push(UniverseRow {
            filters: record.get(filters_idx).unwrap_or("").to_string(),
            badge: record.get(badge_idx).unwrap_or("").to_string(),
            symbol,
            name: record.get(name_idx).unwrap_or("").to_string(),
            market: record.get(market_idx).unwrap_or("").to_string(),
            venue: record.get(venue_idx).unwrap_or("").to_string(),
        });
    }

    Ok(out)
}

// ZMQ service implementation lives below (rep_task/pub_task + FlatBuffers encoding helpers).

fn encode_error(msg: &str) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let message = fbb.create_string(msg);
    let err = fb::ErrorResponse::create(
        &mut fbb,
        &fb::ErrorResponseArgs {
            code: 1,
            message: Some(message),
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::ERROR_RESPONSE,
            correlation_id: None,
            message_type: fb::Message::ErrorResponse,
            message: Some(err.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn build_stream_key<'a>(
    fbb: &mut flatbuffers::FlatBufferBuilder<'a>,
    key: &StreamKeyOwned,
) -> flatbuffers::WIPOffset<fb::StreamKey<'a>> {
    let source_id = fbb.create_string(&key.source_id);
    let symbol = fbb.create_string(&key.symbol);
    let interval = fbb.create_string(&key.interval);
    fb::StreamKey::create(
        fbb,
        &fb::StreamKeyArgs {
            source_id: Some(source_id),
            symbol: Some(symbol),
            interval: Some(interval),
        },
    )
}

fn encode_candle_batch(
    key: &StreamKeyOwned,
    start_sequence: u64,
    candles: &[CandleWire],
) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key = build_stream_key(&mut fbb, key);

    let mut candle_offsets = Vec::with_capacity(candles.len());
    for candle in candles {
        candle_offsets.push(fb::Candle::create(
            &mut fbb,
            &fb::CandleArgs {
                ts_ms: candle.ts_ms,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            },
        ));
    }
    let candle_vec = fbb.create_vector(&candle_offsets);
    let batch = fb::CandleBatch::create(
        &mut fbb,
        &fb::CandleBatchArgs {
            key: Some(key),
            start_sequence,
            candles: Some(candle_vec),
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::CANDLE_BATCH,
            correlation_id: None,
            message_type: fb::Message::CandleBatch,
            message: Some(batch.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn encode_backfill_response(
    key: &StreamKeyOwned,
    start_sequence: u64,
    candles: &[CandleWire],
    has_more: bool,
    next_sequence: u64,
) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key = build_stream_key(&mut fbb, key);

    let mut candle_offsets = Vec::with_capacity(candles.len());
    for candle in candles {
        candle_offsets.push(fb::Candle::create(
            &mut fbb,
            &fb::CandleArgs {
                ts_ms: candle.ts_ms,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            },
        ));
    }
    let candle_vec = fbb.create_vector(&candle_offsets);
    let resp = fb::BackfillCandlesResponse::create(
        &mut fbb,
        &fb::BackfillCandlesResponseArgs {
            key: Some(key),
            start_sequence,
            candles: Some(candle_vec),
            has_more,
            next_sequence,
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::BACKFILL_CANDLES_RESPONSE,
            correlation_id: None,
            message_type: fb::Message::BackfillCandlesResponse,
            message: Some(resp.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn encode_get_cursor_response(
    key: &StreamKeyOwned,
    latest_sequence: u64,
    latest_ts_ms: i64,
) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key_offset = build_stream_key(&mut fbb, key);
    let cursor = fb::Cursor::create(
        &mut fbb,
        &fb::CursorArgs {
            latest_sequence,
            latest_ts_ms,
        },
    );
    let resp = fb::GetCursorResponse::create(
        &mut fbb,
        &fb::GetCursorResponseArgs {
            key: Some(key_offset),
            cursor: Some(cursor),
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::GET_CURSOR_RESPONSE,
            correlation_id: None,
            message_type: fb::Message::GetCursorResponse,
            message: Some(resp.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn parse_interval_ms(interval: &str) -> i64 {
    if interval.len() < 2 {
        return 1_000;
    }
    let (num, unit) = interval.split_at(interval.len() - 1);
    let n: i64 = num.parse().unwrap_or(1);
    let unit_ms = match unit {
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        _ => 1_000,
    };
    n.saturating_mul(unit_ms).max(1)
}

async fn rep_task(
    cfg: RunConfig,
    streams: Arc<RwLock<HashMap<StreamKeyOwned, StreamState>>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut rep_socket = zeromq::RepSocket::new();
    if let Err(err) = rep_socket.bind(&cfg.chunk_rep).await {
        eprintln!("rep bind failed: {err}");
        return;
    }

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            req = rep_socket.recv() => {
                let req = match req {
                    Ok(req) => req,
                    Err(err) => {
                        eprintln!("rep recv error: {err}");
                        continue;
                    }
                };
                let req_bytes: Result<Vec<u8>, _> = req.try_into();
                let req_bytes = match req_bytes {
                    Ok(req_bytes) => req_bytes,
                    Err(err) => {
                        eprintln!("rep recv invalid message: {err}");
                        let _ = rep_socket.send(encode_error("invalid request").into()).await;
                        continue;
                    }
                };

                let resp = match fb::root_as_envelope(&req_bytes) {
                    Ok(env) => {
                        if env.schema_version() != WIRE_SCHEMA_VERSION {
                            encode_error("unsupported schema_version")
                        } else {
                            match env.message_type() {
                                fb::Message::BackfillCandlesRequest => handle_backfill(&cfg, &streams, env).await,
                                fb::Message::GetCursorRequest => handle_get_cursor(&cfg, &streams, env).await,
                                _ => encode_error("unsupported request message_type"),
                            }
                        }
                    }
                    Err(_) => encode_error("invalid envelope"),
                };

                if let Err(err) = rep_socket.send(resp.into()).await {
                    eprintln!("rep send error: {err}");
                }
            }
        }
    }
}

async fn handle_get_cursor(
    cfg: &RunConfig,
    streams: &Arc<RwLock<HashMap<StreamKeyOwned, StreamState>>>,
    env: fb::Envelope<'_>,
) -> Vec<u8> {
    let Some(req) = env.message_as_get_cursor_request() else {
        return encode_error("invalid GetCursorRequest body");
    };
    let Some(key) = req.key() else {
        return encode_error("missing key");
    };
    let req_source = key.source_id().unwrap_or("");
    let req_symbol = key.symbol().unwrap_or("");
    let req_interval = key.interval().unwrap_or("");
    if req_source != cfg.source_id || req_interval != cfg.interval {
        return encode_error("unknown stream key");
    }

    let owned = StreamKeyOwned {
        source_id: cfg.source_id.clone(),
        symbol: req_symbol.to_string(),
        interval: cfg.interval.clone(),
    };
    let guard = streams.read().await;
    let (latest_sequence, latest_ts_ms) = guard
        .get(&owned)
        .and_then(|s| s.history.last().map(|c| (s.history.len() as u64, c.ts_ms)))
        .unwrap_or((0, 0));
    encode_get_cursor_response(&owned, latest_sequence, latest_ts_ms)
}

async fn handle_backfill(
    cfg: &RunConfig,
    streams: &Arc<RwLock<HashMap<StreamKeyOwned, StreamState>>>,
    env: fb::Envelope<'_>,
) -> Vec<u8> {
    let Some(req) = env.message_as_backfill_candles_request() else {
        return encode_error("invalid BackfillCandlesRequest body");
    };
    let Some(key) = req.key() else {
        return encode_error("missing key");
    };
    let req_source = key.source_id().unwrap_or("");
    let req_symbol = key.symbol().unwrap_or("");
    let req_interval = key.interval().unwrap_or("");
    if req_source != cfg.source_id || req_interval != cfg.interval {
        return encode_error("unknown stream key");
    }

    let owned = StreamKeyOwned {
        source_id: cfg.source_id.clone(),
        symbol: req_symbol.to_string(),
        interval: cfg.interval.clone(),
    };

    let from_exclusive = if req.has_from_sequence() {
        req.from_sequence_exclusive()
    } else {
        0
    };
    let start_index = from_exclusive as usize;

    let end_ts_ms = if req.has_end_ts_ms() {
        Some(req.end_ts_ms())
    } else {
        None
    };

    let guard = streams.read().await;
    let Some(state) = guard.get(&owned) else {
        return encode_error("unknown symbol");
    };

    let max_index_exclusive = match end_ts_ms {
        Some(end_ts_ms) => state.history.partition_point(|c| c.ts_ms <= end_ts_ms),
        None => state.history.len(),
    }
    .max(start_index);

    let limit = req.limit() as usize;
    let end_index = if limit == 0 {
        max_index_exclusive
    } else {
        max_index_exclusive.min(start_index.saturating_add(limit.max(1)))
    };

    let slice = if start_index < state.history.len() && start_index < end_index {
        &state.history[start_index..end_index]
    } else {
        &[]
    };

    let has_more = end_index < max_index_exclusive;
    let next_sequence = if has_more { end_index as u64 } else { 0 };
    encode_backfill_response(
        &owned,
        from_exclusive.saturating_add(1),
        slice,
        has_more,
        next_sequence,
    )
}

async fn pub_task(
    cfg: RunConfig,
    streams: Arc<RwLock<HashMap<StreamKeyOwned, StreamState>>>,
    mut shutdown: watch::Receiver<bool>,
    published_candles: Arc<AtomicU64>,
    stored_candles: Arc<AtomicU64>,
) {
    let mut pub_socket = zeromq::PubSocket::new();
    if let Err(err) = pub_socket.bind(&cfg.live_pub).await {
        eprintln!("pub bind failed: {err}");
        return;
    }

    let mut rng_state: u64 = 0x726f6f742d666c75;
    let interval_ms = parse_interval_ms(&cfg.interval);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as i64;

    loop {
        let jitter = if cfg.fault.jitter_ms > 0 {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let span = cfg.fault.jitter_ms.min(cfg.tick_ms.saturating_sub(1));
            if span > 0 {
                (rng_state % (span * 2 + 1)) as i64 - (span as i64)
            } else {
                0
            }
        } else {
            0
        };
        let sleep_ms = (cfg.tick_ms as i64 + jitter).max(1) as u64;

        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(sleep_ms)) => {}
        }

        let keys: Vec<StreamKeyOwned> = cfg
            .symbols
            .iter()
            .map(|symbol| StreamKeyOwned {
                source_id: cfg.source_id.clone(),
                symbol: symbol.clone(),
                interval: cfg.interval.clone(),
            })
            .collect();

        for key in keys {
            let mut runs: Vec<(u64, Vec<CandleWire>)> = Vec::new();
            let mut run_start: Option<u64> = None;
            let mut run_candles: Vec<CandleWire> = Vec::new();

            let mut guard = streams.write().await;
            let state = guard.entry(key.clone()).or_insert_with(|| StreamState {
                next_sequence: 1,
                next_ts_ms: now_ms,
                last_close: 100.0,
                history: Vec::new(),
            });

            for _ in 0..cfg.batch_size {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let r = (rng_state >> 33) as u32;

                let open = state.last_close;
                let delta = ((r as f64 / (u32::MAX as f64)) - 0.5) * 0.8;
                let close = (open + delta).max(0.01);
                let high = open.max(close) + ((r as f64 / (u32::MAX as f64)) * 0.4);
                let low = open.min(close) - ((r as f64 / (u32::MAX as f64)) * 0.4);
                let volume = (((r as f64 / (u32::MAX as f64)) * 1500.0).max(1.0)).round();

                let candle = CandleWire {
                    ts_ms: state.next_ts_ms,
                    open,
                    high,
                    low,
                    close,
                    volume,
                };
                state.history.push(candle);
                stored_candles.fetch_add(1, Ordering::Relaxed);

                let seq = state.next_sequence;
                state.next_sequence = state.next_sequence.saturating_add(1);
                state.next_ts_ms = state.next_ts_ms.saturating_add(interval_ms);
                state.last_close = close;

                let dropped =
                    cfg.fault.drop_percent > 0 && (r % 100) < (cfg.fault.drop_percent as u32);
                let gapped = cfg.fault.gap_every > 0 && (seq % cfg.fault.gap_every == 0);

                if dropped || gapped {
                    if let Some(start) = run_start.take()
                        && !run_candles.is_empty()
                    {
                        runs.push((start, std::mem::take(&mut run_candles)));
                    }
                    continue;
                }

                if run_start.is_none() {
                    run_start = Some(seq);
                }
                run_candles.push(candle);
            }

            if let Some(start) = run_start.take()
                && !run_candles.is_empty()
            {
                runs.push((start, run_candles));
            }
            drop(guard);

            for (start_sequence, candles) in runs {
                let payload = encode_candle_batch(&key, start_sequence, &candles);
                let mut msg = zeromq::ZmqMessage::from(key.topic().as_str());
                msg.push_back(payload.into());
                let _ = pub_socket.send(msg).await;
                published_candles.fetch_add(candles.len() as u64, Ordering::Relaxed);
            }
        }
    }
}

fn main() {
    application_with_assets().run(|cx: &mut App| {
        gpui_component::theme::init(cx);

        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| DevServerView::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
