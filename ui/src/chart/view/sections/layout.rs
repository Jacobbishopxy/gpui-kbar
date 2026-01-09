use core::Interval;
use gpui::{
    Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, Window, div, prelude::*, px, rgb,
    rgba,
};

use crate::chart::view::{
    ChartView, INTERVAL_TRIGGER_WIDTH, TOOLBAR_WIDTH,
    overlays::interval_menu::interval_menu,
    render::RenderState,
    widgets::{stat_row, toolbar_button},
};
use crate::components::loading_sand::loading_sand;

use super::sidebar::sidebar;
use super::watchlist::watchlist_panel;

pub(crate) fn build_sidebar_panels(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    state: &RenderState,
) -> Div {
    let watchlist_panel = watchlist_panel(view, cx);
    let instrument_card = instrument_card(state);
    let trading_stub = trading_stub();
    sidebar(watchlist_panel, instrument_card, trading_stub)
}

pub(crate) fn build_body_layout(chart_area: Div, sidebar: Div) -> Div {
    let left_toolbar = build_left_toolbar();
    let main_column = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_3()
        .p_3()
        .child(chart_area);

    div()
        .flex()
        .flex_1()
        .w_full()
        .min_h(px(560.))
        .child(left_toolbar)
        .child(main_column)
        .child(sidebar)
}

pub(crate) fn build_root_container(header: Div, body: Div, footer: Div) -> Div {
    div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .relative()
        .bg(rgb(0x0b1220))
        .text_color(gpui::white())
        .child(header)
        .child(body)
        .child(footer)
}

pub(crate) fn build_layered_view(
    _view: &mut ChartView,
    cx: &mut Context<ChartView>,
    header: Div,
    body: Div,
    footer: Div,
    search_overlay: Option<Div>,
    interval_menu: Option<Div>,
    settings_overlay: Option<Div>,
    tooltip: Option<Div>,
    loading_overlay: Option<Div>,
) -> Div {
    let root = build_root_container(header, body, footer);
    let track_root = cx.processor(
        |this: &mut ChartView, bounds: Vec<gpui::Bounds<gpui::Pixels>>, _, _| {
            if let Some(root_bounds) = bounds.first() {
                this.root_origin = (
                    f32::from(root_bounds.origin.x),
                    f32::from(root_bounds.origin.y),
                );
            }
        },
    );

    let mut layered = div()
        .relative()
        .w_full()
        .h_full()
        .on_children_prepainted(track_root)
        .child(root);

    if let Some(overlay) = search_overlay {
        layered = layered.child(overlay);
    }

    if let Some(menu) = interval_menu {
        layered = layered.child(menu);
    }

    if let Some(settings) = settings_overlay {
        layered = layered.child(settings);
    }

    if let Some(tip) = tooltip {
        layered = layered.child(tip);
    }

    if let Some(loading) = loading_overlay {
        layered = layered.child(loading);
    }

    let clear_hover = cx.listener(
        |this: &mut ChartView, event: &MouseMoveEvent, window: &mut Window, _| {
            if this.symbol_search_open {
                if this.hover_index.is_some() || this.hover_position.is_some() {
                    this.hover_index = None;
                    this.hover_position = None;
                    window.refresh();
                }
                return;
            }

            let should_clear = match this.chart_bounds {
                Some(bounds) => {
                    let bx = f32::from(bounds.origin.x);
                    let by = f32::from(bounds.origin.y);
                    let bw = f32::from(bounds.size.width);
                    let bh = f32::from(bounds.size.height);
                    let px = f32::from(event.position.x);
                    let py = f32::from(event.position.y);
                    px < bx || px > bx + bw || py < by || py > by + bh
                }
                None => true,
            };

            if should_clear && (this.hover_index.is_some() || this.hover_position.is_some()) {
                this.hover_index = None;
                this.hover_position = None;
                window.refresh();
            }
        },
    );

    layered.on_mouse_move(clear_hover)
}

pub(crate) fn build_loading_overlay(view: &ChartView, cx: &mut Context<ChartView>) -> Option<Div> {
    let symbol = view.loading_symbol.as_deref()?;
    let block_input = cx.listener(|_: &mut ChartView, _: &MouseDownEvent, _, cx| {
        cx.stop_propagation();
    });

    Some(
        div()
            .absolute()
            .left(px(0.))
            .top(px(0.))
            .w_full()
            .h_full()
            .bg(rgba(0x0b122080))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, block_input)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .bg(rgb(0x0b1220))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .rounded_md()
                    .child(loading_sand(32.0, rgb(0xf59e0b)))
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::white())
                            .child(format!("Loading {symbol}...")),
                    ),
            ),
    )
}

pub(crate) fn build_interval_menu(
    view: &mut ChartView,
    cx: &mut Context<ChartView>,
    options: &[(Option<Interval>, &str)],
) -> Option<Div> {
    if !view.interval_select_open {
        return None;
    }

    let origin = (
        view.interval_trigger_origin.0 - view.root_origin.0,
        view.interval_trigger_origin.1 - view.root_origin.1,
    );
    interval_menu(
        view,
        cx,
        options,
        origin,
        view.interval_trigger_height,
        INTERVAL_TRIGGER_WIDTH,
    )
}

fn instrument_card(state: &RenderState) -> Div {
    div()
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x9ca3af))
                .child("Instrument"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_2xl()
                        .text_color(gpui::white())
                        .child(state.price_display.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(state.change_color))
                        .child(state.change_display.clone()),
                ),
        )
        .child(stat_row("Symbol", state.symbol_label.clone()))
        .child(stat_row("Interval", state.interval_label.to_string()))
        .child(stat_row("Candles", state.candle_count.to_string()))
        .child(stat_row("Range", state.range_text.to_string()))
}

fn build_left_toolbar() -> Div {
    let items = [
        "Cursor", "Trend", "Fib", "Brush", "Text", "Measure", "Zoom", "Cross",
    ];
    let mut left_toolbar = div()
        .w(px(TOOLBAR_WIDTH))
        .bg(rgb(0x0f172a))
        .border_r_1()
        .border_color(rgb(0x1f2937))
        .py_3()
        .flex()
        .flex_col()
        .items_center()
        .gap_2();
    for (idx, item) in items.iter().enumerate() {
        left_toolbar = left_toolbar.child(toolbar_button(*item, idx == 0));
    }
    left_toolbar
}

fn trading_stub() -> Div {
    div()
        .child(
            div()
                .text_sm()
                .text_color(gpui::white())
                .child("Trading panel"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x9ca3af))
                .child("Order ticket and positions will appear here."),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x2563eb))
                .text_sm()
                .text_color(gpui::white())
                .child("Open panel"),
        )
}
