use core::{Candle, Interval};
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

mod canvas;
mod error_view;
mod footer;
mod header;
mod view;

use error_view::ErrorView;

pub use view::ChartView;

#[derive(Clone)]
pub struct ChartMeta {
    pub source: String,
    pub initial_interval: Option<Interval>,
}

pub fn launch_chart(candles: Result<Vec<Candle>, String>, meta: ChartMeta) {
    let view_meta = meta.clone();
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        match candles.clone() {
            Ok(base) => {
                let initial_base = base.clone();
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        focus: true,
                        ..Default::default()
                    },
                    move |_, cx| {
                        cx.new(|_| ChartView::new(initial_base.clone(), view_meta.clone(), None))
                    },
                )
                .expect("failed to open window");
            }
            Err(msg) => {
                let err_msg = msg.clone();
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        focus: true,
                        ..Default::default()
                    },
                    move |_, cx| {
                        cx.new(|_| ErrorView::new(view_meta.source.clone(), err_msg.clone()))
                    },
                )
                .expect("failed to open window");
            }
        }
        cx.activate(true);
    });
}
