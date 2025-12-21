# Milestone: Core loaders

Date: 2025-12-21
Scope: core crate

What changed

- Added `Candle` model (timestamp + OHLCV) and column mapping/load options with defaults.
- Implemented `load_csv` and `load_parquet` via polars lazy readers, validating column presence/length.
- Converted timestamps from ns/us/ms epochs, date days, or RFC3339 strings to `OffsetDateTime`.
- Converted numeric fields to f64 with clear errors, and guard against inverted high/low.
- Kept existing `add` stub temporarily for app compatibility until UI wiring uses loaders.

Next steps

- Add loader unit tests with sample CSV/Parquet fixtures.
- Expose optional resampling/aggregation and bounds helpers for chart scaling.
- Wire app/ui to invoke loaders and pass data into gpui state/controls.
