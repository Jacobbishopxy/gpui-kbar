use gpui::{SharedString, div, prelude::*, rgb};

pub(super) fn chart_footer(
    interval_control: impl IntoElement,
    interval_label: SharedString,
    candle_count: usize,
    range_text: SharedString,
) -> impl IntoElement {
    let right = div()
        .flex()
        .items_center()
        .gap_3()
        .text_xs()
        .text_color(rgb(0x9ca3af))
        .child(format!("interval: {interval_label}"))
        .child(format!("candles: {candle_count}"))
        .child(format!("range: {range_text}"))
        .child(
            div()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(0x111827))
                .child("UTC"),
        );

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_3()
        .bg(rgb(0x0f172a))
        .border_t_1()
        .border_color(rgb(0x1f2937))
        .child(div().flex().items_center().gap_3().child(interval_control))
        .child(right)
}
