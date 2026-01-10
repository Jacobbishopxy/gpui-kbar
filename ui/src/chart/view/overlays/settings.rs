use gpui::{
    Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, ScrollWheelEvent, Stateful, Window,
    div, prelude::*, px, rgb, rgba, svg,
};

use crate::chart::view::ChartView;
use crate::chart::view::widgets::header_chip;
use crate::components::button_effect;

fn section(title: &str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x9ca3af))
                .child(title.to_string()),
        )
        .child(content)
}

fn row(label: &str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xe5e7eb))
                .child(label.to_string()),
        )
        .child(content)
}

fn chip_button(
    label: &'static str,
    active: bool,
    handle: impl Fn(&mut ChartView, &MouseDownEvent, &mut Window, &mut Context<ChartView>) + 'static,
    cx: &mut Context<ChartView>,
) -> Stateful<Div> {
    let handle = cx.listener(handle);
    header_chip(label)
        .border_color(if active { rgb(0x2563eb) } else { rgb(0x1f2937) })
        .text_color(if active { rgb(0xffffff) } else { rgb(0xe5e7eb) })
        .on_mouse_down(MouseButton::Left, handle)
}

pub fn settings_overlay(view: &mut ChartView, cx: &mut Context<ChartView>) -> Option<Div> {
    if !view.settings_open {
        return None;
    }

    let close_panel = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, cx| {
        this.close_settings();
        cx.stop_propagation();
        window.refresh();
    });
    let close_overlay = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, cx| {
        this.close_settings();
        cx.stop_propagation();
        window.refresh();
    });
    let block_click = cx.listener(|_: &mut ChartView, _: &MouseDownEvent, _, cx| {
        cx.stop_propagation();
    });
    let block_mouse_move = cx.listener(|_: &mut ChartView, _: &MouseMoveEvent, _, cx| {
        cx.stop_propagation();
    });
    let block_scroll = cx.listener(|_: &mut ChartView, _: &ScrollWheelEvent, _, cx| {
        cx.stop_propagation();
    });

    let perf_mode = view.perf_mode;
    let live_mode = view.live_mode;
    let n = view.perf_n;
    let step = view.perf_step_secs;

    let source_row = row(
        "Source",
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(chip_button(
                "File",
                !perf_mode && !live_mode,
                |this, _, window, cx| {
                    this.set_perf_mode_enabled(false, window, cx);
                    this.set_live_mode_enabled(false, window, cx);
                },
                cx,
            ))
            .child(chip_button(
                "Live",
                live_mode,
                |this, _, window, cx| this.set_live_mode_enabled(true, window, cx),
                cx,
            ))
            .child(chip_button(
                "Perf",
                perf_mode,
                |this, _, window, cx| this.set_perf_mode_enabled(true, window, cx),
                cx,
            )),
    );

    let perf_dataset_row = row(
        "Dataset",
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(chip_button(
                "50k",
                n == 50_000,
                |this, _, window, cx| {
                    this.set_perf_n(50_000);
                    this.set_perf_mode_enabled(true, window, cx);
                },
                cx,
            ))
            .child(chip_button(
                "200k",
                n == 200_000,
                |this, _, window, cx| {
                    this.set_perf_n(200_000);
                    this.set_perf_mode_enabled(true, window, cx);
                },
                cx,
            ))
            .child(chip_button(
                "1M",
                n == 1_000_000,
                |this, _, window, cx| {
                    this.set_perf_n(1_000_000);
                    this.set_perf_mode_enabled(true, window, cx);
                },
                cx,
            )),
    );

    let perf_step_row = row(
        "Step",
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(chip_button(
                "1s",
                step == 1,
                |this, _, window, cx| {
                    this.set_perf_step_secs(1);
                    if this.perf_mode {
                        this.start_perf_preset_load(this.perf_n, window, cx);
                    }
                },
                cx,
            ))
            .child(chip_button(
                "60s",
                step == 60,
                |this, _, window, cx| {
                    this.set_perf_step_secs(60);
                    if this.perf_mode {
                        this.start_perf_preset_load(this.perf_n, window, cx);
                    }
                },
                cx,
            ))
            .child(chip_button(
                "300s",
                step == 300,
                |this, _, window, cx| {
                    this.set_perf_step_secs(300);
                    if this.perf_mode {
                        this.start_perf_preset_load(this.perf_n, window, cx);
                    }
                },
                cx,
            )),
    );

    let replay_row = {
        let active = view.replay_enabled();
        row(
            "Replay",
            chip_button(
                if active { "On" } else { "Off" },
                active,
                |this, _, window, _| {
                    let next = !this.replay_enabled();
                    this.set_replay_mode(next);
                    window.refresh();
                },
                cx,
            ),
        )
    };

    let reset_row = row(
        "Defaults",
        chip_button(
            "Reset",
            false,
            |this, _, window, cx| {
                this.reset_settings_to_defaults(window, cx);
                window.refresh();
            },
            cx,
        ),
    );
    let cleanup_row = row(
        "Legacy",
        chip_button(
            "Cleanup",
            false,
            |this, _, window, _| {
                this.cleanup_legacy_perf_active_source();
                window.refresh();
            },
            cx,
        ),
    );

    let mut data_section = div().flex().flex_col().gap_3().child(source_row);
    if perf_mode {
        data_section = data_section.child(perf_dataset_row).child(perf_step_row);
    }

    let panel = div()
        .w(px(360.))
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .on_mouse_down(MouseButton::Left, block_click)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_lg().text_color(gpui::white()).child("Settings"))
                .child(button_effect::apply(
                    div()
                        .w(px(32.))
                        .h(px(32.))
                        .rounded_md()
                        .bg(rgb(0x111827))
                        .border_1()
                        .border_color(rgb(0x1f2937))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, close_panel)
                        .child(
                            svg()
                                .path("cross-circle.svg")
                                .w(px(18.))
                                .h(px(18.))
                                .text_color(rgb(0xe5e7eb)),
                        )
                        .id("settings-close"),
                    0x111827,
                )),
        )
        .child(section(
            "Data",
            data_section,
        ))
        .child(section(
            "Chart",
            div().flex().flex_col().gap_3().child(replay_row),
        ))
        .child(section(
            "Actions",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(reset_row)
                .child(cleanup_row),
        ));

    Some(
        div()
            .absolute()
            .left(px(0.))
            .top(px(0.))
            .w_full()
            .h_full()
            .bg(rgba(0x00000000))
            .on_mouse_down(MouseButton::Left, close_overlay)
            .on_mouse_move(block_mouse_move)
            .on_scroll_wheel(block_scroll)
            .child(div().absolute().right(px(16.)).top(px(76.)).child(panel)),
    )
}
