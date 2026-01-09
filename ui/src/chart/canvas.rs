use std::sync::Arc;

use core::Candle;
use gpui::{
    BorderStyle, Bounds, Canvas, PathBuilder, canvas, point, px, quad, rgb, size, transparent_black,
};

use super::aggregation::AggregatedCandle;

#[derive(Clone)]
pub(super) struct CandleViewport {
    candles: Arc<[Candle]>,
    start: usize,
    end: usize,
    aggregated: Option<Arc<[AggregatedCandle]>>,
    volume_max: Option<f64>,
}

pub(super) fn chart_canvas(
    candles: Arc<[Candle]>,
    start: usize,
    end: usize,
    price_min: f64,
    price_max: f64,
    hover_local: Option<usize>,
    hover_x: Option<f32>,
    hover_y: Option<f32>,
    aggregated: Option<Arc<[AggregatedCandle]>>,
) -> Canvas<CandleViewport> {
    canvas(
        move |_, _, _| CandleViewport {
            candles: candles.clone(),
            start,
            end,
            aggregated: aggregated.clone(),
            volume_max: None,
        },
        move |bounds, viewport, window, _| {
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
            if viewport.candles.is_empty() || height <= 0.0 || width <= 0.0 {
                return;
            }

            let start = viewport.start.min(viewport.candles.len());
            let end = viewport.end.min(viewport.candles.len()).max(start);
            let candles = &viewport.candles[start..end];
            if candles.is_empty() {
                return;
            }
            let candle_count = candles.len();

            let range = (price_max - price_min).max(1e-9);
            let x_for_idx = |idx: usize| -> f32 {
                let t = (idx as f32 + 0.5) / candle_count as f32;
                ox + t * width
            };

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

            if let Some(aggregated) = viewport.aggregated.as_deref()
                && !aggregated.is_empty()
            {
                let columns = aggregated.len();
                let column_width = (width / columns as f32).max(f32::EPSILON);
                let body_width = (column_width * 0.6).max(f32::EPSILON);
                for (col, agg) in aggregated.iter().enumerate() {
                    let open_y = price_to_y(agg.open);
                    let close_y = price_to_y(agg.close);
                    let high_y = price_to_y(agg.high);
                    let low_y = price_to_y(agg.low);

                    let body_top = open_y.min(close_y);
                    let body_height = (open_y - close_y).abs().max(1.0);
                    let color = if agg.close >= agg.open {
                        rgb(0x22c55e)
                    } else {
                        rgb(0xef4444)
                    };

                    let x = ox + (col as f32 + 0.5) * column_width;

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
            } else {
                let columns = width.floor().max(1.0) as usize;
                if candle_count <= columns {
                    let candle_width = (width / candle_count as f32).max(f32::EPSILON);
                    let body_width = (candle_width * 0.6).max(f32::EPSILON);
                    for (idx, candle) in candles.iter().enumerate() {
                        let x = x_for_idx(idx);
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
                } else {
                    let column_width = (width / columns as f32).max(f32::EPSILON);
                    let body_width = (column_width * 0.6).max(f32::EPSILON);
                    for col in 0..columns {
                        let g_start = col * candle_count / columns;
                        let g_end = ((col + 1) * candle_count / columns).max(g_start + 1);
                        let group = &candles[g_start..g_end];
                        let first = &group[0];
                        let last = &group[group.len() - 1];

                        let mut high = first.high;
                        let mut low = first.low;
                        for c in group {
                            high = high.max(c.high);
                            low = low.min(c.low);
                        }

                        let open_y = price_to_y(first.open);
                        let close_y = price_to_y(last.close);
                        let high_y = price_to_y(high);
                        let low_y = price_to_y(low);
                        let body_top = open_y.min(close_y);
                        let body_height = (open_y - close_y).abs().max(1.0);
                        let color = if last.close >= first.open {
                            rgb(0x22c55e)
                        } else {
                            rgb(0xef4444)
                        };

                        let x = ox + (col as f32 + 0.5) * column_width;

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
                }
            }

            // hover crosshair
            if hover_local.is_some() {
                let x = if let Some(x) = hover_x {
                    x.clamp(ox, ox + width)
                } else if let Some(local_idx) = hover_local {
                    x_for_idx(local_idx.min(candle_count.saturating_sub(1)))
                } else {
                    return;
                };
                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(x), px(oy)));
                builder.line_to(point(px(x), px(oy + height)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0xf59e0b));
                }
            }

            if let Some(y) = hover_y {
                let y = y.clamp(oy, oy + height);
                let mut builder = PathBuilder::stroke(px(1.));
                builder.move_to(point(px(ox), px(y)));
                builder.line_to(point(px(ox + width), px(y)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, rgb(0xf59e0b));
                }
            }
        },
    )
}

