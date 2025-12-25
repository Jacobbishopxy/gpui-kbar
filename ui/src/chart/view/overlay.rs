use gpui::{Div, div, prelude::*, px, rgb};

use super::ChartView;

impl ChartView {
    pub(super) fn tooltip_overlay(&self, start: usize, end: usize) -> Option<Div> {
        let (idx, (mx, my), bounds) = (self.hover_index?, self.hover_position?, self.chart_bounds?);
        let candle = self.candles.get(idx)?;
        if idx < start || idx >= end {
            return None;
        }

        let origin_x = f32::from(bounds.origin.x);
        let origin_y = f32::from(bounds.origin.y);
        let max_x = origin_x + f32::from(bounds.size.width);
        let max_y = origin_y + f32::from(bounds.size.height);
        let mut x = mx + 12.0;
        let mut y = my + 12.0;
        let tip_width = 180.0;
        let tip_height = 88.0;
        if x + tip_width > max_x {
            x = (max_x - tip_width).max(origin_x);
        }
        if y + tip_height > max_y {
            y = (max_y - tip_height).max(origin_y);
        }

        let ts = candle.timestamp;
        let idx_line = format!("#{idx}");
        let o_line = format!("O: {:.4}", candle.open);
        let h_line = format!("H: {:.4}", candle.high);
        let l_line = format!("L: {:.4}", candle.low);
        let c_line = format!("C: {:.4}", candle.close);
        let v_line = format!("V: {:.2}", candle.volume);

        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .bg(rgb(0x111827))
                .border_1()
                .border_color(rgb(0x1f2937))
                .rounded_md()
                .shadow_lg()
                .p_2()
                .text_xs()
                .flex()
                .flex_col()
                .gap_1()
                .child(ts.to_string())
                .child(idx_line)
                .child(o_line)
                .child(h_line)
                .child(l_line)
                .child(c_line)
                .child(v_line),
        )
    }
}
