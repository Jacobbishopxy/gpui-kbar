# Plan: Indicator Modules + Dynamic Plugin System (Prereq for Indicator Caches)

Date: 2026-01-10
Scope: `core/ui/app` indicator architecture + plugin loading

Execution milestone (this repo)

- `plans/candlestick-gpui/milestone-12.md` is the current owner for implementing Phase 0/1/3 of this plan (plugin ABI/loader work may land in a follow-up milestone).

Problem statement

We can’t persist “indicator caches” until:

1) Indicators are defined as first-class modules with stable IDs, parameters, and deterministic outputs.
2) Third parties (or we) can implement indicators out-of-tree and ship them as dynamic libraries, which the app can discover and load at runtime.

Goals

- Define a stable indicator model that supports:
  - stateless indicators (pure function on candles)
  - stateful/streaming indicators (incremental update as candles append)
  - multi-output indicators (e.g., MACD has multiple lines + histogram)
- Provide a plugin SDK that allows a user to build an indicator into a `.dll/.so/.dylib` and drop it into a folder.
- Add a runtime UI flow to select/remember a plugin folder and enable/disable loaded indicators.
- Define the persistence contract for “indicator cache” so milestone-6 can store/restore indicator state.

Key constraints (Rust + dynamic libs)

- Rust `trait` objects are not ABI-stable across compiler versions; a plugin boundary must use a stable ABI.
- Prefer a C-compatible ABI (explicit structs + function pointers) and version it.
- Treat plugins as “trusted code” in-process initially; sandboxing/hardening is a later milestone.

High-level architecture

1) `indicators` (in-tree): built-in indicators implemented against the same stable model used by plugins.
2) `indicator_sdk` (in-tree): the public ABI + types + helper macros for plugin authors.
3) `indicator_host` (in-tree): app/runtime integration: discovery, loading, registry, UI, persistence hooks.
4) “plugin” crates (out-of-tree): depend on `indicator_sdk`, compile as `cdylib`, export a single entry symbol.

Deliverables

1) Indicator core model (no plugins yet)

    - Define “indicator definition”:
      - `IndicatorId` (stable string, e.g. `com.example.rsi`)
      - `DisplayName`
      - `Version` (semantic or integer ABI-independent version)
      - `Inputs` (required data series: candles, volume, etc.)
      - `Params` schema (typed: bool/int/float/string/enum; with defaults, ranges, formatting)
      - `Outputs` schema (named series + style hints: line/histogram, color, y-axis group)
    - Define compute interface:
      - Batch compute: `compute_all(candles, params) -> outputs`
      - Streaming compute: `init(params) -> state`, `update(state, new_candles) -> output_deltas`
      - Determinism requirements: same inputs => same outputs; stable rounding rules.
    - Define cache model:
      - Cache key: `(indicator_id, indicator_version, params_hash, symbol, interval, source_id, candles_revision)`
      - Cache payload:
        - outputs (compressed arrays)
        - state snapshot (optional; for streaming indicators)
        - metadata: last candle timestamp + cursor/revision used

1) Plugin ABI v1 (SDK + host)

    - Define a C-compatible ABI module (`indicator_sdk::abi`), including:
      - `INDICATOR_ABI_VERSION` (u32)
      - `PluginManifest` (plugin name, plugin version, supported ABI version range)
      - `IndicatorDescriptor` (id/name/version, params schema, outputs schema)
      - `HostApi` callbacks (logging, allocation helpers, maybe time)
      - `IndicatorVTable` (function pointers):
        - `create_instance(params_bytes) -> *mut Instance`
        - `free_instance(*mut Instance)`
        - `compute_all(instance, candles_view, outputs_sink) -> Status`
        - `update(instance, appended_candles_view, outputs_sink) -> Status`
        - `snapshot_state(instance, sink) -> Status`
        - `restore_state(instance, bytes) -> Status`
      - `Status` + error string retrieval (avoid Rust panics across boundary)
      - Byte encoding: prefer `rkyv` or `bincode` for params/state blobs, but treat as “opaque bytes” at ABI level.
    - Define a single exported entrypoint symbol, e.g.:
      - `kbar_indicator_plugin_entry_v1(host_api: *const HostApi) -> *const PluginApiV1`
    - Implement loading via `libloading` in the app, with:
      - ABI/version check + manifest read
      - descriptor registration into a runtime registry
      - crash containment strategy (initially: propagate errors; later: isolate/sandbox)

