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

- `cargo fmt`: format the Rust workspace.
- `cargo clippy --all-targets --all-features`: lint all crates.
- `cargo test --workspace`: run Rust tests (per-crate if added).
- `cargo run -p app`: launch the runtime UI.
- `uv run scripts/generate_universe.py`: build base symbol universe.
- `uv run scripts/generate_kbar.py -n 3000 -i 1`: generate sample kbar data; adjust `-n` (count) and `-i` (interval) as needed.

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

## Skills

- `skill-creator`: guide for creating effective skills when users want a new skill or to update an existing one that extends Codex's capabilities with specialized knowledge, workflows, or tool integrations. (file: C:/Users/xiey/.codex/skills/.system/skill-creator/SKILL.md)
- `skill-installer`: install Codex skills into `$CODEX_HOME/skills` from a curated list or a GitHub repo path; use when asked to list or install skills, including private repos. (file: C:/Users/xiey/.codex/skills/.system/skill-installer/SKILL.md)
- Discovery: available skills are listed in project docs and may also appear in a runtime "## Skills" section (name + description + file path); these are the sources of truth, and skill bodies live on disk at the listed paths.
- Trigger rules: if the user names a skill (with `$SkillName` or plain text) or the task clearly matches a skill's description, use that skill for that turn; multiple mentions mean use them all; do not carry skills across turns unless re-mentioned.
- Missing/blocked: if a named skill is unavailable or the path cannot be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1. After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2. If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; avoid bulk loads.
  3. If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  4. If assets or templates exist, reuse them instead of recreating from scratch.
- Description as trigger: the YAML `description` in `SKILL.md` is the primary trigger signal; rely on it to decide applicability; if unsure, ask a brief clarification before proceeding.
- Coordination and sequencing: if multiple skills apply, choose the minimal set that covers the request and state the order you'll use them; announce which skills you're using and why (one short line); if you skip an obvious skill, say why.
- Context hygiene: keep context small by summarizing long sections instead of pasting them; only load extra files when needed; avoid deeply nested references and prefer one-hop files explicitly linked from `SKILL.md`; when variants exist, pick only the relevant reference files and note that choice.
- Safety and fallback: if a skill cannot be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.
