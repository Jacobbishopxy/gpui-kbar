# Milestone: Core loader tests

Date: 2025-12-21
Scope: core crate

What changed

- Added CSV and Parquet fixture-based tests covering happy path and missing-column error handling.
- Parquet fixture uses real datetime column and polars writer to exercise timestamp parsing.
- Temp-file helpers ensure tests operate on real files and clean up after themselves.

Next steps

- Add resampling/bounds helpers in core for chart scaling.
- Expose public API surface for UI/app to request column mapping and load options.
- Wire app/ui to call loaders and surface errors to the gpui layer.
