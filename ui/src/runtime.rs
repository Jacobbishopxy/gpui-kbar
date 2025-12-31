use std::path::{Path, PathBuf};

use core::{Candle, LoadOptions, load_csv, load_parquet};
use gpui::{
    App, Application, Bounds, Context, MouseButton, MouseDownEvent, PathPromptOptions, Render,
    SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use std::{cell::RefCell, rc::Rc};

use crate::store::default_store;
use crate::{ChartMeta, ChartView};

#[derive(Copy, Clone, Debug)]
enum RuntimeInputFormat {
    Csv,
    Parquet,
}

fn detect_format(path: &Path) -> Option<RuntimeInputFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "csv" => Some(RuntimeInputFormat::Csv),
        "parquet" | "parq" => Some(RuntimeInputFormat::Parquet),
        _ => None,
    }
}

pub fn launch_runtime() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(RuntimeView::new),
        )
        .expect("failed to open runtime picker window");
        cx.activate(true);
    });
}

struct RuntimeView {
    chart: gpui::Entity<ChartView>,
    picker_open: bool,
    loading: bool,
    error: Option<String>,
    source: Option<String>,
    store: Option<Rc<RefCell<core::DuckDbStore>>>,
    restored: bool,
}

impl RuntimeView {
    fn new(cx: &mut Context<Self>) -> Self {
        let store = default_store();
        let store_rc = store.map(|s| Rc::new(RefCell::new(s)));

        let chart = cx.new(|_| {
            ChartView::new(
                Vec::<Candle>::new(),
                ChartMeta {
                    source: "Select data".into(),
                    initial_interval: None,
                },
                store_rc.clone(),
            )
        });
        Self {
            chart,
            picker_open: false,
            loading: false,
            error: None,
            source: None,
            store: store_rc,
            restored: false,
        }
    }

    fn start_file_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from("Select OHLCV CSV or Parquet")),
        });
        let view = cx.entity();
        self.loading = true;
        self.error = None;
        window.refresh();

        window
            .spawn(cx, async move |async_cx| {
                let selection = receiver.await;
                async_cx
                    .update(|window, app| {
                        let _ = view.update(app, |view, cx| {
                            view.loading = false;
                            match selection {
                                Ok(Ok(Some(mut paths))) => {
                                    if let Some(path) = paths.pop() {
                                        view.begin_load_path(path, window, cx);
                                    } else {
                                        window.refresh();
                                    }
                                }
                                Ok(Ok(None)) => window.refresh(),
                                Ok(Err(err)) => {
                                    view.error = Some(format!("Failed to open file picker: {err}"));
                                    window.refresh();
                                }
                                Err(_) => {
                                    view.error = Some("File picker cancelled".to_string());
                                    window.refresh();
                                }
                            }
                        });
                    })
                    .ok();
            })
            .detach();
    }

    fn begin_load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.picker_open = false;
        self.error = None;
        window.refresh();

        if let Some(store) = self.store.as_ref() {
            let cached = store
                .borrow()
                .load_candles(&path.display().to_string(), None)
                .ok()
                .filter(|c| !c.is_empty());
            if let Some(candles) = cached {
                self.loading = false;
                self.apply_loaded(path.display().to_string(), candles, window, cx);
                return;
            }
        }

        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move {
                let format = detect_format(&path).ok_or_else(|| {
                    "could not determine file format (csv or parquet)".to_string()
                })?;

                let candles = match format {
                    RuntimeInputFormat::Csv => load_csv(&path, LoadOptions::default()),
                    RuntimeInputFormat::Parquet => load_parquet(&path, LoadOptions::default()),
                }
                .map_err(|e| format!("failed to load {}: {e}", path.display()))?;

                if candles.is_empty() {
                    return Err(format!("no candles loaded from {}", path.display()));
                }

                Ok((path.display().to_string(), candles))
            }
        });

        let view = cx.entity();

        window
            .spawn(cx, async move |async_cx| {
                let result = task.await;
                async_cx
                    .update(|window, app| {
                        let _ = view.update(app, |view, cx| {
                            view.loading = false;
                            match result {
                                Ok((source, candles)) => {
                                    view.apply_loaded(source, candles, window, cx);
                                }
                                Err(msg) => {
                                    view.error = Some(msg);
                                    window.refresh();
                                }
                            }
                        });
                    })
                    .ok();
            })
            .detach();
    }

    fn apply_loaded(
        &mut self,
        source: String,
        candles: Vec<Candle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.source = Some(source.clone());
        self.error = None;
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
        let _ = self.chart.update(cx, |chart, cx| {
            chart.replace_data(candles, source);
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

        let active = store
            .borrow()
            .get_session_value("active_source")
            .ok()
            .flatten();
        let cached = active.as_deref().and_then(|source| {
            store
                .borrow()
                .load_candles(source, None)
                .ok()
                .filter(|c| !c.is_empty())
                .map(|candles| (source.to_string(), candles))
        });

        if let Some((source, candles)) = cached {
            self.apply_loaded(source, candles, window, cx);
        }
    }
}

impl Render for RuntimeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.restore_session(_window, cx);

        let toggle_picker = cx.listener(|this: &mut Self, _: &MouseDownEvent, window, _| {
            this.picker_open = !this.picker_open;
            window.refresh();
        });

        let open_file = cx.listener(|this: &mut Self, _: &MouseDownEvent, window, cx| {
            this.picker_open = false;
            this.start_file_prompt(window, cx);
        });

        let status_text = if self.loading {
            "Loading..."
        } else if let Some(src) = &self.source {
            src.as_str()
        } else {
            "No data selected"
        };

        let status = div()
            .text_sm()
            .text_color(rgb(0x9ca3af))
            .child(status_text.to_string());

        let error = self
            .error
            .as_ref()
            .map(|msg| div().text_sm().text_color(rgb(0xef4444)).child(msg.clone()));

        let picker_panel = if self.picker_open {
            Some(
                div()
                    .absolute()
                    .top(px(64.))
                    .left(px(12.))
                    .w(px(220.))
                    .bg(rgb(0x111827))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .rounded_md()
                    .shadow_md()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .bg(rgb(0x1f2937))
                            .text_sm()
                            .text_color(gpui::white())
                            .on_mouse_down(MouseButton::Left, open_file)
                            .child("Browse local file"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .bg(rgb(0x0f172a))
                            .text_sm()
                            .text_color(rgb(0x9ca3af))
                            .opacity(0.6)
                            .child("TCP (coming soon)"),
                    ),
            )
        } else {
            None
        };

        let sidebar = div()
            .w(px(240.))
            .h_full()
            .bg(rgb(0x0f172a))
            .border_1()
            .border_color(rgb(0x1f2937))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_lg().text_color(gpui::white()).child("Data"))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x1f2937))
                    .text_sm()
                    .text_color(gpui::white())
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, toggle_picker)
                    .child("Select data")
                    .child(if self.picker_open { "^" } else { "v" }),
            )
            .child(status);

        let sidebar = if let Some(err) = error {
            sidebar.child(err)
        } else {
            sidebar
        };

        let chart_area = div()
            .flex_1()
            .h_full()
            .bg(rgb(0x0b1220))
            .child(self.chart.clone());

        let mut root = div()
            .flex()
            .w_full()
            .h_full()
            .relative()
            .bg(rgb(0x0b1220))
            .child(sidebar)
            .child(chart_area);

        if let Some(panel) = picker_panel {
            root = root.child(panel);
        }

        root
    }
}
