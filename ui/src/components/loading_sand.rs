use std::{panic::Location, time::Duration};

use gpui::{Animation, AnimationExt, Div, ElementId, Rgba, div, prelude::*, px, svg};

#[track_caller]
pub fn loading_sand(size: f32, color: Rgba) -> impl IntoElement {
    let animation = Animation::new(Duration::from_millis(900)).repeat();
    let id = ElementId::CodeLocation(*Location::caller());

    div()
        .w(px(size))
        .h(px(size))
        .with_animation(id, animation, move |this: Div, delta| {
            let (frame_one, frame_two) = frame_opacities(delta.clamp(0.0, 1.0));
            this.child(
                svg()
                    .path("time-sand-1.svg")
                    .w(px(size))
                    .h(px(size))
                    .text_color(color)
                    .opacity(frame_one),
            )
            .child(
                svg()
                    .path("time-sand-2.svg")
                    .w(px(size))
                    .h(px(size))
                    .text_color(color)
                    .opacity(frame_two),
            )
        })
}

fn frame_opacities(delta: f32) -> (f32, f32) {
    if delta < 0.5 {
        let t = delta / 0.5;
        (1.0 - t, t)
    } else {
        let t = (delta - 0.5) / 0.5;
        (t, 1.0 - t)
    }
}
