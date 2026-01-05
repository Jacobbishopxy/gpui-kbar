# Loading spinner test matrix (temporary)

Goal: identify why loading_sand animation stalls during symbol loads and converge on a stable scheduling pattern. We will try the following in order, one at a time.

## Baseline (current)
- Event-driven load flow (LoadMsg Start/Finished).
- render(): request_animation_frame + refresh when loading_symbol is set.
- on_next_frame pump (start_loading_frame_pump) refreshes while loading.
- Observe: intermittent stalls (often first/second load), sometimes panic when RAF off paint (previously).

## Test 1: Remove pump, rely on render RAF only
- Change: disable start_loading_frame_pump (no on_next_frame recursion); keep render-time RAF+refresh when loading_symbol is Some.
- Rationale: pump may fight invalidator or miss paint phase; isolate render-only scheduling.
- Expected: spinner should tick on all loads; if stalls persist, pump is not the culprit.

## Test 2: Timer-driven refresh while loading
- Change: keep pump disabled; add a lightweight gpui::Timer during loading that emits a “tick” event to self, and on tick if loading_symbol.is_some() call window.refresh() (no RAF).
- Rationale: decouple from on_next_frame scheduling; avoid calling RAF outside paint.
- Expected: steady refresh cadence; animation should run if RAF in render triggers paint.

## Test 3: Idempotent pump guard
- Change: reintroduce pump but guard with a flag so only one pump runs; only refresh if loading_symbol.is_some() and invalidator.is_dirty() is false (if accessible).
- Rationale: avoid over-scheduling or starving invalidator; ensure one pump at a time.
- Expected: frames flow without recursion overload.

## Test 4: Minimal gpui repro
- Change: create a tiny view that spawns a 3s sleep task and shows loading_sand; use render RAF only.
- Rationale: determine if Windows gpui suppresses RAF+refresh during long background tasks; informs whether we need a timer workaround.
- Expected: if spinner stalls here too, bug is in gpui; adopt timer workaround.

### Result (2026-01-07)
- Built `dev` workspace member: minimal window, loads `loading_sand` via `application_with_assets`, runs heavy CSV+CPU (~5s), uses render RAF, pump, and timer refresh. Spinner animates the whole time. Conclusion: CPU task itself doesn’t block `loading_sand` when assets + pump/timer are present; the issue in `ui` is elsewhere (likely state/invalidator interaction).

## Logging tweaks during tests
- Track per-load: load_id, started_at, finished_at, frame_log_count at start/end.
- Optionally log when requesting RAF and when pump/timer refresh fires (once every N frames).

Cleanup: delete this file after tests and remove extra instrumentation.