pub(super) fn volume_canvas(
    candles: Arc<[Candle]>,
    start: usize,
    end: usize,
    hover_local: Option<usize>,
    hover_x: Option<f32>,
    aggregated: Option<Arc<[AggregatedCandle]>>,
    volume_max: Option<f64>,
) -> Canvas<CandleViewport> {
    canvas(
        move |_, _, _| CandleViewport {
            candles: candles.clone(),
            start,
            end,
            aggregated: aggregated.clone(),
            volume_max,
        },
        move |bounds, viewport, window, _| {
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
            if viewport.candles.is_empty() || height <= 0.0 || width <= 0.0 {
                return;
            }

            let start = viewport.start.min(viewport.candles.len());
            let end = viewport.end.min(viewport.candles.len()).max(start);
            let candles = &viewport.candles[start..end];
            if candles.is_empty() {
                return;
            }
            let candle_count = candles.len();

            let x_for_idx = |idx: usize| -> f32 {
                let t = (idx as f32 + 0.5) / candle_count as f32;
                ox + t * width
            };

            if let Some(aggregated) = viewport.aggregated.as_deref()
                && !aggregated.is_empty()
            {
                let columns = aggregated.len();
                let column_width = (width / columns as f32).max(f32::EPSILON);
                let bar_width = (column_width * 0.7).max(f32::EPSILON);
                let max_vol = viewport
                    .volume_max
                    .or_else(|| {
                        aggregated
                            .iter()
                            .map(|c| c.volume)
                            .fold(None, |acc, v| Some(acc.unwrap_or(0.0).max(v)))
                    })
                    .unwrap_or(0.0)
                    .max(1e-9);

                for (col, agg) in aggregated.iter().enumerate() {
                    let x = ox + (col as f32 + 0.5) * column_width;
                    let normalized = (agg.volume / max_vol).clamp(0.0, 1.0);
                    let bar_h = (normalized as f32 * height).max(1.0);
                    let y = oy + height - bar_h;
                    let color = if agg.close >= agg.open {
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
            } else {
                let columns = width.floor().max(1.0) as usize;
                if candle_count <= columns {
                    let max_vol = candles
                        .iter()
                        .map(|c| c.volume)
                        .fold(0.0_f64, f64::max)
                        .max(1e-9);

                    let candle_width = (width / candle_count as f32).max(f32::EPSILON);
                    let bar_width = (candle_width * 0.7).max(f32::EPSILON);

                    for (idx, candle) in candles.iter().enumerate() {
                        let x = x_for_idx(idx);
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
                } else {
                    let mut max_group_vol = 0.0_f64;
                    for col in 0..columns {
                        let g_start = col * candle_count / columns;
                        let g_end = ((col + 1) * candle_count / columns).max(g_start + 1);
                        let mut vol_sum = 0.0_f64;
                        for c in &candles[g_start..g_end] {
                            vol_sum += c.volume;
                        }
                        max_group_vol = max_group_vol.max(vol_sum);
                    }
                    let max_group_vol = max_group_vol.max(1e-9);

                    let column_width = (width / columns as f32).max(f32::EPSILON);
                    let bar_width = (column_width * 0.7).max(f32::EPSILON);

                    for col in 0..columns {
                        let g_start = col * candle_count / columns;
                        let g_end = ((col + 1) * candle_count / columns).max(g_start + 1);
                        let group = &candles[g_start..g_end];
                        let first = &group[0];
                        let last = &group[group.len() - 1];
                        let mut vol_sum = 0.0_f64;
                        for c in group {
                            vol_sum += c.volume;
                        }

                        let x = ox + (col as f32 + 0.5) * column_width;
                        let normalized = (vol_sum / max_group_vol).clamp(0.0, 1.0);
                        let bar_h = (normalized as f32 * height).max(1.0);
                        let y = oy + height - bar_h;
                        let color = if last.close >= first.open {
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
                }
            }

            if let Some(local_idx) = hover_local {
                let x = if let Some(x) = hover_x {
                    x.clamp(ox, ox + width)
                } else {
                    x_for_idx(local_idx.min(candle_count.saturating_sub(1)))
                };
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
