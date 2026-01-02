# Milestone: Live ZMQ ingest with FlatBuffers + DuckDB cache

Date: 2025-12-28
Scope: data/core/app runtime

Goals

- Ingest real-time candle/tick batches from a ZMQ publisher with reconnect/backpressure handling.
- Serialize/deserialize market data via a FlatBuffers schema with versioning and tests; share types with the core/app.
- Persist streamed data into DuckDB for caching, resampling, and warm starts; enable offline playback from the cache.
- Keep the UI/resampler fed via a unified loader that merges live streams with cached/backfilled data.

Plan

- Define a FlatBuffers schema for candle/tick/trade messages (symbol, interval, source timestamp, sequence, payload), and check in generated Rust code or a build script to regenerate.
- Add a ZMQ subscriber service (async) with configurable endpoints, heartbeat/reconnect/backoff, drop detection, and minimal metrics/logging; feed frames into FlatBuffers decode.
- Implement an ingestion pipeline that validates ordering/sequence, deduplicates, and converts decoded structs into core Candle types while updating a rolling in-memory window and dispatching to the UI/resampler.
- Introduce a DuckDB store: DDL for candles/trades; append-only ingestion; periodic checkpoints/compaction; queries to hydrate memory on start and to backfill gaps.
- Provide a unified loader API: start from DuckDB cache if present, then stream from ZMQ and write through to DuckDB; support resume from the last persisted cursor/sequence.
- Add a test/demo harness: a publisher that replays sample CSV via ZMQ using FlatBuffers; an integration test that covers decode -> ingest -> DuckDB persistence and reload.
- Polish watchlist panel UI (scroll container with max size, ellipsis overflow, consistent controls, updated close icon).
- Improve UI readability and performance:
  - [x] Avoid cloning candles in render/canvas: pass slices/Arc<[Candle]> into chart/volume canvases and reuse buffers instead of cloning per frame.
  - [x] Avoid cloning the full symbol universe on every search render: borrow and filter with iterators or cache per-filter subsets.
  - [x] Reduce `render` monolith size: split header/watchlist/instrument/overlays into helpers to simplify future changes.
  - [x] Reuse resampled data: keep `base_candles` in Arc and cache interval resamples to avoid full clones on every interval switch or replace.

Status

- [ ] FlatBuffers schema defined and codegen integrated into the build.
- [ ] ZMQ subscriber service wired into the app/core data path with reconnect/heartbeat.
- [ ] DuckDB-backed cache persists streamed data and hydrates on startup.
- [ ] Unified loader merges cache + live stream and feeds the UI/resampler.
- [ ] Harness/demo publishes FlatBuffers over ZMQ and verifies DuckDB round-trip.
- [ ] Watchlist panel UI polish (scroll, sizing, overflow, close icon).
- [x] Local symbol search now hydrates from `data/universe.csv` + `mapping.csv`, with per-symbol candle generation via `scripts/generate_kbar.py`.
