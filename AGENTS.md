# Repository Guidelines

## Project Structure & Module Organization

- `core/`: data layer (DuckDB, Polars) with loaders/resampling and an example at `core/examples/dump_cache.rs`.
- `ui/`: GPUI components and runtime views (`src/runtime.rs`, charts, assets wiring).
- `app/`: binary entry that launches the UI runtime.
- `scripts/`: Python generators for universe and kbar sample data; outputs flow into `data/`.
- `data/`: cached DuckDB, mapping CSVs, and candle samples; treat as generated artifacts unless updating source CSVs.
- `assets/`: SVG icons consumed by the UI; keep filenames stable to match references.
- `tmp/`, `target/`, `plans/`: scratch/build outputs; avoid manual edits.

## Build, Test, and Development Commands

- `cargo fmt` — format the Rust workspace.
- `cargo clippy --all-targets --all-features` — lint all crates.
- `cargo test --workspace` — run Rust tests (per-crate if added).
- `cargo run -p app --bin runtime` — launch the runtime UI.
- `uv run scripts/generate_universe.py` — build base symbol universe.
- `uv run scripts/generate_kbar.py -n 3000 -i 1` — generate sample kbar data; adjust `-n` (count) and `-i` (interval) as needed.

## Coding Style & Naming Conventions

- Rust 2024 edition; 4-space indent; prefer `?` over `unwrap`; keep modules snake_case and types CamelCase.
- Run `cargo fmt`/`clippy` before commits; add concise `///` docs on new public items in `core`.
- Asset additions should be lower-kebab-case SVGs in `assets/` and referenced via `ui::assets`.

## Testing Guidelines

- Add unit tests near logic (e.g., `core/src/store.rs` transformations); name with `#[test] fn describes_behavior()`.
- Favor integration checks with realistic CSV/DuckDB fixtures in `data/` rather than mocks.
- If automated coverage is not feasible, document manual verification (e.g., run the runtime and validate charts/load flows).

## Commit & Pull Request Guidelines

- Match existing history: emoji prefix + concise summary (e.g., `✨ Add runtime launcher`).
- PRs: include a short description, linked issue (if any), tests run (`cargo test`, data scripts), and UI before/after screenshots for visual changes.
- Keep changes focused; call out migrations or data regenerations (`uv run ...`) explicitly in the description.
