use std::{panic::Location, time::Duration};

use gpui::{Animation, AnimationExt, Div, ElementId, Rgba, div, prelude::*, px, svg};

const FLIP_DURATION: Duration = Duration::from_millis(1200);
const HOLD_PORTION: f32 = 0.35;
const DIM_FACTOR: f32 = 0.92;

#[track_caller]
pub fn loading_sand(size: f32, color: Rgba) -> impl IntoElement {
    let animation = Animation::new(FLIP_DURATION).repeat();
    let id = ElementId::CodeLocation(*Location::caller());
    let dimmed = dim_color(color, DIM_FACTOR);

    div().w(px(size)).h(px(size)).relative().with_animation(
        id,
        animation,
        move |this: Div, delta| {
            let (frame_one, frame_two) = frame_opacities(delta.clamp(0.0, 1.0));
            this.child(
                svg()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .path("time-sand-1.svg")
                    .w(px(size))
                    .h(px(size))
                    .text_color(color)
                    .opacity(frame_one),
            )
            .child(
                svg()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .path("time-sand-2.svg")
                    .w(px(size))
                    .h(px(size))
                    .text_color(dimmed)
                    .opacity(frame_two),
            )
        },
    )
}

fn frame_opacities(delta: f32) -> (f32, f32) {
    let clamped = delta.clamp(0.0, 1.0);
    let half_phase = if clamped < 0.5 {
        clamped * 2.0
    } else {
        (clamped - 0.5) * 2.0
    };
    let fade_start = HOLD_PORTION;
    let fade_range = (1.0 - HOLD_PORTION).max(0.0001);
    let fade = ((half_phase - fade_start) / fade_range).clamp(0.0, 1.0);

    if clamped < 0.5 {
        (1.0 - fade, fade)
    } else {
        (fade, 1.0 - fade)
    }
}

fn dim_color(color: Rgba, factor: f32) -> Rgba {
    let clamp = |v: f32| v.clamp(0.0, 1.0);
    Rgba {
        r: clamp(color.r * factor),
        g: clamp(color.g * factor),
        b: clamp(color.b * factor),
        a: color.a,
    }
}
