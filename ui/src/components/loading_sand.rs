use std::{
    panic::Location,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use gpui::{
    Animation, AnimationExt, Div, ElementId, Hsla, Rgba, Transformation, div, percentage,
    prelude::*, px, svg,
};
use gpui_component::{Sizable, Size as ComponentSize, spinner::Spinner};

const FLIP_DURATION: Duration = Duration::from_millis(1200);
const HOLD_PORTION: f32 = 0.35;
const DIM_FACTOR: f32 = 0.92;
const SPINNER_OPACITY: f32 = 0.75;
const SPINNER_SCALE: f32 = 0.9;
static FRAME_LOGS: AtomicU32 = AtomicU32::new(0);
const FRAME_LOG_CAP: u32 = u32::MAX;

/// Reset the logging counter so we can see fresh animation ticks.
pub fn reset_frame_logs() {
    FRAME_LOGS.store(0, Ordering::Relaxed);
}

#[track_caller]
pub fn loading_sand(size: f32, color: Rgba) -> impl IntoElement {
    let animation = Animation::new(FLIP_DURATION).repeat();
    let id = ElementId::CodeLocation(*Location::caller());
    let dimmed = dim_color(color, DIM_FACTOR);
    let spinner_color = Hsla::from(color).opacity(SPINNER_OPACITY);
    let spinner_size = ComponentSize::Size(px(size * SPINNER_SCALE));

    div().w(px(size)).h(px(size)).relative().with_animation(
        id,
        animation,
        move |this: Div, delta| {
            let (frame_one, frame_two) = frame_opacities(delta.clamp(0.0, 1.0));
            let rotation = Transformation::rotate(percentage(delta));
            let n = FRAME_LOGS.fetch_add(1, Ordering::Relaxed);
            if n < FRAME_LOG_CAP {
                println!(
                    "[loading_sand] animation tick delta={:.3} frame_log_count={}",
                    delta,
                    FRAME_LOGS.load(Ordering::Relaxed)
                );
            } else if n == FRAME_LOG_CAP {
                println!("[loading_sand] animation tick log cap reached ({FRAME_LOG_CAP})");
            }
            let mut this = this.child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .w(px(size))
                    .h(px(size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new().with_size(spinner_size).color(spinner_color)),
            );
            this = this
                .child(
                    svg()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .path("time-sand-1.svg")
                        .w(px(size))
                        .h(px(size))
                        .text_color(color)
                        .with_transformation(rotation)
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
                        .with_transformation(rotation)
                        .opacity(frame_two),
                );
            this
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
