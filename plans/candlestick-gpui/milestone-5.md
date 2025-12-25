# Milestone: Multi-source UI and chart enhancements

Date: 2025-12-26
Scope: ui/app

Goals

- Add an alternate app path (keep the current app for debugging) where data is chosen at runtime via a sidebar button and a pop-out file picker instead of CLI args.
- The pop-out picker should also offer a TCP connection option as a placeholder (unimplemented for now).
- Add a volume subplot beneath the price chart.
- Add a horizontal cursor line and show the corresponding price on the y-axis.

Plan

- Fork or add a new binary entrypoint to keep the existing CLI-driven app intact for debugging.
- Build a sidebar with a “Select data” action that opens a modal/pop-out to choose a local file; include a disabled TCP option placeholder.
- Route the chosen file path into the existing loader/resampler flow and refresh the chart data.
- Extend the chart rendering to include a stacked volume subplot (shared x-axis).
- Add horizontal cursor line rendering and y-axis price readout aligned with the cursor.
- Ensure overlay layering: tooltip under interval menu, but above chart/volume.

Status

- [ ] Runtime file picker sidebar flow implemented.
- [ ] TCP connection placeholder exposed in the picker (non-functional).
- [x] Volume subplot renders under the main chart.
- [ ] Horizontal cursor line and y-axis price label rendered.
- [x] Tooltip layering fixed relative to interval menu.

Notes

- Keep the current CLI app unchanged for debugging and parity checks.
- Ensure the new UI path reuses existing core loaders/resampler.
