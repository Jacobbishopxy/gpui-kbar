use gpui::{Div, SharedString, div, prelude::*, px, rgb};

pub(super) fn range_button(label: impl Into<SharedString>, active: bool) -> Div {
    let label = label.into();
    let (bg, text, border) = if active {
        (rgb(0x1f2937), rgb(0xffffff), rgb(0x2563eb))
    } else {
        (rgb(0x111827), rgb(0xe5e7eb), rgb(0x1f2937))
    };

    div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_sm()
        .text_color(text)
        .child(label)
}

pub(super) fn chart_footer(
    quick_ranges: impl IntoElement,
    interval_label: SharedString,
    candle_count: usize,
    range_text: SharedString,
    live: bool,
    playback_label: SharedString,
    timezone_label: SharedString,
) -> Div {
    let playback_color = if live { rgb(0x22c55e) } else { rgb(0xf59e0b) };

    let playback = div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(playback_color))
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
