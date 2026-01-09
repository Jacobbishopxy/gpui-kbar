use gpui::{
    Bounds, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    ScrollWheelEvent, div, prelude::*, px, rgb, rgba,
};

use crate::chart::view::ChartView;

/// Builds the main chart area (price + volume + time axis).
#[allow(clippy::too_many_arguments)]
pub fn chart_body(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    price_labels: [String; 3],
    chart: impl IntoElement,
    volume: impl IntoElement,
    start_label: String,
    mid_label: String,
    end_label: String,
    candle_count: usize,
) -> Div {
    let track_chart_bounds =
        cx.processor(|this: &mut ChartView, bounds: Vec<Bounds<Pixels>>, _, _| {
            if let Some(canvas_bounds) = bounds.first() {
                this.chart_bounds = Some(*canvas_bounds);
            }
        });

    let handle_scroll = cx.listener(
        |this: &mut ChartView, event: &ScrollWheelEvent, window, _| {
            this.handle_scroll(event, window);
        },
    );

    let handle_mouse_down =
        cx.listener(|this: &mut ChartView, event: &MouseDownEvent, window, _| {
            if this.settings_open || this.symbol_search_open {
                return;
            }
            if event.button == MouseButton::Left {
                this.dragging = true;
                this.last_drag_position =
                    Some((f32::from(event.position.x), f32::from(event.position.y)));
                window.refresh();
            }
        });

    let handle_mouse_up = cx.listener(|this: &mut ChartView, _: &MouseUpEvent, window, _| {
        if this.settings_open {
            this.dragging = false;
            this.last_drag_position = None;
            window.refresh();
            return;
        }
        this.dragging = false;
        this.last_drag_position = None;
        let _ = this.persist_viewport();
        window.refresh();
    });

    let handle_mouse_move = cx.listener(
        move |this: &mut ChartView, event: &MouseMoveEvent, window, _| {
            if this.settings_open {
                return;
            }
            this.handle_hover(event, candle_count);
            this.handle_drag(event, window);
        },
    );

    let canvas_region = div()
        .flex_1()
        .w_full()
        .h_full()
        .relative()
        .on_children_prepainted(track_chart_bounds)
        .child(div().flex_1().w_full().h_full().child(chart));

    let hover_price_label =
        if let (Some((_, y)), Some(bounds)) = (view.hover_position, view.chart_bounds) {
            let height = f32::from(bounds.size.height);
            if height <= 0.0 {
                None
            } else {
                let oy = f32::from(bounds.origin.y);
                let frac = ((y - oy) / height).clamp(0.0, 1.0);
                let price = view.price_max - (view.price_max - view.price_min) * frac as f64;
                let label_h = 18.0;
                let mut top = frac * height - label_h * 0.5;
                top = top.clamp(0.0, height - label_h);

                Some(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(top))
                        .w(px(82.))
                        .h(px(label_h))
                        .px_1()
                        .bg(rgba(0x1f293780))
                        .border_1()
                        .border_color(rgba(0x37415180))
                        .rounded_sm()
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_xs()
                        .text_color(gpui::white())
                        .child(format!("{price:.4}")),
                )
            }
        } else {
            None
        };

    let mut price_axis = div()
        .w(px(82.))
        .h_full()
        .flex()
        .flex_col()
        .justify_between()
        .items_end()
        .px_2()
        .bg(rgb(0x0f172a))
        .border_r_1()
        .border_color(rgb(0x1f2937))
        .text_xs()
        .text_color(rgb(0x9ca3af))
        .relative()
        .child(price_labels[0].clone())
        .child(price_labels[1].clone())
        .child(price_labels[2].clone());

    price_axis = if let Some(label) = hover_price_label {
        price_axis.child(label)
    } else {
        price_axis
    };

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
        .border_t_1()
        .border_color(rgb(0x1f2937))
        .child(start_label)
        .child(mid_label)
        .child(end_label);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .h_full()
        .min_h(px(420.))
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .overflow_hidden()
        .child(chart_row)
        .child(
            div()
                .flex()
                .w_full()
                .h(px(120.))
                .min_h(px(100.))
                .child(
                    div()
                        .w(px(82.))
                        .h_full()
                        .bg(rgb(0x0f172a))
                        .border_r_1()
                        .border_color(rgb(0x1f2937)),
                )
                .child(div().flex_1().w_full().h_full().child(volume)),
        )
        .child(time_axis)
}