1) Runtime UX

    - Settings: “Plugins” section:
      - Choose folder (file picker)
      - Scan and list discovered plugins
      - Enable/disable per-plugin and per-indicator
      - Show compatibility/ABI mismatch errors inline
      - “Reload plugins” action
    - Remember plugin folder + enabled states in DuckDB user session (similar to other settings).

1) Persistence hooks (to unblock indicator cache persistence milestone)

    - Define how the chart runtime requests indicator outputs:
      - “attach indicator” to chart state
      - compute in background executor
      - cache results keyed by above cache key
      - when restoring session: load cached outputs/state first, then resume updates with new candles
    - Ensure plugin indicators and built-in indicators share the same cache schema.

Plan (phased)

Phase 0: Decisions (1–2 days)

- Choose ABI strategy:
  - Option A (recommended): C ABI + function tables + opaque bytes.
  - Option B: `abi_stable` crate (faster dev, but adds dependency + patterns).
  - Option C: WASM plugins (better sandboxing, but not “dynamic libs”).
- Choose params/state encoding format (opaque at boundary).
- Decide whether plugins can access file/network (default: no explicit host API for this yet).

Phase 1: Indicator model + built-in indicators (in-tree)

- Add `core::indicators` (or a new crate `indicators`) with:
  - `IndicatorId`, `ParamSchema`, `OutputSchema`, `IndicatorDefinition`
  - reference indicators: `SMA`, `EMA`, `RSI` (enough to exercise multi-output later)
- Wire into UI overlay system minimally:
  - select indicator
  - render one overlay line
  - background compute + cancellation on symbol/interval changes

Phase 2: Plugin SDK + host loader

- Add `indicator_sdk` exposing:
  - ABI types + helper macros to declare descriptors and vtables
  - sample plugin template in `dev/` or `examples/` (not shipped by default)
- Add `indicator_host` in app/ui:
  - folder scanning
  - load/unload + registry
  - error reporting

Phase 3: Persistence + cache restore

- Define DuckDB tables/keys:
  - `indicator_cache` keyed by `(indicator_id, indicator_version, params_hash, symbol, interval, source, last_ts)`
  - store `outputs_blob`, `state_blob`, `metadata_json`
- On startup/session restore:
  - restore enabled indicators
  - hydrate cached outputs and render immediately
  - resume updates when new candles arrive

Validation

- Unit tests:
  - params hashing stability
  - schema serialization round-trips
  - cache key behavior across symbol/interval changes
- Integration tests:
  - load a sample plugin from a fixture folder (CI can build it as part of workspace if desired)
  - compute outputs match expected fixtures
  - persist -> restart -> restore -> identical outputs
- Manual:
  - choose plugin folder, hot reload, enable/disable indicator overlays
  - verify no UI stalls (compute runs off UI thread)

Risks / open questions

- Cross-platform dynamic loading quirks (Windows DLL search paths; macOS codesigning).
- In-process plugins can crash the app; sandboxing (WASM or out-of-proc) may be needed later.
- Versioning strategy: indicator “algorithm version” vs plugin package version vs ABI version.

Status

- [ ] Phase 0 decisions recorded.
- [ ] Phase 1 indicator model + built-ins.
- [ ] Phase 2 plugin SDK + runtime loader.
- [ ] Phase 3 persistence + restore to unblock “indicator caches persisted/restored”.
