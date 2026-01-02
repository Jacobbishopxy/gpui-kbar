use gpui::{Div, div, prelude::*, px, rgb};

/// Shared header wrapper for the chart layout.
pub(super) fn chart_header(left: impl IntoElement, right: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_4()
        .py_3()
        .min_h(px(64.))
        .bg(rgb(0x0f172a))
        .border_b_1()
        .border_color(rgb(0x1f2937))
        .child(left)
        .child(right)
}
