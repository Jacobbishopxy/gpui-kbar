# Loading Spinner Stall: Root Cause + Fix + Prevention

## Summary

The `loading_sand` spinner was not blocked by CSV/resample work. The UI was simply producing very few frames during loading because we were forcing continuous repaints while also doing expensive per-frame rendering (chart state rebuild + painting thousands of candles each frame).

## Symptoms

- Loads take seconds, but the spinner advances only a handful of frames (looks frozen).
- Minimal dev repros (spinner + background sleep) animate smoothly.

This pattern is almost always **frame starvation / low FPS**, not an async-task blocking the UI thread.

## Why `dev/src/bin/loading_spinner_repro.rs` was smooth

That repro is smooth because each frame is cheap (spinner + small layout), so continuous refresh/RAF doesn’t overload painting.

## Root cause

While `loading_symbol.is_some()`, we intentionally requested RAF + `refresh()` so the overlay could animate. But the render pass still did heavy work each frame:

- `RenderState::from_view(self)` rebuilds visible candle data (`visible.to_vec()` → `Arc<[Candle]>`) every render.
- `chart_canvas` and `volume_canvas` paint per-candle primitives (O(candles) work per frame).

So the overlay existed, but each “next frame” was expensive → very low FPS → spinner appears stalled.

## Fix

### Skip heavy chart rendering while the blocking loading overlay is visible

File: `ui/src/chart/view/render.rs`

- Added a loading fast-path (`SKIP_CHART_RENDER_WHILE_LOADING`).
- When `loading_symbol.is_some()`, we still request frames, but we early-return a minimal view that only renders the loading overlay (no chart canvases, no `RenderState::from_view`).

This keeps per-frame cost low while loading, so the spinner stays smooth.

### Dev repro: heavy paint starves spinner

File: `dev/src/bin/loading_spinner_heavy_paint.rs`

- Paints thousands of synthetic “candles” each frame while showing the overlay.
- Toggle `SKIP_HEAVY_WHILE_LOADING` to demonstrate the mitigation.

## How to avoid this again

- Avoid per-frame allocations/copies in `render()` paths (especially proportional to data size).
- Avoid O(N) paint loops on every refresh when N can be thousands; cache or downsample.
- If you must show an overlay during long work, render a minimal scene (or freeze a snapshot) while loading.

## Debug checklist

1. If a spinner “stalls”, assume frame starvation.
2. Profile the render path for work that scales with data size (clones, allocations, per-item paint).
3. Temporarily hide/skip expensive content while loading.
4. Validate with `dev/src/bin/loading_spinner_heavy_paint.rs`.
