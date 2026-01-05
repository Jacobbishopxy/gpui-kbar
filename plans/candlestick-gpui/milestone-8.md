# Milestone: Loading overlay responsiveness

Date: 2026-01-03
Scope: ui runtime / data loading

Goals

- Keep the loading overlay spinner animating during real symbol loads.
- Move CSV load, resample, and DuckDB/session writes off the UI thread while retaining persistence.
- Keep the chart replace_data path lightweight so the render loop is never blocked.
- Ensure the overlay animation stays fluid (request animation frames while loading).
- Move the entire load/persist/resample pipeline into a single background task and feed the UI ready data.

Plan

- Root cause: symbol loads ran DuckDB writes and resampling on the UI thread after the CSV task returned (ui/src/chart/view/state.rs).
- Move the full I/O path into the background executor: cache/CSV read, planned interval resamples, DuckDB writes, and session updates; return a LoadResult with base + resample cache.
- UI hop should only swap in Arc<[Candle]>, reuse pre-resampled intervals, refresh/request_animation_frame, and update watchlist/session without store locks.
- Keep frames flowing while loading: request_animation_frame when loading_symbol is set and give loading_sand an explicit rotation.
- Next validation: confirm spinner fluidity on slow symbols, widen resample coverage if interval changes mid-load still trigger UI work, and recheck persistence after restart.

Status

- [x] Store handle made Send (Arc<Mutex<_>>).
- [x] CSV load + DuckDB writes moved off the UI thread; store locks avoided on UI.
- [ ] Resample/prep moved off UI thread via background LoadResult (needs validation on interval changes).
- [ ] Overlay spinner animates during real symbol loads (manual check).
- [ ] Persistence verified after restart (manual check).

Notes from 2026-01-05 investigation

- Observed log: loading_sand ticks only before window.spawn finishes; no ticks during spawn processing. Example:
  - `[load] window.spawn scheduling symbol=US02Y at Instant { t: 95469.0365074s }`
  - `[loading_sand] animation tick delta=0.000 frame_log_count=1`
  - ...ticks stop until completion...
  - `[load] window.spawn completed symbol=US02Y after 2.9239127s`
- Root cause: UI thread does work after background task completes:
  - `replace_data_from_load` can resample on UI when the current interval is missing from the precomputed cache.
  - `apply_range_index` + `persist_session`/`persist_viewport` lock DuckDB on the UI thread.
- Possible solutions:
  - Precompute resamples for all needed intervals (e.g., QUICK_RANGE_WINDOWS + current) inside the background task so the UI never resamples.
  - Move session/watchlist persistence to a background task after the UI state swap.
  - Keep `request_animation_frame` while loading (already present) to ensure frames are scheduled.
- 2026-01-06 follow-up:
  - Instrumentation shows UI hop is tiny (<200µs) and replace_data_from_load ~15µs, so stalls are not from foreground work.
  - Frames stop despite request_animation_frame; added window.refresh while loading to force frame scheduling on Windows.
  - Mixed results: some loads still stall animation (first/second loads), others animate; likely need a state-driven pattern to avoid ad-hoc pumping.

## TODO

State-driven animation-safe pattern (recommended)

Never mutate state directly in background threads.

Instead:

```rs
enum Msg {
    Start,
    Finished(Data),
}

impl View for MyView {
    fn update(&mut self, cx: &mut ViewContext<Self>, msg: Msg) {
        match msg {
            Msg::Start => {
                cx.spawn(|cx| async move {
                    let data = blocking_io();
                    cx.update(|cx| cx.emit(Msg::Finished(data)));
                });
            }
            Msg::Finished(data) => {
                self.data = Some(data);
            }
        }
    }
}
```

This keeps:

animations smooth

state changes deterministic

rendering predictable

Notes from 2026-01-07 state-driven pass

- Manual runs still showed the spinner freezing on the first/second symbols while the third animated (e.g., US02Y/US10Y stall, BTCUSD spins; later runs stalled entirely), with occasional gpui panics when request_animation_frame was called outside paint.
- Implemented a self-emitted load pipeline: ChartView now implements `EventEmitter<LoadMsg>` and subscribes to itself; `start_symbol_load` only emits `LoadMsg::Start`, and background work emits `LoadMsg::Finished`.
- All load state changes now flow through `handle_load_event`, with background CSV/cache/resample/session writes staying on the background executor. Finished events carry a `load_id` to drop stale completions.
- UI hop swaps precomputed arcs and spawns persistence off-thread; a light `on_next_frame` pump plus render-time RAF keep frames alive while `loading_symbol` is set.
- Next validation: rerun runtime to confirm spinner ticks for every symbol load and that no off-paint animation calls panic. If stalls persist, try dropping the explicit pump to isolate RAF vs. refresh behavior.
