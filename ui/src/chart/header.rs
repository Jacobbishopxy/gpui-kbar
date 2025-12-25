use gpui::{SharedString, div, prelude::*, rgb};

pub(super) fn chart_header(source: &str, right: impl IntoElement) -> impl IntoElement {
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
                .child(SharedString::from(source.to_string())),
        )
        .child(div().child(right))
}
