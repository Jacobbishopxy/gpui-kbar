use core::{bounds, resample, Candle, Interval};
use gpui::{
    BorderStyle, Bounds, Context, PathBuilder, Render, SharedString, Window, canvas, div, point,
    prelude::*, px, quad, rgb, size, transparent_black,
};

use super::ChartMeta;

pub(super) struct ChartView {
    candles: Vec<Candle>,
    price_min: f64,
    price_max: f64,
    interval: Option<Interval>,
    source: String,
}

impl ChartView {
    pub(super) fn new(base_candles: Vec<Candle>, meta: ChartMeta) -> Self {
        let (candles, interval) = match meta.initial_interval {
            Some(i) => (resample(&base_candles, i), Some(i)),
            None => (base_candles, None),
        };
        let (price_min, price_max) = padded_bounds(&candles);
        Self {
            candles,
            price_min,
            price_max,
            interval,
            source: meta.source,
        }
    }

    fn interval_label(interval: Option<Interval>) -> SharedString {
        let label = match interval {
            Some(Interval::Minute(n)) => format!("{n}m"),
            Some(Interval::Hour(n)) => format!("{n}h"),
            Some(Interval::Day(n)) => format!("{n}d"),
            None => "raw".to_string(),
        };
        SharedString::from(label)
    }
}

impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let interval_label = Self::interval_label(self.interval);
        let candle_count = self.candles.len();
        let range_text =
            SharedString::from(format!("{:.4} - {:.4}", self.price_min, self.price_max));

        let candles = self.candles.clone();
        let price_min = self.price_min;
        let price_max = self.price_max;

        let chart = canvas(
            move |_, _, _| candles.clone(),
            move |bounds, candles, window, _| {
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
                if candles.is_empty() || height <= 0.0 || width <= 0.0 {
                    return;
                }

                let range = (price_max - price_min).max(1e-9);
                let candle_width = (width / candles.len() as f32).max(1.0);
                let body_width = (candle_width * 0.6).max(1.0);

                let price_to_y = |price: f64| -> f32 {
                    let normalized = ((price - price_min) / range).clamp(0.0, 1.0);
                    oy + (1.0 - normalized as f32) * height
                };

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

                for (idx, candle) in candles.iter().enumerate() {
                    let x = ox + idx as f32 * candle_width + candle_width * 0.5;
                    let open_y = price_to_y(candle.open);
                    let close_y = price_to_y(candle.close);
                    let high_y = price_to_y(candle.high);
                    let low_y = price_to_y(candle.low);

                    let body_top = open_y.min(close_y);
                    let body_height = (open_y - close_y).abs().max(1.0);
                    let color = if candle.close >= candle.open {
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
        .flex_1()
        .w_full();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .p_3()
                    .bg(rgb(0x111827))
                    .border_b_1()
                    .border_color(rgb(0x1f2937))
                    .child(
                        div()
                            .text_sm()
                            .child(SharedString::from(self.source.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .text_sm()
                            .child(format!("interval: {interval_label}"))
                            .child(format!("candles: {candle_count}"))
                            .child(format!("range: {range_text}")),
                    ),
            )
            .child(chart)
    }
}

fn padded_bounds(candles: &[Candle]) -> (f64, f64) {
    let (min, mut max) = bounds(candles).unwrap_or((0.0, 1.0));
    if min == max {
        max = min + 1.0;
    }
    let pad = ((max - min) * 0.01).max(0.0);
    (min - pad, max + pad)
}
