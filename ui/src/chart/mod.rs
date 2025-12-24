use core::{Candle, Interval};
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

mod canvas;
mod header;
mod view;

use view::ChartView;

#[derive(Clone)]
pub struct ChartMeta {
    pub source: String,
    pub initial_interval: Option<Interval>,
}

pub fn launch_chart(base_candles: Vec<Candle>, meta: ChartMeta) {
    let view_meta = meta.clone();
    let initial_base = base_candles.clone();
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            move |_, cx| cx.new(|_| ChartView::new(initial_base.clone(), view_meta.clone())),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
