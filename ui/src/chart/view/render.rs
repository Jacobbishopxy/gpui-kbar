use core::Interval;
use gpui::{
    Bounds, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render,
    ScrollWheelEvent, SharedString, Window, div, prelude::*, px, rgb,
};
use time::macros::format_description;

use super::{padded_bounds, ChartView};
use super::super::{canvas::chart_canvas, header::chart_header};

const INTERVAL_OPTIONS: &[(Option<Interval>, &str)] = &[
    (None, "Raw"),
    (Some(Interval::Minute(1)), "1m"),
    (Some(Interval::Minute(5)), "5m"),
    (Some(Interval::Minute(15)), "15m"),
    (Some(Interval::Hour(1)), "1h"),
    (Some(Interval::Day(1)), "1d"),
];

impl Render for ChartView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let interval_label = ChartView::interval_label(self.interval);
        let (start, end) = self.visible_range();
        let visible = if start < end {
            &self.candles[start..end]
        } else {
            &self.candles[..]
        };
        let candle_count = visible.len();
        let (price_min, price_max) = padded_bounds(visible);
        self.price_min = price_min;
        self.price_max = price_max;
        let range_text = SharedString::from(format!("{:.4} - {:.4}", price_min, price_max));
        let tooltip = self.tooltip_overlay(start, end);

        let price_labels = [
            format!("{price_max:.4}"),
            format!("{:.4}", (price_min + price_max) * 0.5),
            format!("{price_min:.4}"),
        ];

        let time_fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
        let start_label = visible
            .first()
            .map(|c| c.timestamp.format(&time_fmt).unwrap_or_else(|_| c.timestamp.to_string()));
        let mid_label = visible
            .get(candle_count.saturating_sub(1) / 2)
            .map(|c| c.timestamp.format(&time_fmt).unwrap_or_else(|_| c.timestamp.to_string()));
        let end_label = visible
            .last()
            .map(|c| c.timestamp.format(&time_fmt).unwrap_or_else(|_| c.timestamp.to_string()));

        let candles = visible.to_vec();
        let price_min = self.price_min;
        let price_max = self.price_max;
        let hover_local = self.hover_index.and_then(|idx| {
            if start <= idx && idx < end {
                Some(idx - start)
            } else {
                None
            }
        });

        let track_chart_bounds =
            _cx.processor(|this: &mut Self, bounds: Vec<Bounds<Pixels>>, _, _| {
                if let Some(canvas_bounds) = bounds.first() {
                    this.chart_bounds = Some(*canvas_bounds);
                }
            });

        let handle_scroll = _cx.listener(|this: &mut Self, event: &ScrollWheelEvent, window, _| {
            this.handle_scroll(event, window);
        });

        let handle_mouse_down =
            _cx.listener(|this: &mut Self, event: &MouseDownEvent, window, _| {
                if event.button == MouseButton::Left {
                    this.dragging = true;
                    this.last_drag_position =
                        Some((f32::from(event.position.x), f32::from(event.position.y)));
                    window.refresh();
                }
            });

        let handle_mouse_up = _cx.listener(|this: &mut Self, _: &MouseUpEvent, window, _| {
            this.dragging = false;
            this.last_drag_position = None;
            window.refresh();
        });

        let handle_mouse_move =
            _cx.listener(move |this: &mut Self, event: &MouseMoveEvent, window, _| {
                this.handle_hover(event, candle_count);
                this.handle_drag(event, window);
            });

        let chart = chart_canvas(candles, price_min, price_max, hover_local)
            .flex_1()
            .w_full()
            .h_full();

        let mut canvas_region = div()
            .flex_1()
            .w_full()
            .h_full()
            .relative()
            .on_children_prepainted(track_chart_bounds)
            .child(chart);
        if let Some(tip) = tooltip {
            canvas_region = canvas_region.child(tip);
        }

        let price_axis = div()
            .w(px(82.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .items_end()
            .px_2()
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .child(price_labels[0].clone())
            .child(price_labels[1].clone())
            .child(price_labels[2].clone());

        let chart_row = div()
            .flex_1()
            .flex()
            .w_full()
            .h_full()
            .min_h(px(320.))
            .on_mouse_down(MouseButton::Left, handle_mouse_down)
            .on_mouse_move(handle_mouse_move)
            .on_mouse_up(MouseButton::Left, handle_mouse_up)
            .on_scroll_wheel(handle_scroll)
            .child(price_axis)
            .child(canvas_region);

        let time_axis = div()
            .h(px(28.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .bg(rgb(0x0f172a))
            .child(start_label.unwrap_or_else(|| "---".into()))
            .child(mid_label.unwrap_or_else(|| "---".into()))
            .child(end_label.unwrap_or_else(|| "---".into()));

        let interval_button =
            |option: Option<Interval>, label: &str| -> Div {
                let is_active = self.interval == option;
                let handler =
                    _cx.listener(move |this: &mut Self, _: &MouseDownEvent, window, _| {
                        this.apply_interval(option);
                        window.refresh();
                    });
                let label_text = SharedString::from(label.to_string());
                let bg = if is_active { rgb(0x1f2937) } else { rgb(0x111827) };
                let border = if is_active { rgb(0xf59e0b) } else { rgb(0x1f2937) };

                div()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(bg)
                    .text_sm()
                    .text_color(gpui::white())
                    .on_mouse_down(MouseButton::Left, handler)
                    .child(label_text)
            };

        let mut interval_row = div()
            .flex()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(0x0f172a))
            .border_b_1()
            .border_color(rgb(0x1f2937));
        for (option, label) in INTERVAL_OPTIONS {
            interval_row = interval_row.child(interval_button(*option, label));
        }

        let chart_area = div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(360.))
            .child(chart_row)
            .child(time_axis);

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(chart_header(
                &self.source,
                interval_label,
                candle_count,
                range_text,
            ))
            .child(interval_row)
            .child(chart_area)
    }
}
