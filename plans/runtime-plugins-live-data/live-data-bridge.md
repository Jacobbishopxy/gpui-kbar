# Plan: Live Data Bridge (Server + Client) - Prereq for "Live Ingest Track"

Date: 2026-01-10
Scope: server-side market data service + client-side app integration (ZeroMQ + FlatBuffers)

Server repo location

- Current backend workspace: `../flux`
  - Crates: `flux-core`, `flux-schema`, `flux-io`, `flux-db`, `flux-service`, `flux-cli`, `flux-loadgen`.
  - Existing planning docs: `../flux/docs/plan.md` (ZMQ + FlatBuffers + TimescaleDB design), `../flux/docs/milestone-1.md`.

Problem statement

The app is a client-side UI. For live ingest we need a server-side component that:

1) Broadcasts live updates with ordering guarantees and reconnect support.
2) Serves missing/backfill data for warm starts, gap fill, and historical queries.

This document is the contract between the client app and the server service.

Goals

- One canonical data model and cursor semantics shared between streaming and backfill.
- Minimal operational footprint to get to end-to-end live + gap fill working.
- Clear separation of responsibilities: server handles ingestion, persistence, and query; client handles rendering + UX.

Non-goals (initially)

- User auth/entitlements, multi-tenant isolation, billing.
- Exchange connectivity breadth (start with one feed or a replay harness).
- Exactly-once delivery end-to-end (we target at-least-once + idempotent apply).

Shared contract (client <-> server)

1) Identity & keys

- `source_id`: identifies the feed (e.g., `SIM`, `BINANCE_SPOT`, `IBKR`, etc.)
- `symbol`: canonical string (aligned with `data/symbols.csv` mapping if applicable)
- `interval`: the candle interval (raw ticks/trades may be separate later)

2) Cursor / sequencing (required for dedup + reconnect)

- Streaming messages carry:
  - `sequence` (monotonic per `(source_id, symbol, interval)` stream)
  - `ts` (event/candle timestamp)
- Backfill responses include:
  - `start_sequence` / `end_sequence` (or a list with per-row sequence)
- Client applies updates idempotently by `sequence` (fallback `ts` only if unavoidable; sequence is strongly preferred).

3) Data types

- Candles: `(ts, open, high, low, close, volume)` + optional metadata (trade_count, vwap later)
- Optional "corrections" flag if a candle can be revised (initially assume append-only for simplicity).

Transport + encoding (baseline)

- Streaming: ZeroMQ `PUB/SUB`.
- Backfill/query ("RPC"): ZeroMQ `REQ/REP` (Phase 1) or `DEALER/ROUTER` (Phase 2+).
- Encoding: FlatBuffers (one schema for all messages, versioned).
- Compression (optional): zstd on payload frames for large backfills.

Message framing (recommended)

- ZMQ multipart frames:
  - Streaming: `[topic: bytes][payload: bytes]`
  - RPC: `[payload: bytes]` (REQ/REP) or `[routing...][payload: bytes]` (ROUTER)
- `topic` is ASCII and used only for subscription filtering; all data is in FlatBuffers payload.

Topic scheme (suggested)

- `candles.<source_id>.<symbol>.<interval>` for candle batches.
- (Later) `trades.<source_id>.<symbol>` or `ticks...` if the stream expands.

FlatBuffers schema (required deliverable)

- A single `market_data.fbs` defining:
  - `Envelope { schema_version, msg_type, correlation_id?, payload }`
  - `SubscribeCandlesRequest { source_id, symbol, interval, from_sequence? }`
  - `SubscribeCandlesAck { accepted, stream_key, latest_sequence }`
  - `CandleBatch { stream_key, start_sequence, candles[] }`
  - `BackfillCandlesRequest { source_id, symbol, interval, from_sequence_exclusive?, limit, end_ts? }`
  - `BackfillCandlesResponse { stream_key, start_sequence, candles[], has_more, next_sequence? }`
  - `GetCursorRequest/Response`
  - `HealthRequest/Response`
