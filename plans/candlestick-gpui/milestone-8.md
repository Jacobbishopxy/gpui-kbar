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
