# Loading Spinner Stall: Root Cause + Fix + Prevention

## Summary

The `loading_sand` spinner was not actually “blocked” by the CSV/resample load. Instead, the window only produced a handful of frames during each load because we were *forcing continuous repaints* while simultaneously doing *very expensive per-frame rendering* (painting thousands of candles every frame and rebuilding render state).

Result: the loading overlay existed, but it only repainted ~6–11 times over ~5–6 seconds, so the spinner appeared frozen.

## Symptoms (what we observed)

- Each symbol load took ~5–6 seconds.
- While `loading_symbol` was `Some(...)`, logs showed:
  - only ~6–11 `[loading_sand] animation tick ...` lines
  - `pump_frames` stayed in the same range
- Minimal dev repros (spinner + background sleep) animated smoothly the whole time.

This combination indicates **frame starvation / low FPS**, not that the async load was blocking the UI thread.

## Why `dev/src/bin/loading_spinner_repro.rs` was smooth

That repro animates smoothly because each frame is cheap:

- it renders the spinner + a small layout
- it does **not** repaint an O(N) chart scene on every refresh

In other words, the scheduler was fine; the runtime was simply doing too much work per frame.

## Root cause (what was “stalling”)

In the main runtime, while `loading_symbol.is_some()` we were intentionally calling RAF + `refresh()` continuously so the overlay could animate:

- `ui/src/chart/view/render.rs` requests `window.request_animation_frame()` and `window.refresh()` while loading

But the same render pass also always did heavy work:

- `RenderState::from_view(self)` creates `visible.to_vec()` then `Arc<[Candle]>` each render (alloc + copy).
- `chart_canvas` and `volume_canvas` paint loops draw per-candle primitives (O(candles) work per frame).

So the system was asked to repaint continuously while each repaint was expensive → the window could only produce a few frames → the spinner advanced only a few steps.

## Fix (what we changed)

### 1) Skip heavy chart rendering while the blocking loading overlay is visible

File: `ui/src/chart/view/render.rs`

- Added a loading fast-path gated by `SKIP_CHART_RENDER_WHILE_LOADING`.
- When `loading_symbol.is_some()`, we still request RAF + refresh (so the overlay can animate), but we **early-return a minimal view** that only shows the loading overlay (no `RenderState::from_view`, no chart canvases).

This keeps the per-frame cost low while loading, so the spinner stays smooth.

### 2) Persist logs per run (for repeatable debugging)

File: `ui/src/logging.rs`

- `log_loading(...)` now writes to a unique per-run file: `tmp/loading_spinner_<epoch_ms>_pid<pid>.log`.
- This prevents logs from being overwritten and makes “before/after” comparisons easy.

### 3) Dev repro for “heavy paint starves spinner”

File: `dev/src/bin/loading_spinner_heavy_paint.rs`

- Reproduces the stall by painting thousands of synthetic “candles” each frame while the overlay is animating.
- Toggle `SKIP_HEAVY_WHILE_LOADING` to see the effect immediately.

## How to avoid this problem again (rules of thumb)

### Don’t do expensive work on every frame

- Avoid per-frame allocations/copies (e.g., `visible.to_vec()` inside `render()`).
- Avoid O(N) paint loops for large N on every refresh, especially if N can be thousands.
- Cache derived render state and only rebuild it when inputs change (new data, interval change, zoom change, window resize, etc.).

### Be careful with “keep repainting while loading”

Forcing continuous RAF + `refresh()` is fine **only if the render path is cheap**.

If you need a spinner during a long async task:

- Either render only a minimal overlay while loading (what we do now), or
- Freeze/snapshot the expensive background content (render it once, reuse it), or
- Degrade expensive content during loading (downsample/decimate, lower fidelity).

### Respect GPUI frame-phase constraints

`Window::request_animation_frame()` can only be called during GPUI’s layout/paint phases (it panics if called from the wrong context). Keep RAF requests inside render/paint-related code paths; use `window.refresh()` from event handlers/pumps.

## Debug checklist (next time something “stalls”)

1. Run the app and open the newest `tmp/loading_spinner_*.log`.
2. Compare:
   - load duration (start → finish)
   - number of `[loading_sand]` ticks while loading
3. If ticks are tiny (e.g., ~10 over seconds), it’s almost always **frame starvation**.
4. Use `dev/src/bin/loading_spinner_heavy_paint.rs` to validate whether “heavy paint + continuous refresh” reproduces the behavior.

