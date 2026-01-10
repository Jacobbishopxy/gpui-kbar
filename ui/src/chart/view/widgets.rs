use gpui::{Div, SharedString, Stateful, div, prelude::*, px, rgb, svg};

use crate::components::button_effect;

pub fn toolbar_button(label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    let label = label.into();
    let bg_hex = if active { 0x111827 } else { 0x0f172a };
    let button_id: SharedString = format!("toolbar-button-{label}").into();
    button_effect::apply(
        div()
            .w(px(36.))
            .h(px(36.))
            .rounded_md()
            .bg(rgb(bg_hex))
            .border_1()
            .border_color(rgb(0x1f2937))
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(rgb(0xe5e7eb))
            .child(label)
            .id(button_id),
        bg_hex,
    )
}

pub fn header_chip(label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    let chip_id: SharedString = format!("header-chip-{label}").into();
    button_effect::apply(
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
            .id(chip_id),
        0x111827,
    )
}

pub fn header_icon(path: &str, tooltip: &str) -> Stateful<Div> {
    let tooltip_text = SharedString::from(tooltip.to_owned());
    let icon_id = SharedString::from(format!("header-icon-{path}-{tooltip}"));
    let icon_path = SharedString::from(path.to_owned());
    let icon = div()
        .w(px(36.))
        .h(px(36.))
        .rounded_md()
        .bg(rgb(0x111827))
        .border_1()
        .border_color(rgb(0x1f2937))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon_path)
                .w(px(18.))
                .h(px(18.))
                .text_color(rgb(0xe5e7eb)),
        )
        .id(icon_id);

    button_effect::apply(icon, 0x111827).tooltip(move |_, cx| {
        let text = tooltip_text.clone();
        cx.new(|_| HeaderIconTooltip { text }).into()
    })
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

struct HeaderIconTooltip {
    text: SharedString,
}

impl Render for HeaderIconTooltip {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(0x111827))
            .border_1()
            .border_color(rgb(0x1f2937))
            .text_xs()
            .text_color(gpui::white())
            .child(self.text.clone())
    }
}
