use core::Candle;
use gpui::{
    BorderStyle, Bounds, Canvas, PathBuilder, canvas, point, px, quad, rgb, size, transparent_black,
};

pub(super) fn chart_canvas(
    candles: Vec<Candle>,
    price_min: f64,
    price_max: f64,
    hover_local: Option<usize>,
) -> Canvas<Vec<Candle>> {
    canvas(
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
            // Use the actual per-candle width so all candles fit within the viewport, even when
            // there are more candles than pixels. Clamp to >0 to avoid division by zero.
            let candle_width = (width / candles.len() as f32).max(f32::EPSILON);
            let body_width = (candle_width * 0.6).max(f32::EPSILON);

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

            // hover crosshair
            if let Some(local_idx) = hover_local {
                let x = ox + local_idx as f32 * candle_width + candle_width * 0.5;
                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(x), px(oy)));
                builder.line_to(point(px(x), px(oy + height)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0xf59e0b));
                }
            }
        },
    )
}

pub(super) fn volume_canvas(
    candles: Vec<Candle>,
    hover_local: Option<usize>,
) -> Canvas<Vec<Candle>> {
    canvas(
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

            let max_vol = candles
                .iter()
                .map(|c| c.volume)
                .fold(0.0_f64, f64::max)
                .max(1e-9);

            let candle_width = (width / candles.len() as f32).max(f32::EPSILON);
            let bar_width = (candle_width * 0.7).max(f32::EPSILON);

            for (idx, candle) in candles.iter().enumerate() {
                let x = ox + idx as f32 * candle_width + candle_width * 0.5;
                let normalized = (candle.volume / max_vol).clamp(0.0, 1.0);
                let bar_h = (normalized as f32 * height).max(1.0);
                let y = oy + height - bar_h;
                let color = if candle.close >= candle.open {
                    rgb(0x22c55e)
                } else {
                    rgb(0xef4444)
                };

                let bar_bounds = Bounds {
                    origin: point(px(x - bar_width * 0.5), px(y)),
                    size: size(px(bar_width), px(bar_h)),
                };
                window.paint_quad(quad(
                    bar_bounds,
                    px(1.),
                    color,
                    px(0.),
                    color,
                    BorderStyle::default(),
                ));
            }

            if let Some(local_idx) = hover_local {
                let x = ox + local_idx as f32 * candle_width + candle_width * 0.5;
                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(x), px(oy)));
                builder.line_to(point(px(x), px(oy + height)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0xf59e0b));
                }
            }
        },
    )
}
