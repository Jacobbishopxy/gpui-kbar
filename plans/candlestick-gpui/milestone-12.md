# Milestone: Indicators (model + overlays + caches)

Date: 2026-01-13
Scope: core/ui/app

Context

Milestone 6 left indicator caches + overlays/drawings as the remaining “TradingView shell” follow-ups.
This milestone extracts that work into a dedicated indicator track and ties execution to the canonical
indicator architecture plan in `plans/runtime-plugins-live-data/indicator-plugins.md`.

Goals

- Define a stable in-tree indicator model (IDs, params schema, outputs schema) that also works for future plugins.
- Ship at least one built-in indicator overlay end-to-end (compute off the UI thread + render + toggle UX).
- Persist and restore indicator outputs/state so session restore can paint immediately (no “cold start” overlays).
- Lay the minimum UI scaffolding needed for future plugin discovery/loading (without implementing the plugin ABI yet).

Plan (bound to `plans/runtime-plugins-live-data/indicator-plugins.md`)

Phase 0: Decisions (documented in `indicator-plugins.md`)

- Choose ABI strategy + params/state encoding (even if Phase 2 is deferred).
- Decide indicator versioning rules (algorithm version vs package version vs ABI version).

Phase 1: Indicator model + built-ins (in-tree)

- Add an indicator definition model (id/name/version, params schema, outputs schema).
- Implement a reference built-in indicator set (start with SMA/EMA/RSI) against that model.
- Add stable hashing for params to support cache keys.

Phase 3: Persistence + restore (to unblock “indicator caches persisted/restored”)

- Define DuckDB schema for an `indicator_cache` (or evolve existing indicator storage) so we can:
  - write computed outputs/state keyed by `(indicator_id, indicator_version, params_hash, symbol, interval, source_id, cursor/revision)`.
  - load cached overlays during session restore before recompute.
- Integrate the cache lifecycle into symbol/interval switching and live updates (invalidate/extend deterministically).

Runtime UX

- Make the header “Indicators” affordance open a real overlay/panel (add/remove indicators, edit params, toggle visibility).
- Render at least one overlay line on top of price candles, with background recompute + cancellation on state changes.
- (Optional) Add a placeholder “Plugins” subsection that links to the plugin plan and reserves settings persistence keys.

Validation

- Unit tests: params hash stability; compute determinism on fixed candle fixtures.
- Integration/manual: enable indicator, restart app, verify overlay restores immediately from cache and continues updating.

Status

- [ ] Phase 0 decisions recorded in `plans/runtime-plugins-live-data/indicator-plugins.md`.
- [ ] Phase 1 indicator model + built-in indicators.
- [ ] Phase 3 DuckDB persistence + restore wired into runtime.
- [ ] Indicators UI overlay implemented and bound to the header control.
