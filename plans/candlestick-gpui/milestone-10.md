# Milestone: Runtime settings panel + presets integration

Date: 2026-01-09
Scope: app/ui runtime UX + configuration

Goals

- Provide a single popout “Settings” panel for user-configurable options (data source, chart behavior, performance toggles).
- Integrate synthetic perf presets into the main runtime (not just `dev`), so we can test/render large datasets without external CSVs.
- Persist settings in the existing DuckDB-backed user session (and restore on startup).

Plan

1) Define settings model + persistence
   - Add a `UserSettings` struct (and serialization) for runtime settings.
   - Persist/load via DuckDB user session keys (similar to `interval`, `range_index`, `replay_mode`).

2) Add settings popout panel UI
   - Add `settings_open: bool` and toggle from a header gear icon.
   - Implement a right-side drawer overlay (non-blocking), with close on outside-click and Escape.

3) Integrate perf presets into app runtime
   - Surface `50k / 200k / 1M` + `step_secs` inside the settings panel.
   - On apply: generate candles on `background_executor` and replace chart data, using the existing loading overlay.

4) Add app CLI presets (optional but useful)
   - Add `--preset`, `--n`, `--step-secs` flags to `app` to start in perf mode or override initial symbol.

Validation

- Manual: open Settings, switch presets repeatedly, verify spinner stays smooth and interactions remain responsive.
- Persist/restore: restart app and confirm settings (including perf mode) restore correctly.

Status

- [x] Settings model + persistence (`perf_mode`, `perf_n`, `perf_step_secs`).
- [x] Settings drawer overlay UI (header → “…” → Settings).
- [x] Perf presets integrated into app runtime (restore perf mode + generate on background).
- [x] App CLI presets (`--preset`, `--n`, `--step-secs`, `--symbol`).

Next

- [ ] Settings keyboard UX: close on `Esc`, and block chart interactions while Settings is open.
- [ ] Replace header “…” with a dedicated settings icon asset.
- [ ] Add Settings actions: reset to defaults and optional migration/cleanup for legacy `active_source="__PERF__..."`.
