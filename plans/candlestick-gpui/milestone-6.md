# Milestone: TradingView-style UI shell

Date: 2025-12-27
Scope: ui/app

Goals

- Reproduce a TradingView-like layout with header/chart/drawing toolbar/sidebar/footer.
- Ship core chart interactions (zoom/pan, crosshair/tooltip, price/time axes markers).
- Add watchlist and instrument summary scaffolding with symbol switching and data reloads.
- Lay foundations for overlays and drawing tools.

Plan

- Layout: build GPUI structure with header (symbol search input that can pop a symbol-search frame, frequent selector moved into the header next to search, interval selector, indicator/add buttons), main chart canvas, left drawing toolbar, right sidebar (watchlist plus instrument summary plus trading CTA stub), footer (left-aligned interval buttons: 1D 5D 1M 3M 6M 1Y 5Y ALL, plus playback/timezone readout).
- State model: central controller for symbol, interval, loaded data, overlays, drawings, viewport (scales/cursor), and panel toggles; pass read-only views into components.
- Data: reuse loaders/resampler; implement symbol switching and caching; maintain viewport-ready buffers and min/max per window for quick redraws.
- Persistence: persist session state (watchlist, active symbol/interval/range, replay flags) plus candles/indicator outputs in DuckDB for fast reloads and offline caching.
- Rendering: layered pipeline (grid -> volume -> candles -> overlays -> drawings -> crosshair/tooltip); add price axis and time axis ticks/labels with cursor badges aligned to edges.
- Interactions: wheel/trackpad zoom centered on cursor, drag-to-pan, hover crosshair with tooltip, keyboard shortcuts for interval switching and reset view.
- Drawings: start with trendline, horizontal line, rectangle; gesture state machine (start/drag/commit) with handles for move/delete and inline edit/remove toolbar.
- Overlays: begin with configurable moving averages; create overlay registry to add RSI/MACD later without touching core rendering; simple settings popover.
- Sidebar: watchlist list with last/percent change; click to load symbol; instrument summary card (price, percent change, session badges, key stats) plus stub trading panel entry point.
- Polish/perf: light/dark themes, label snapping to candles, debounced resize, cached background layers; add small snapshot test using sample CSV to guard render regressions.

Status

- [x] Layout shell sketched: header, chart/volume stack, left drawing toolbar stub, right watchlist/instrument/trading stubs, footer interval control.
- [x] Price axis restyled and hover price label anchored inside the y-axis column (consistent across price/volume).
- [x] Symbol search rebuilt as a centered TradingView-style popover with filters, scrollable results, and compact sizing (incl. `min_h_0` + scrollbar so results actually scroll).
- [x] Started UI refactor: extracted symbol search overlay + shared widgets/context modules; added sections/overlays scaffolding to slim render.rs.
- [x] Further refactor: moved header/sidebar layouts and interval menu into dedicated modules to keep render.rs readable.
- [x] Interval selector menu re-anchored using local positioning so header controls stay aligned when outer layouts (e.g. runtime sidebar, future collapsible panes) shift.
- [x] Watchlist triggers symbol loads; footer shows playback/timezone; quick ranges and replay toggles added; DuckDB storage layer with range filters landed.
- [x] DuckDB-backed caching + session restore for active symbol/interval/range/replay/watchlist wired into runtime + watchlist; symbol catalog (data/symbols.csv) now drives watchlist display/load paths with add/remove persistence.
- [x] User session snapshot struct added (active source, interval, range, replay, watchlist) with single-call hydration; candle writes/readbacks de-duped per timestamp to prevent duplication on restore.
- [x] Split persistent storage into `data/config.duckdb` (session_state/watchlist) + `data/data.duckdb` (candles/indicator_values/universe) with legacy `data/cache.duckdb` migration.
- [ ] Indicator caches persisted/restored via DuckDB.
- [ ] Implement overlays/drawings/interactions polish; snapshot tests/perf passes.

Next up

1) Persist indicator caches in DuckDB alongside candles/session and hydrate on startup.
2) Hook interval presets to real keyboard shortcuts and highlight active interval in header.
3) Make timezone dynamic (system or exchange tz) and surface actual replay state (paused/playing, speed).
4) Start overlay/drawing scaffolding (MA overlay toggle + basic trendline gestures).
5) Add a minimal snapshot/render test using sample CSV to guard UI regressions.
