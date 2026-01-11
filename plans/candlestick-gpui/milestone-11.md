# Milestone: Live ingest harness + hardening

Date: 2026-01-11
Scope: dev/ui/app live runtime

Context

Milestone 7 implemented the basic end-to-end live path (schema consumption via `flux-schema`, live subscribe/backfill, DuckDB cache + cursor persistence). This milestone collects follow-ups so live data is practical to develop and resilient to gaps.

Goals

- Make the dev mock server a GUI controller (start/stop, choose symbol(s), adjust tick/batch, inject gaps).
- Add an explicit "live watermark" (timestamp + sequence) contract so both server and client agree what "live right now" means.
- Make the client resilient: detect missing sequences, backfill while still receiving live, then merge/dedup and persist.
- Improve UX visibility: footer indicator reflects connection/receive health (not just `live_mode=true`).

Plan

1) Dev server: add a GPUI control panel binary
   - Add `dev/src/bin/flux_dev_server.rs` (new) that runs a ZMQ PUB+REP service plus a GPUI window.
   - UI controls:
     - Start/Stop broadcasting.
     - Endpoints: PUB + REP sockets (defaults match the app).
     - Stream: `source_id`, `interval`, and symbol selector populated from `data/data.duckdb` (`universe`) with `data/universe.csv` fallback.
     - Rate controls: `tick_ms`, `batch_size`.
     - Fault injection: optional "skip sequences" / "drop N%" / jitter (to test client gap handling).
   - Server behavior:
     - Maintain monotonic per-stream `sequence` and the latest candle timestamp (the "watermark").
     - Persist generated/replayed candles in memory (at minimum) so REP backfill can serve missing ranges.

2) Schema: add "stream status / watermark" messages in `flux-schema`
   - Extend `../flux/crates/flux-schema/schemas/market_data.fbs` with:
     - `StreamStatusRequest { key }`
     - `StreamStatusResponse { key, latest_sequence, latest_ts_ms, server_time_ms }`
   - Generate/update Rust bindings in `flux-schema` and bump/propagate `WIRE_SCHEMA_VERSION`.
   - Compatibility: keep existing CandleBatch + BackfillCandlesRequest/Response unchanged so older clients still work if they ignore status.

3) Client: implement a live coordinator that can backfill concurrently
   - Update `ui/src/live.rs`:
     - Add encode/decode helpers for `StreamStatus*`.
     - Return full backfill metadata (start_sequence, candles, has_more, next_sequence) so backfill can be chunked.
   - Add a "live coordinator" layer (new module or inside `ChartView`) that:
     - Starts SUB immediately and buffers CandleBatch events.
     - Fetches the server watermark via REP (status request) to define the backfill target.
     - Runs backfill from the persisted cursor up to the target while SUB continues receiving.
     - Merges: apply backfill first, then drain buffered live batches in sequence order; dedup by timestamp (and sequence when available).
     - On detected gaps during steady-state (incoming `start_sequence` > expected), trigger a targeted backfill in the background and merge when ready.
     - Persists: update DuckDB candles + cursor after merges so restarts resume cleanly.

4) UX: make live health visible in the footer
   - Keep the existing blinking dot, but add explicit states:
     - disconnected/connecting/subscribed (idle)/receiving/stalled.
   - Show a short "last update age" label or tooltip (e.g., "Live • 0.3s ago", "Stale • 8.2s").

Validation

- Manual: run `cargo run -p dev --bin flux_dev_server` then `cargo run -p app`; switch Settings->Source->Live; verify the dot blinks on data and gap injection triggers backfill without freezing the chart.
- Automated: add unit/integration coverage for gap detection + backfill merge ordering (keep tests close to `ui/src/live.rs` / coordinator logic).

Status

- [x] CLI harness/demo publishes FlatBuffers over ZMQ and verifies DuckDB round-trip (`dev/src/bin/flux_mock_replay.rs`, `ui/src/live.rs`).
- [ ] GPUI dev server controller with fault injection.
- [ ] `flux-schema` adds stream status/watermark messages and versioning.
- [ ] Client gap handling: concurrent backfill + merge/dedup + cursor persistence.
- [ ] Footer shows connection/receive health + last-update age.

Backend notes (Flux)

- Backend repo: `../flux` (see `../flux/docs/plan.md` and `plans/runtime-plugins-live-data/live-data-bridge.md` for the contract).
- Wire schema source of truth: `../flux/crates/flux-schema/schemas/market_data.fbs`.
- Implement REP handlers on `tcp://127.0.0.1:5557`:
  - Backfill (existing): `BackfillCandlesRequest -> BackfillCandlesResponse`
  - Status (new): `StreamStatusRequest -> StreamStatusResponse` (watermark)
- Expected sockets (defaults used by the app): `tcp://127.0.0.1:5556` (PUB) and `tcp://127.0.0.1:5557` (REQ/REP).
