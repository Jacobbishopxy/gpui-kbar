use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration as StdDuration;

use gpui::{
    App, AsyncWindowContext, Bounds, Context as GpuiContext, MouseButton, Render, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use ui::application_with_assets;
use ui::components::loading_sand::loading_sand;
use ui::logging::log_loading;

const WINDOW_WIDTH: f32 = 420.0;
const WINDOW_HEIGHT: f32 = 260.0;
const USE_RAF_IN_RENDER: bool = true;
const USE_FRAME_PUMP: bool = true;
const USE_TIMER_REFRESH: bool = true;
const LOAD_DURATION_MS: u64 = 4000;

static LOAD_SEQ: AtomicU64 = AtomicU64::new(0);

struct StallReproView {
    loading: bool,
    pump_active: bool,
    load_id: u64,
    dirty_counter: u64,
}

impl StallReproView {
    fn new() -> Self {
        Self {
            loading: false,
            pump_active: false,
            load_id: 0,
            dirty_counter: 0,
        }
    }

    fn start_load(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        if self.loading {
            return;
        }
        let load_id = LOAD_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        self.load_id = load_id;
        self.loading = true;
        self.dirty_counter = 0;
        log_loading(format!(
            "[stall-repro] start load_id={} pump_active_before={}",
            load_id, self.pump_active
        ));
        if USE_FRAME_PUMP && !self.pump_active {
            self.pump_active = true;
            log_loading(format!("[stall-repro] pump start load_id={}", load_id));
            start_frame_pump(window, cx.entity());
        }
        if USE_TIMER_REFRESH && !self.pump_active {
            log_loading(format!("[stall-repro] timer start load_id={}", load_id));
            start_timer_refresh(window, cx, cx.entity(), load_id);
        }
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
                        entity.update(app, |view, _| {
                            view.loading = false;
                            view.pump_active = false;
                        });
                        log_loading(format!("[stall-repro] finish load_id={load_id}"));
                        window.refresh();
                    });
                }
            })
            .detach();
    }
}

impl Render for StallReproView {
    fn render(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let on_click = cx.listener(|view: &mut StallReproView, _, window, cx| {
            view.start_load(window, cx);
        });

        if self.loading && USE_RAF_IN_RENDER {
            log_loading(format!(
                "[stall-repro] render load_id={} raf+refresh dirty_counter={}",
                self.load_id, self.dirty_counter
            ));
            window.request_animation_frame();
            window.refresh();
        }

        let overlay = if self.loading {
            Some(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .w_full()
                    .h_full()
                    .bg(rgb(0x0b122080))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_4()
                            .py_3()
                            .bg(rgb(0x0b1220))
                            .border_1()
                            .border_color(rgb(0x1f2937))
                            .rounded_md()
                            .child(loading_sand(28.0, rgb(0xf59e0b)))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child(format!("Loading (id={})", self.load_id)),
                            ),
                    ),
            )
        } else {
            None
        };

        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .p_4()
            .gap_3()
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
                            .child("Start repro load"),
                    )
                    .child(if self.loading {
                        div()
                            .text_sm()
                            .text_color(gpui::white())
                            .child("Overlay shown; expecting animation to keep ticking")
                    } else {
                        div().text_sm().text_color(rgb(0x9ca3af)).child("Idle")
                    }),
            )
            .child(div().text_xs().text_color(rgb(0x9ca3af)).child(format!(
                "RAF: {}, pump: {}, timer: {}, duration: {}ms",
                USE_RAF_IN_RENDER, USE_FRAME_PUMP, USE_TIMER_REFRESH, LOAD_DURATION_MS
            )));

        if let Some(overlay) = overlay {
            body.relative().child(overlay)
        } else {
            body
        }
    }
}

fn start_frame_pump(window: &mut Window, entity: gpui::Entity<StallReproView>) {
    window.on_next_frame(move |window, app| {
        let mut still_loading = false;
        let mut load_id = 0;
        let mut dirty_counter = 0;
        entity.update(app, |view, _| {
            still_loading = view.loading;
            load_id = view.load_id;
            if still_loading {
                view.dirty_counter = view.dirty_counter.saturating_add(1);
                dirty_counter = view.dirty_counter;
            }
        });
        if still_loading {
            log_loading(format!(
                "[stall-repro] pump tick load_id={} dirty_counter={}",
                load_id, dirty_counter
            ));
            window.refresh();
            start_frame_pump(window, entity.clone());
        } else {
            entity.update(app, |view, _| view.pump_active = false);
        }
    });
}

fn start_timer_refresh(
    window: &mut Window,
    cx: &mut GpuiContext<StallReproView>,
    entity: gpui::Entity<StallReproView>,
    load_id: u64,
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
                        .timer(StdDuration::from_millis(16))
                        .await;
                    let keep_going = owned_cx
                        .update(|window: &mut Window, app: &mut App| {
                            let still_loading = entity.update(app, |view, _| view.loading);
                            if still_loading {
                                log_loading(format!("[stall-repro] timer tick load_id={load_id}"));
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
            |_, cx| cx.new(|_| StallReproView::new()),
        )
        .expect("open window");
        cx.activate(true);
    });
}
