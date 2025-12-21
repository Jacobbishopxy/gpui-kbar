# Milestone: Core module refactor

Date: 2025-12-21
Scope: core crate

What changed

- Split core into modules: types (Candle/ColumnMapping/LoadOptions/Interval), load (CSV/Parquet + parsing/validation), resample (bounds + interval aggregation), error (LoadError).
- Kept lib exports stable for callers and retained temporary `add` stub.
- Tests updated to use new module layout; `cargo test -p core` passes.

Next steps

- Re-export a concise public API for UI/app (already done in lib) when wiring gpui chart.
- Wire UI/app to use loaders, bounds, and resampling.
