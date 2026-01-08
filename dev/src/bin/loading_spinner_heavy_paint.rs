use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration as StdDuration;

use gpui::{
    App, AsyncWindowContext, BorderStyle, Bounds, Canvas, Context as GpuiContext, MouseButton,
    MouseDownEvent, PathBuilder, Render, Window, WindowBounds, WindowOptions, canvas, div, point,
    prelude::*, px, quad, rgb, rgba, size, transparent_black,
};
use ui::application_with_assets;
use ui::components::loading_sand::{frame_log_count, loading_sand, reset_frame_logs};
use ui::logging::log_loading;

const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 600.0;

const LOAD_DURATION_MS: u64 = 6000;

// Adjust until the overlay spinner starts "stalling" like the main runtime.
const CANDLE_COUNT: usize = 6000;

// If true, this bin demonstrates the "fix" pattern: keep the overlay animating by
// skipping expensive painting while loading.
const SKIP_HEAVY_WHILE_LOADING: bool = false;

static LOAD_SEQ: AtomicU64 = AtomicU64::new(0);

struct HeavyPaintRepro {
    loading: bool,
    load_id: u64,
}

impl HeavyPaintRepro {
    fn new() -> Self {
        Self {
            loading: false,
            load_id: 0,
        }
    }

    fn start_load(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        if self.loading {
            return;
        }
        reset_frame_logs();
        let load_id = LOAD_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        self.load_id = load_id;
        self.loading = true;
        log_loading(format!(
            "[heavy-repro] start load_id={} frame_logs={}",
            load_id,
            frame_log_count()
        ));
        window.refresh();

        let entity = cx.entity();
        window
            .spawn(cx, move |async_cx: &mut AsyncWindowContext| {
                let mut owned_cx = async_cx.clone();
                let entity = entity.clone();
                async move {
                    owned_cx
                        .background_executor()
                        .clone()
                        .timer(StdDuration::from_millis(LOAD_DURATION_MS))
                        .await;
                    let _ = owned_cx.update(|window: &mut Window, app: &mut App| {
                        entity.update(app, |view, _| view.loading = false);
                        log_loading(format!(
                            "[heavy-repro] finish load_id={} frame_logs={}",
                            load_id,
                            frame_log_count()
                        ));
                        window.refresh();
                    });
                }
            })
            .detach();
    }
}

impl Render for HeavyPaintRepro {
    fn render(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let on_click = cx.listener(
            |view: &mut HeavyPaintRepro, _: &MouseDownEvent, window, cx| {
                view.start_load(window, cx);
            },
        );

        if self.loading {
            window.request_animation_frame();
            window.refresh();
        }

        let mut root = div().relative().w_full().h_full().bg(rgb(0x0b1220)).child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x2563eb))
                .text_sm()
                .text_color(gpui::white())
                .absolute()
                .left(px(12.))
                .top(px(12.))
                .on_mouse_down(MouseButton::Left, on_click)
                .child(format!(
                    "Start heavy load (candles={}, skip_while_loading={})",
                    CANDLE_COUNT, SKIP_HEAVY_WHILE_LOADING
                )),
        );

        let show_heavy = !self.loading || !SKIP_HEAVY_WHILE_LOADING;
        if show_heavy {
            root = root.child(heavy_chart_canvas(CANDLE_COUNT).w_full().h_full());
        } else {
            root = root.child(
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child("Heavy painting skipped while loading"),
            );
        }

        if self.loading {
            root = root.child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .w_full()
                    .h_full()
                    .bg(rgba(0x0b122080))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .bg(rgb(0x0b1220))
                            .border_1()
                            .border_color(rgb(0x1f2937))
                            .rounded_md()
                            .child(loading_sand(32.0, rgb(0xf59e0b)))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child(format!("Loading (id={})", self.load_id)),
                            ),
                    ),
            );
        }

        root
    }
}

fn heavy_chart_canvas(candle_count: usize) -> Canvas<()> {
    canvas(
        move |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(quad(
                bounds,
                px(0.),
                rgb(0x0b1220),
                px(0.),
                transparent_black(),
                BorderStyle::default(),
            ));

            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            if candle_count == 0 || height <= 0.0 || width <= 0.0 {
                return;
            }

            // gridlines (min/mid/max)
            for frac in [0.0f32, 0.5, 1.0] {
                let y = oy + height * (1.0 - frac);
                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(ox), px(y)));
                builder.line_to(point(px(ox + width), px(y)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0x1f2937));
                }
            }

            let candle_width = (width / candle_count as f32).max(f32::EPSILON);
            let body_width = (candle_width * 0.6).max(f32::EPSILON);

            for idx in 0..candle_count {
                let t = idx as f32 * 0.02;
                let base = 0.5 + 0.35 * t.sin();
                let open = (base + 0.08 * (t * 0.7).sin()).clamp(0.0, 1.0);
                let close = (base + 0.08 * (t * 0.9).cos()).clamp(0.0, 1.0);
                let high = open.max(close) + 0.07;
                let low = open.min(close) - 0.07;

                let x = ox + idx as f32 * candle_width + candle_width * 0.5;
                let open_y = oy + (1.0 - open) * height;
                let close_y = oy + (1.0 - close) * height;
                let high_y = oy + (1.0 - high.clamp(0.0, 1.0)) * height;
                let low_y = oy + (1.0 - low.clamp(0.0, 1.0)) * height;

                let body_top = open_y.min(close_y);
                let body_height = (open_y - close_y).abs().max(1.0);
                let color = if close >= open {
                    rgb(0x22c55e)
                } else {
                    rgb(0xef4444)
                };

                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(x), px(high_y)));
                builder.line_to(point(px(x), px(low_y)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0xe5e7eb));
                }

                let body_bounds = Bounds {
                    origin: point(px(x - body_width * 0.5), px(body_top)),
                    size: size(px(body_width), px(body_height)),
                };
                window.paint_quad(quad(
                    body_bounds,
                    px(2.),
                    color,
                    px(0.),
                    color,
                    BorderStyle::default(),
                ));
            }
        },
    )
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
            |_, cx| cx.new(|_| HeavyPaintRepro::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
