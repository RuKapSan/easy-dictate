# Repository Guidelines

## Project Structure & Module Organization
- `frontend/` — static UI (`index.html`, `main.js`, `styles.css`).
- `src-tauri/` — Tauri v2 + Rust app:
  - `src/` (`audio.rs`, `input.rs`, `openai.rs`, `settings.rs`, `main.rs`, `lib.rs`).
  - `src/bin/dev_server.rs` — simple static dev server on `127.0.0.1:1420`.
  - `tauri.conf.json` — app config (uses `frontendDist: ../frontend`).
  - `icons/`, `permissions/`, `capabilities/`, `gen/schemas/` — packaging and ACLs.
- `src-tauri/target/` — build artifacts (do not commit).

## Build, Test, and Development Commands
- Prereqs: Rust ≥ 1.77.2. Install CLI: `cargo install tauri-cli`.
- Dev app (auto-runs dev server): `tauri dev` (from repo root).
- Dev server only: `cargo run --manifest-path src-tauri/Cargo.toml --bin dev-server` → http://localhost:1420.
- Build installers: `tauri build` → artifacts under `src-tauri/target/{debug,release}`.
- Lint/Format (Rust): `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings`.
- Tests (Rust): `cargo test --manifest-path src-tauri/Cargo.toml`.

## Coding Style & Naming Conventions
- Rust: rustfmt default style; deny clippy warnings. Names — modules/functions `snake_case`, types `PascalCase`, constants `UPPER_SNAKE_CASE`.
- Frontend: small ES modules, `camelCase` for JS, keep files small and framework‑free.

## Testing Guidelines
- Place unit tests inline with `#[cfg(test)] mod tests` near logic.
- Integration tests under `src-tauri/tests/`; name files `*_tests.rs`; test fns `test_*`.
- Prefer fast, deterministic tests; for async use `#[tokio::test]`.

## Commit & Pull Request Guidelines
- Conventional style recommended: `feat|fix|docs|refactor|build|ci|chore(scope): message`.
- Keep commits focused; reference issues (`#123`).
- PRs: describe change and user impact; include screenshots/GIFs for UI; list OS tested; ensure `fmt`, `clippy`, `test` pass. Call out changes to `tauri.conf.json`/permissions.

## Security & Configuration Tips
- Never commit API keys. App stores key in user `settings.json`; `OPENAI_BASE_URL` is optional override (dev example: `$env:OPENAI_BASE_URL="https://api.openai.com"`).
- Large binaries/assets belong outside git; keep `frontend/` assets lightweight.

## Agent-Specific Instructions
- Scope: this file governs the entire repo. Update `permissions/` and `capabilities/` when adding new Tauri commands. Touch UI in `frontend/`, platform logic in `src-tauri/src/`. Avoid broad refactors without an issue first.
