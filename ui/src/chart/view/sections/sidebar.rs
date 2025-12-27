use gpui::{Div, SharedString, div, prelude::*, px, rgb};

use crate::chart::view::{widgets::{header_chip, stat_row}, SIDEBAR_WIDTH};

pub fn sidebar(
    watchlist_panel: Div,
    instrument_card: Div,
    trading_stub: Div,
) -> Div {
    div()
        .w(px(SIDEBAR_WIDTH))
        .bg(rgb(0x0b1220))
        .border_l_1()
        .border_color(rgb(0x1f2937))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(watchlist_panel)
        .child(instrument_card)
        .child(trading_stub)
}
