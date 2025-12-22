# Milestone: Core module refactor

Date: 2025-12-21
Scope: core crate

What changed

- Split core into modules: types (Candle/ColumnMapping/LoadOptions/Interval), load (CSV/Parquet + parsing/validation), resample (bounds + interval aggregation), error (LoadError).
- Kept lib exports stable for callers.
- Tests updated to use new module layout; `cargo test -p core` passes.
- UI/app now load via core loaders; UI renders a minimal candlestick view (optional one-time resample).

Next steps

- Add interactive controls for interval/pan/zoom: track zoom/offset, hook drag/scroll handlers, and apply transforms in the render path.
- Implement hover tooltips with hit-testing of candle bodies/wicks; render an overlay with candle data near the cursor.
- Draw axes/labels (price/time) alongside existing gridlines to improve readability.
- Surface load/empty-data errors in the UI (not just CLI bailouts); render an error view when loaders fail.
- Add an in-app interval selector wired to `core::resample` so users can switch between raw/1m/5m/etc without restarting.
