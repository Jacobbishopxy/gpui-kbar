# Milestone: Loading overlay responsiveness

Date: 2026-01-08
Scope: ui runtime / chart rendering + loading UX

Goals

- Keep the loading overlay spinner animating during real symbol loads.
- Ensure the UI thread stays responsive while loads happen on background executors.
- Avoid per-frame allocations / expensive paint work that can starve frames during loading.

Result

The “spinner stall” was caused by frame starvation, not by CSV/resample blocking the UI thread.

While `loading_symbol` was set we were forcing continuous repaint/RAF so the overlay could animate,
but the render path was still doing expensive work (chart state rebuild + painting thousands of
candles). Each frame took long enough that only a handful of frames were produced over several
seconds, making the spinner appear frozen.

Fix

- Loading fast-path: while `loading_symbol.is_some()` the view requests frames but skips heavy chart
  rendering and returns a minimal root + loading overlay.
  - File: `ui/src/chart/view/render.rs`
- Removed a major per-frame allocation: `RenderState::from_view` no longer does
  `Arc::from(visible.to_vec())`. Canvases now paint from the full `Arc<[Candle]>` plus visible
  `(start,end)` indices (slice in paint).
  - Files: `ui/src/chart/view/render.rs`, `ui/src/chart/canvas.rs`
- All temporary debug logging/instrumentation used during investigation was removed.

Follow-up (2026-01-09)

- Reduced “time to first render” on real symbol loads:
  - CSV/Parquet loader now projects only the required columns and uses typed column access (`core/src/load.rs`).
  - Avoid eager resampling of historical intervals when switching symbols; only load `None` + current interval, compute others lazily (`ui/src/chart/view/state.rs`).
  - DuckDB cache writes no longer block symbol load completion; writes happen in the background and use DuckDB `Appender` for bulk inserts (`ui/src/chart/view/state.rs`, `core/src/store.rs`).

Status

- [x] Loading overlay spinner animates during real symbol loads (manual validation).
- [x] No off-phase RAF panics (RAF only requested from render).
- [x] No per-frame visible candle clone/allocation (slice in paint).
- [x] Investigation notes captured in `docs/wiki/loading-spinner-stall.md`.

Next up

1) Reduce paint cost when candle_count >> pixel_width (decimation / 1 candle per pixel column).
2) Cache expensive derived values (e.g., visible-range min/max) so zoom/pan doesn’t recompute
   everything every frame.
3) Add a small perf/regression harness under `dev/` to guard against reintroducing per-frame clones.
