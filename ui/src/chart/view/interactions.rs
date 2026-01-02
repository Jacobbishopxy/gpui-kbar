use gpui::{MouseButton, MouseMoveEvent, ScrollWheelEvent, Window, px};

use super::ChartView;

impl ChartView {
    pub(super) fn handle_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window) {
        if self.symbol_search_open || self.candles.is_empty() {
            return;
        }
        let delta = event.delta.pixel_delta(px(16.0));
        let scroll_y = f32::from(delta.y);
        if scroll_y.abs() < f32::EPSILON {
            return;
        }
        let center = self.view_offset + self.visible_len() * 0.5;
        let zoom_factor = if scroll_y < 0.0 { 1.1 } else { 0.9 };
        self.zoom = (self.zoom * zoom_factor).clamp(1.0, self.candles.len() as f32);
        let new_visible = self.visible_len();
        let new_offset = center - new_visible * 0.5;
        let visible_count = new_visible.round().max(1.0) as usize;
        self.view_offset = self.clamp_offset(new_offset, visible_count);
        let _ = self.persist_viewport();
        window.refresh();
    }

    pub(super) fn handle_hover(&mut self, event: &MouseMoveEvent, candle_count: usize) {
        if self.symbol_search_open {
            return;
        }
        if let (Some(bounds), true) = (
            self.chart_bounds,
            !self.candles.is_empty() && candle_count > 0,
        ) {
            let bx = f32::from(bounds.origin.x);
            let by = f32::from(bounds.origin.y);
            let bw = f32::from(bounds.size.width);
            let bh = f32::from(bounds.size.height);
            let px = f32::from(event.position.x);
            let py = f32::from(event.position.y);
            if px >= bx && px <= bx + bw && py >= by && py <= by + bh {
                let candle_width = (bw / candle_count as f32).max(1.0);
                let local_x = (px - bx).max(0.0);
                let local_idx = (local_x / candle_width).floor() as usize;
                let start_idx = self.visible_range().0;
                let idx = (start_idx + local_idx).min(self.candles.len().saturating_sub(1));
                self.hover_index = Some(idx);
                self.hover_position = Some((px, py));
            } else {
                self.hover_index = None;
                self.hover_position = None;
            }
        }
    }

    pub(super) fn handle_drag(&mut self, event: &MouseMoveEvent, window: &mut Window) {
        if self.symbol_search_open {
            self.dragging = false;
            self.last_drag_position = None;
            window.refresh();
            return;
        }
        if !self.dragging || self.candles.is_empty() {
            window.refresh();
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            self.dragging = false;
            self.last_drag_position = None;
            window.refresh();
            return;
        }

        if let Some((last_x, _)) = self.last_drag_position {
            let dx = f32::from(event.position.x) - last_x;
            let width = self
                .chart_bounds
                .map(|b| f32::from(b.size.width).max(1.0))
                .unwrap_or(1.0);
            let visible = self.visible_len();
            if visible > 0.0 {
                let candles_per_px = visible / width.max(1e-3);
                let new_offset = self.view_offset - dx * candles_per_px;
                let visible_count = visible.round().max(1.0) as usize;
                self.view_offset = self.clamp_offset(new_offset, visible_count);
            }
        }

        self.last_drag_position = Some((f32::from(event.position.x), f32::from(event.position.y)));
        window.refresh();
    }
}
