use gpui::{
    Context, Div, MouseButton, MouseDownEvent, SharedString, Window, div, prelude::*, px, rgb, rgba,
};

use crate::chart::view::ChartView;

const POPUP_WIDTH: f32 = 920.0;
const POPUP_HEIGHT: f32 = 630.0;

pub fn symbol_search_overlay(view: &mut ChartView, cx: &mut Context<ChartView>) -> Option<Div> {
    if !view.symbol_search_open {
        return None;
    }

    let search_filters = [
        "All", "Stocks", "Funds", "Futures", "Forex", "Crypto", "Indices", "Bonds", "Economy",
        "Options",
    ];
    let search_results = [
        ("100", "NDQ", "US 100 Index", "index cfd", "TVC"),
        ("ETF", "NDQ", "BetaShares NASDAQ 100 ETF", "fund etf", "ASX"),
        ("ETF", "NDQ", "Invesco QQQ Trust Series I", "fund etf", "TRADEGATE"),
        ("ETF", "NDQ", "Invesco QQQ Trust Series I", "fund etf", "BER"),
        ("ETF", "NDQ", "Invesco QQQ Trust Series I", "fund etf", "HAM"),
        ("100", "NDQM", "NASDAQ 100 Index (NDX)", "index cfd", "FXOpen"),
        ("CASH", "NDQ100", "Nasdaq Cash", "index cfd", "Eightcap"),
        (
            "CW",
            "NDQCC",
            "Cititwarrants 36.2423 NDQ 07-Jun-35 Instal Mini",
            "warrant",
            "CHIXAU",
        ),
        ("CR", "NDQUSD", "Nasdaq666", "spot crypto", "CRYPTO"),
        (
            "3L",
            "NDQ3L",
            "SG Issuer SA Exchange Traded Product 2022-03-18",
            "fund etf",
            "Euronext Paris",
        ),
        (
            "3S",
            "NDQ3S",
            "SG Issuer SA War 2022- Without fixed mat on ...",
            "fund etf",
            "Euronext Paris",
        ),
        ("USD", "NDQUSD", "US Tech (NDQ) / US Dollar", "index cfd", "easyMarkets"),
    ];

    let mut filters = div().flex().items_center().gap_2();
    for (idx, label) in search_filters.iter().enumerate() {
        let active = idx == 0;
        let bg = if active { rgb(0x1f2937) } else { rgb(0x111827) };
        let text = if active { rgb(0xffffff) } else { rgb(0x9ca3af) };
        filters = filters.child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(bg)
                .text_xs()
                .text_color(text)
                .child(*label),
        );
    }

    let mut results_list = div()
        .flex()
        .flex_col()
        .flex_1()
        .bg(rgb(0x0b1220))
        .border_1()
        .border_color(rgb(0x1f2937))
        .rounded_md()
        .h_full()
        .id("search-results")
        .overflow_y_scroll();
    for (idx, (badge, symbol, name, market, venue)) in search_results.iter().enumerate() {
        let active = idx == 0;
        let row_bg = if active { rgb(0x0f172a) } else { rgb(0x0b1220) };
        let border_color = if active { rgb(0x2563eb) } else { rgb(0x1f2937) };
        let close_row = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
            this.symbol_search_open = false;
            window.refresh();
        });

        let mut row = div()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .bg(row_bg)
            .on_mouse_down(MouseButton::Left, close_row)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(32.))
                            .h(px(32.))
                            .rounded_full()
                            .bg(rgb(0x1f2937))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(gpui::white())
                            .child(*badge),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().text_color(gpui::white()).child(*symbol))
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(0x1f2937))
                                            .text_xs()
                                            .text_color(rgb(0x9ca3af))
                                            .child(*market),
                                    ),
                            )
                            .child(div().text_xs().text_color(rgb(0x9ca3af)).child(*name)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child(*market)
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x1f2937))
                            .text_xs()
                            .text_color(gpui::white())
                            .child(*venue),
                    ),
            );

        row = if active {
            row.border_1().border_color(border_color)
        } else {
            row.border_b_1().border_color(border_color)
        };
        results_list = results_list.child(row);
    }

    let close_overlay = cx.listener(|this: &mut ChartView, _: &MouseDownEvent, window, _| {
        this.symbol_search_open = false;
        window.refresh();
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
            .p_3()
            .child(
                div()
                    .w(px(POPUP_WIDTH))
                    .h(px(POPUP_HEIGHT))
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1f2937))
                    .rounded_md()
                    .shadow_lg()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .h_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child("Symbol Search"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.))
                                            .h(px(24.))
                                            .rounded_full()
                                            .bg(rgb(0x1f2937))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_xs()
                                            .text_color(gpui::white())
                                            .on_mouse_down(MouseButton::Left, close_overlay)
                                            .child("X"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0x1f2937))
                                    .bg(rgb(0x111827))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x9ca3af))
                                            .child("Search"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::white())
                                            .child("NDQ"),
                                    ),
                            )
                            .child(filters)
                            .child(results_list)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x6b7280))
                                    .child("Search using ISIN and CUSIP codes"),
                            ),
                    ),
            ),
    )
}
