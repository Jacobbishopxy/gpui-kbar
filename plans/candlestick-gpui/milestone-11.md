# Milestone: Live ingest harness + hardening

Date: 2026-01-11
Scope: data/core/app runtime

Context

Milestone 7 implemented the basic end-to-end live path (schema consumption via `flux-schema`, live subscribe/backfill, DuckDB cache + cursor persistence). This milestone collects the remaining follow-ups that were originally tracked under milestone 7.

Status

- [x] Harness/demo publishes FlatBuffers over ZMQ and verifies DuckDB round-trip.

Notes

- Mock service: `dev/src/bin/flux_mock_replay.rs`
- Round-trip test: `ui/tests/live_roundtrip.rs`

Backend notes (Flux)

- Backend repo: `../flux` (see `../flux/docs/plan.md` and `plans/runtime-plugins-live-data/live-data-bridge.md` for the contract).
- Wire schema source of truth: `../flux/crates/flux-schema/schemas/market_data.fbs`.
- Expected sockets (defaults used by the app): `tcp://127.0.0.1:5556` (PUB) and `tcp://127.0.0.1:5557` (REQ/REP).
