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
- Rendering: layered pipeline (grid -> volume -> candles -> overlays -> drawings -> crosshair/tooltip); add price axis and time axis ticks/labels with cursor badges aligned to edges.
- Interactions: wheel/trackpad zoom centered on cursor, drag-to-pan, hover crosshair with tooltip, keyboard shortcuts for interval switching and reset view.
- Drawings: start with trendline, horizontal line, rectangle; gesture state machine (start/drag/commit) with handles for move/delete and inline edit/remove toolbar.
- Overlays: begin with configurable moving averages; create overlay registry to add RSI/MACD later without touching core rendering; simple settings popover.
- Sidebar: watchlist list with last/percent change; click to load symbol; instrument summary card (price, percent change, session badges, key stats) plus stub trading panel entry point.
- Polish/perf: light/dark themes, label snapping to candles, debounced resize, cached background layers; add small snapshot test using sample CSV to guard render regressions.

Status

- [x] Layout shell sketched: header, chart/volume stack, left drawing toolbar stub, right watchlist/instrument/trading stubs, footer interval control.
- [x] Price axis restyled and hover price label anchored inside the y-axis column (consistent across price/volume).
- [x] Symbol search rebuilt as a centered TradingView-style popover with filters, scrollable results, and compact sizing.
- [ ] Wire watchlist/symbol switching + data reloads; implement overlays/drawings/interactions polish; snapshot tests/perf passes.
