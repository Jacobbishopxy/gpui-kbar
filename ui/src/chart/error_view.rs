use gpui::{Render, SharedString, Window, div, prelude::*, px, rgb};

pub(super) struct ErrorView {
    source: String,
    message: String,
}

impl ErrorView {
    pub(super) fn new(source: String, message: String) -> Self {
        Self { source, message }
    }
}

impl Render for ErrorView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let source = SharedString::from(self.source.clone());
        let message = SharedString::from(self.message.clone());

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x0b1220))
            .text_color(gpui::white())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .w_full()
                    .h_full()
                    .text_center()
                    .child(div().text_lg().child("Load error"))
                    .child(div().text_sm().text_color(rgb(0x9ca3af)).child(source))
                    .child(
                        div()
                            .max_w(px(640.))
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x111827))
                            .border_1()
                            .border_color(rgb(0x1f2937))
                            .child(message),
                    ),
            )
    }
}