- Codegen is integrated on both sides (build script) and schema version is checked at runtime.

Server-side project plan

Phase S1: Minimal "market-data-service"

- Ingestion inputs (pick one to start):
  - Replay harness from existing CSV/kbar samples, OR
  - Bridge from an upstream ZMQ publisher, OR
  - Direct exchange connector (later)
- Persistence:
  - TimescaleDB (recommended; matches `../flux/docs/plan.md`), or
  - DuckDB for local/dev iteration, or
  - Postgres/ClickHouse for long-running production (later)
- Sockets (suggested defaults):
  - `ingest_sub` (optional): `SUB` from upstream feeds (live/backtest input)
  - `live_pub`: `PUB` to clients for live updates
  - `chunk_rep`: `REP` for backfill/cursor queries (upgrade to ROUTER when needed)
- Behaviors:
  - Assign monotonic `sequence` per `(source_id, symbol, interval)` and persist it.
  - Publish `CandleBatch` on `live_pub` (topic includes stream key).
  - Serve `BackfillCandlesRequest` and `GetCursorRequest` on `chunk_rep`.

Phase S2: Ordering + gap detection

- Ensure per-stream sequence assignment is monotonic and persisted.
- Gap detection:
  - If ingestion sees missing sequences or missing time buckets, record gaps for observability.
- Optional compaction:
  - rollup/partitioning by day/symbol

Phase S3: Operational hardening

- Metrics: per-stream lag, msg rate, dropped frames.
- Config: endpoints, storage path, retention.
- Replay harness as a first-class "integration test mode".

Client-side project plan (this repo)

Phase C1: Transport + model integration

- Add a "Live Source" type in the app's data source selection.
- Implement:
  - subscribe to a stream for `(source_id, symbol, interval)`
  - apply updates to in-memory candles (dedup by sequence)
  - write-through to local DuckDB cache (optional in Phase 1)
- Reconnect strategy:
  - on reconnect, query latest known cursor from local state and request `from_sequence`
  - if server indicates too old / retention exceeded, fall back to ZMQ backfill then resume stream

Phase C2: Backfill + warm start

- On startup or symbol switch:
  - load last session's candles from local DuckDB cache (fast paint; optional but recommended)
  - request server backfill from last cached cursor to now (ZMQ RPC)
  - then attach live stream
- Merge semantics:
  - prefer server sequence ordering
  - if overlaps, dedup by sequence (or ts if needed)

Phase C3: UX + controls

- Show connection status (connected/reconnecting/stale)
- Expose endpoint config in Settings (ties into milestone-10 style settings persistence)
- Manual "resync" action

Validation (bridge-level)

- Contract tests:
  - server emits a deterministic stream from a fixed CSV fixture
  - client subscribes, persists, restarts, and resumes from cursor without duplicates
- Failure tests:
  - server restarts; client reconnects and resumes
  - induced gap: drop N messages; client uses backfill to repair
- Manual:
  - run server locally, point app at it, watch live candles advance and backfill on reconnect

Open questions / decisions to record (Phase 0)

- ZMQ patterns: `PUB/SUB + REQ/REP` vs `PUB/SUB + DEALER/ROUTER`; topic scheme granularity.
- FlatBuffers: schema layout (single envelope vs per-message root tables), compatibility/versioning, optional zstd.
- Cursor truth: server-only (recommended) vs client-generated.
- Storage: DuckDB-only for dev vs a production DB path.

Status

- [ ] Phase 0 decisions captured (ZMQ patterns, FlatBuffers schema, cursor semantics).
- [ ] Server Phase S1 working end-to-end with replay harness.
- [ ] Client Phase C1 streaming integration.
- [ ] Client Phase C2 backfill + warm start.
- [ ] Hardening phases (S2/S3, C3).
