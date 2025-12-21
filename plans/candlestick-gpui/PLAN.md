# Candlestick gpui Plan

Goal: use latest gpui to render an interactive candlestick chart that loads OHLCV data from CSV or Parquet.

Workspace split

- core: data model (Candle, times), loaders for CSV/Parquet, optional resampling/indicators, error handling.
- ui: gpui views for chart + controls (file path status, timeframe selector), drawing on a canvas with pan/zoom + hover tooltip.
- bin: entrypoint wiring CLI args to loaders and launching the gpui runtime.

Dependencies to add (once coding)

- gpui latest (git dep: `gpui = { git = "https://github.com/zed-industries/zed", package = "gpui" }`).
- polars for CSV/Parquet (`features = ["lazy", "csv", "parquet", "dtype-datetime"]`); fallback to `csv` + `parquet` crates if lighter.
- time/chrono for timestamps; anyhow/thiserror for ergonomic errors; clap for CLI args.

Core plan

1) Define `Candle { ts: OffsetDateTime, open, high, low, close, volume }` plus conversion helpers.
2) Implement loaders: `load_csv(path, opts)` and `load_parquet(path, opts)` returning `Vec<Candle>`; infer/allow column mappings; validate OHLC ordering.
3) Provide optional resample/aggregate by interval (e.g., 1m/5m/1h) and compute bounds for chart scaling.

UI plan

1) Bootstrap gpui app with state (candles, viewport, zoom/pan, hover index).
2) Build a chart view that draws wicks/bodies, axes/gridlines, and hover crosshair/tooltip; reuse computed bounds from core.
3) Wire interactions: scroll to zoom, drag to pan, resize handling, keyboard shortcuts, click-to-select candle.
4) Add basic controls (open file button or CLI path display, timeframe dropdown) and error banner when load fails.

Bin plan

1) Parse CLI args: path, format (auto-detect by extension), optional start/end range and interval.
2) Call loaders, then launch gpui app passing the prepared data/state.
3) Log progress/errors to stdout/stderr; exit with non-zero on failed load.

Testing & samples

- Add sample CSV/Parquet fixtures for tests; unit-test loaders + resampler + bounds calculation.
- Smoke test that `bin` builds and launches with sample data; consider screenshot/Golden test for rendering later.

Open considerations

- Decide on timezone handling (assume UTC initially) and data column naming conventions.
- Performance: chunked loading for large files; reuse buffers when redrawing.
