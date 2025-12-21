# Milestone: Core resample & bounds

Date: 2025-12-21
Scope: core crate

What changed

- Added Interval type and duration helper to support minute/hour/day bucket sizes.
- Implemented bounds calculation across candles and a resample function that aggregates OHLCV per interval.
- Added unit test covering bounds and resampling behavior.

Status

- Core now has loaders + validation, bounds, and resampling with tests. Doctests disabled to avoid std `core` name clash.

Next steps

- Expose a public API surface (e.g., a module re-export) for UI/app to request column mapping/load options and resampling.
- Wire loaders/resampling into UI/app, driving gpui chart state and controls.
