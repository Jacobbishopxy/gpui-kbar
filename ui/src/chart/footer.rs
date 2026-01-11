use gpui::{Div, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::components::button_effect;

pub(super) fn range_button(label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    let label = label.into();
    let range_id: SharedString = format!("range-button-{label}").into();
    let (bg_hex, text, border) = if active {
        (0x1f2937, rgb(0xffffff), rgb(0x2563eb))
    } else {
        (0x111827, rgb(0xe5e7eb), rgb(0x1f2937))
    };

    button_effect::apply(
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .bg(rgb(bg_hex))
            .border_1()
            .border_color(border)
            .text_sm()
            .text_color(text)
            .child(label)
            .id(range_id),
        bg_hex,
    )
}

pub(super) fn chart_footer(
    quick_ranges: impl IntoElement,
    interval_label: SharedString,
    candle_count: usize,
    range_text: SharedString,
    playback_label: SharedString,
    playback_dot_hex: u32,
    timezone_label: SharedString,
) -> Div {
    let playback = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(10.))
                .h(px(10.))
                .rounded_full()
                .border_1()
                .border_color(rgb(0x1f2937))
                .bg(rgb(playback_dot_hex)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(rgb(0xe5e7eb))
                .child(playback_label.clone())
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x9ca3af))
                        .child(format!("{interval_label} • {candle_count} bars")),
                ),
        );

    let range_badge = div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x111827))
        .border_1()
        .border_color(rgb(0x1f2937))
        .text_xs()
        .text_color(rgb(0x9ca3af))
        .child(format!("Δ {range_text}"));

    let timezone = div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x111827))
        .border_1()
        .border_color(rgb(0x1f2937))
        .text_sm()
        .text_color(rgb(0xe5e7eb))
        .child(timezone_label);

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
        .child(div().flex().items_center().gap_2().child(quick_ranges))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(playback)
                .child(range_badge)
                .child(timezone),
        )
}
