# Next Step

- Wire the UI/app to use core's public API: load CSV/Parquet via `core::load_csv` / `core::load_parquet`, apply `resample`/`bounds`, and feed the data into a gpui candlestick view.
- Remove the temporary `add` stub once the UI/app compile against the new core API.
