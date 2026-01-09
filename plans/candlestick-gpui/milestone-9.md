# Milestone: Chart render performance pass

Date: 2026-01-08
Scope: ui chart rendering

Goals

- Keep interactions (pan/zoom/hover) smooth on large datasets (10k–500k candles).
- Ensure performance scales primarily with pixels, not candle count.
- Prevent regressions (avoid accidental per-frame clones/allocations).

Context

We fixed the loading overlay “stall” by skipping heavy chart rendering while loading and by
removing the per-frame `visible.to_vec()` allocation. The next bottleneck is raw paint cost:
`chart_canvas` and `volume_canvas` are O(N) per frame today.

Plan

1) Decimate drawing when candles >> pixels
   - For each pixel column, draw at most one candle (or min/max wick + last close).
   - Target: O(width_px) paint complexity.
   - Files: `ui/src/chart/canvas.rs`, potentially `ui/src/chart/view/render.rs` to pass in pixel width.
   - Status: implemented by grouping visible candles into ~1-per-pixel columns and slicing in paint.

2) Cache visible-range aggregates
   - Maintain min/max/volume max for the current visible window, update incrementally on pan/zoom.
   - Avoid rescanning the full visible slice each frame.
   - Status: implemented a cache keyed by (data revision, visible start/end, pixel columns) and reuse it for hover-driven refreshes.

3) Optional: pre-bake geometry
   - Move repeated per-candle path building into cached vertex buffers if GPUI/blade benefits.

Validation

- Add a `dev` bin that renders a synthetic dataset with configurable candle count and measures
  perceived FPS during pan/zoom.
- Run on Windows with a “slow symbol” and verify consistent responsiveness.

Status

- [x] Decimated paint (O(width_px)).
- [x] Visible-range aggregate cache (reuse padded min/max + aggregated columns).
- [x] Time-axis label cache (avoid timestamp formatting every hover refresh).
- [x] Dev perf harness: `cargo run -p dev --bin chart_perf_harness -- --n 200000`.
- [x] Perf-mode header presets (50k/200k/1M) to regenerate synthetic candles without restarting.

Next up

- Integrate perf presets into the main runtime (app) and add a user settings popout panel (see `plans/candlestick-gpui/milestone-10.md`).
