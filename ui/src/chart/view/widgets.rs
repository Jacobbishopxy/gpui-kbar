use gpui::{Div, SharedString, div, prelude::*, px, rgb};

pub fn toolbar_button(label: impl Into<SharedString>, active: bool) -> Div {
    let label = label.into();
    let bg = if active { rgb(0x111827) } else { rgb(0x0f172a) };
    div()
        .w(px(36.))
        .h(px(36.))
        .rounded_md()
        .bg(bg)
        .border_1()
        .border_color(rgb(0x1f2937))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .text_color(rgb(0xe5e7eb))
        .child(label)
}

pub fn header_chip(label: impl Into<SharedString>) -> Div {
    let label = label.into();
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0x111827))
        .border_1()
        .border_color(rgb(0x1f2937))
        .text_sm()
        .text_color(rgb(0xe5e7eb))
        .child(label)
}

pub fn stat_row(label: impl Into<SharedString>, value: impl Into<String>) -> Div {
    let label = label.into();
    div()
        .flex()
        .items_center()
        .justify_between()
        .text_xs()
        .text_color(rgb(0x9ca3af))
        .child(label)
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xe5e7eb))
                .child(value.into()),
        )
}
