# Live Data Dev (Milestone 7)

This repo consumes live candle batches over ZeroMQ + FlatBuffers using `flux-schema`.

## Backend (Flux)

- Repo: `../flux`
- Run: `cargo run -p flux-service`
- Endpoints (override as needed):
  - `FLUX_LIVE_PUB` (bind) default: `tcp://0.0.0.0:5556`
  - `FLUX_CHUNK_REP` (bind) default: `tcp://0.0.0.0:5557`
- Wire schema: `../flux/crates/flux-schema/schemas/market_data.fbs`

## Local Mock Service (This Repo)

- Run: `cargo run -p dev --bin flux_mock_replay`
- Purpose: lightweight PUB+REP server that replays `data/candles/*.csv` via FlatBuffers and serves backfill.

## Client (gpui-kbar)

- Run: `cargo run -p app`
- Optional env vars:
  - `FLUX_LIVE_PUB` (connect) default: `tcp://127.0.0.1:5556`
  - `FLUX_CHUNK_REP` (connect) default: `tcp://127.0.0.1:5557`
  - `FLUX_SOURCE_ID` default: `SIM`
  - `FLUX_INTERVAL` default: `1s`
