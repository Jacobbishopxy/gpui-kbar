use gpui::{SharedString, div, prelude::*, rgb};

pub(super) fn chart_footer(
    interval_label: SharedString,
    candle_count: usize,
    range_text: SharedString,
) -> impl IntoElement {
    div()
        .flex()
        .justify_end()
        .items_center()
        .p_3()
        .bg(rgb(0x111827))
        .border_t_1()
        .border_color(rgb(0x1f2937))
        .child(
            div()
                .flex()
                .gap_3()
                .text_sm()
                .child(format!("interval: {interval_label}"))
                .child(format!("candles: {candle_count}"))
                .child(format!("range: {range_text}")),
        )
}
