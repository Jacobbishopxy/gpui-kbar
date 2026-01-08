use anyhow::Context;
use gpui::{
    App, AsyncWindowContext, Bounds, Context as GpuiContext, MouseButton, Render, SharedString,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use polars::prelude::*;
use std::{fs, fs::File, path::PathBuf, time::Instant};
use ui::application_with_assets;
use ui::components::loading_sand::loading_sand;

const WINDOW_WIDTH: f32 = 560.0;
const WINDOW_HEIGHT: f32 = 360.0;
const USE_PUMP: bool = true;
const USE_TIMER_REFRESH: bool = true;

struct DevView {
    loading: bool,
    last_result: SharedString,
    pump_active: bool,
}

impl DevView {
    fn new() -> Self {
        Self {
            loading: false,
            last_result: SharedString::from("Idle"),
            pump_active: false,
        }
    }

    fn start_job(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.last_result = SharedString::from("Running heavy CSV + CPU task...");
        if USE_PUMP && !self.pump_active {
            self.pump_active = true;
            start_loading_pump(window, cx.entity());
        }
        if USE_TIMER_REFRESH && !self.pump_active {
            start_loading_timer(window, cx, cx.entity());
        }
        window.refresh();

        let entity = cx.entity();
        window
            .spawn(cx, move |async_cx: &mut AsyncWindowContext| {
                let mut owned_cx = async_cx.clone();
                let task = owned_cx
                    .background_executor()
                    .clone()
                    .spawn(async move { run_heavy_job().map_err(|e| e.to_string()) });
                let entity = entity.clone();
                async move {
                    let result: Result<String, String> = task.await;
                    let _ = owned_cx.update(|window: &mut Window, app: &mut App| {
                        entity.update(app, |view, _| {
                            view.loading = false;
                            view.last_result = SharedString::from(match &result {
                                Ok(summary) => summary.clone(),
                                Err(err) => format!("Error: {err}"),
                            });
                            view.pump_active = false;
                        });
                        window.refresh();
                    });
                }
            })
            .detach();
    }
}

impl Render for DevView {
    fn render(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let on_click = cx.listener(|view: &mut DevView, _, window, cx| {
            view.start_job(window, cx);
        });

        if self.loading {
            window.request_animation_frame();
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .p_4()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x2563eb))
                            .text_sm()
                            .text_color(gpui::white())
                            .on_mouse_down(MouseButton::Left, on_click)
                            .child("Start heavy load"),
                    )
                    .child(if self.loading {
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(32.))
                                    .h(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(loading_sand(28.0, rgb(0xf59e0b))),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child("Loading..."),
                            )
                    } else {
                        div().text_sm().text_color(rgb(0x9ca3af)).child("Idle")
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .rounded_md()
                    .bg(rgb(0x111827))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .p_3()
                    .text_sm()
                    .text_color(gpui::white())
                    .child(self.last_result.clone()),
            )
    }
}

fn run_heavy_job() -> Result<String, anyhow::Error> {
    let start = Instant::now();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest.join("../data/candles");
    let mut total_rows = 0usize;
    let mut files_processed = 0usize;
    let mut accum = 0f64;

    for entry in fs::read_dir(&data_dir).context("reading data/candles")? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }
        let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
        let df = CsvReader::new(file).finish().context("csv read")?;
        total_rows += df.height();
        files_processed += 1;

        if let Ok(close) = df.column("close") {
            let close = close.f64().context("cast close")?;
            let vals: Vec<f64> = close.into_no_null_iter().collect();
            let mut local: f64 = 0.0;
            for _ in 0..50 {
                for v in vals.iter() {
                    local += v.sin().cos();
                }
            }
            accum += local;
        }
    }

    let elapsed = start.elapsed();
    Ok(format!(
        "Processed {files_processed} files, {total_rows} rows, accum checksum {:.4}, elapsed {:?}",
        accum, elapsed
    ))
}

fn start_loading_pump(window: &mut Window, entity: gpui::Entity<DevView>) {
    window.on_next_frame(move |window, app| {
        let mut still_loading = false;
        entity.update(app, |view, _| {
            still_loading = view.loading;
        });
        if still_loading {
            window.refresh();
            start_loading_pump(window, entity.clone());
        } else {
            entity.update(app, |view, _| view.pump_active = false);
        }
    });
}

fn start_loading_timer(
    window: &mut Window,
    cx: &mut GpuiContext<DevView>,
    entity: gpui::Entity<DevView>,
) {
    window
        .spawn(cx, move |async_cx: &mut AsyncWindowContext| {
            let mut owned_cx = async_cx.clone();
            let entity = entity.clone();
            async move {
                loop {
                    owned_cx
                        .background_executor()
                        .clone()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let keep_going = owned_cx
                        .update(|window: &mut Window, app: &mut App| {
                            let still_loading = entity.update(app, |view, _| view.loading);
                            if still_loading {
                                window.refresh();
                            }
                            still_loading
                        })
                        .unwrap_or(false);
                    if !keep_going {
                        break;
                    }
                }
            }
        })
        .detach();
}

fn main() {
    application_with_assets().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| DevView::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
