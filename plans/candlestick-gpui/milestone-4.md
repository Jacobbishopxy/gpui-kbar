# Milestone: Core module refactor

Date: 2025-12-21
Scope: core crate

What changed

- Split core into modules: types (Candle/ColumnMapping/LoadOptions/Interval), load (CSV/Parquet + parsing/validation), resample (bounds + interval aggregation), error (LoadError).
- Kept lib exports stable for callers.
- Tests updated to use new module layout; `cargo test -p core` passes.
- UI/app now load via core loaders; UI renders a minimal candlestick view (optional one-time resample).

Next steps

- Re-exported concise public API and removed the temporary `add` stub; UI/app call loaders/resampler.
- Wired UI/app to load CSV/Parquet, compute bounds, optional resample, and render via gpui.
- Re-introduce interactions (hover tooltip, pan/zoom, interval controls) incrementally with careful hit-testing; add axes/labels and error surfacing once rendering is stable.
