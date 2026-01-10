use gpui::{Div, Stateful, prelude::*, rgb};

pub fn apply(button: Stateful<Div>, base_bg: u32) -> Stateful<Div> {
    let (hover_bg, active_bg) = hover_and_active_bg(base_bg);
    apply_custom(button, hover_bg, active_bg)
}

pub fn apply_custom(button: Stateful<Div>, hover_bg: u32, active_bg: u32) -> Stateful<Div> {
    button
        .cursor_pointer()
        .hover(move |s| s.bg(rgb(hover_bg)))
        .active(move |s| s.bg(rgb(active_bg)))
        .on_hover(|_, window, _| window.refresh())
}

fn hover_and_active_bg(base_bg: u32) -> (u32, u32) {
    match base_bg {
        0x0b1220 => (0x0f172a, 0x020617),
        0x0f172a => (0x111827, 0x0b1220),
        0x111827 => (0x1f2937, 0x0f172a),
        0x1f2937 => (0x374151, 0x111827),
        _ => (tint(base_bg, 0.18), shade(base_bg, 0.18)),
    }
}

fn tint(color: u32, amount: f32) -> u32 {
    let (r, g, b) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let r = tint_channel(r as f32, amount) as u32;
    let g = tint_channel(g as f32, amount) as u32;
    let b = tint_channel(b as f32, amount) as u32;
    (r << 16) | (g << 8) | b
}

fn shade(color: u32, amount: f32) -> u32 {
    let (r, g, b) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let r = shade_channel(r as f32, amount) as u32;
    let g = shade_channel(g as f32, amount) as u32;
    let b = shade_channel(b as f32, amount) as u32;
    (r << 16) | (g << 8) | b
}

fn tint_channel(channel: f32, amount: f32) -> u8 {
    let amount = amount.clamp(0.0, 1.0);
    ((channel + (255.0 - channel) * amount)
        .round()
        .clamp(0.0, 255.0)) as u8
}

fn shade_channel(channel: f32, amount: f32) -> u8 {
    let amount = amount.clamp(0.0, 1.0);
    ((channel * (1.0 - amount)).round().clamp(0.0, 255.0)) as u8
}
