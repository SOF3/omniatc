# Copilot onboarding notes for omniatc

## Repository overview
- **Purpose**: omniatc is a Rust-based air-traffic-control simulator with a Bevy + egui client, reusable simulation core, math helpers, and CLI tooling for map assets.
- **Project type**: Rust workspace (edition 2024, rust-version 1.89) targeting native desktop and WebAssembly.
- **Primary runtimes**: Rust stable for builds/tests; nightly only for `cargo fmt` because `rustfmt.toml` enables unstable formatting options.
- **Key dependencies**: Bevy, egui/bevy_egui, and Trunk for the web build.

## Project layout (high-level)
- `Cargo.toml` (workspace members: `omniatc-client`, `omniatc-core`, `omniatc-maps`, `omniatc-math`, `omniatc-store`).
- `omniatc-client/` – desktop/WASM client; entry point at `omniatc-client/src/main.rs`.
- `omniatc-core/` – simulation core; public modules in `omniatc-core/src/lib.rs`.
- `omniatc-math/` – math + physics + units; entry at `omniatc-math/src/lib.rs` (tests in `tests.rs`).
- `omniatc-store/` – data model for persistence.
- `omniatc-maps/` – tools for building and processing map assets, plus builtin game maps (each map in its own module).
- `assets/` – contains committed runtime assets plus generated outputs; only `assets/maps/` and `assets/schema.json.gz` are generated and gitignored, other `assets/` contents are tracked and required at runtime.
- `web.html` + `Trunk.toml` – Trunk web build configuration (target `web.html`, public URL `/omniatc`).
- Config files: `rustfmt.toml`, `typos.toml`, `.editorconfig`, `.gitignore`.

## CI / automation
- `.github/workflows/ci.yaml` runs:
  - `cargo +nightly fmt --all -- --check`
  - `cargo clippy --all --tests -F precommit-checks -- -D warnings`
  - `cargo test --all`
- `.github/workflows/build.yaml` builds web (`trunk build`), assets/schema (`omniatc-maps`), docs (`cargo doc`), and cross-compiled desktop binaries.

## Build + validation (validated commands)
**Always run these in this order before committing (matches CI):**
1. `cargo +nightly fmt --all -- --check`
   - Requires `rustup component add --toolchain nightly rustfmt`.
   - `cargo fmt --all -- --check` on stable fails with: `format_code_in_doc_comments` is unstable.
2. `cargo clippy --all --tests -F precommit-checks -- -D warnings`
   - First run downloads dependencies; completed successfully.
3. `cargo test --all`
   - Completed successfully (took ~3 minutes on first run).

**Map generation:**
- `cargo run -p omniatc-maps build-assets`
  - Creates `assets/maps/*.osav` (gitignored).

**Desktop client run:**
- `cargo run -p omniatc-client`
  - Starts a windowed app; in headless CI a quick validation was done via `timeout 5s cargo run -p omniatc-client` which exits with code 124 after compiling.
  - Run locally without `timeout` to actually play.

**Web build (Trunk):**
- Prereqs: `rustup target add wasm32-unknown-unknown` and Trunk `v0.21.5` (downloaded via the workflow `wget | tar xz`).
- Command: `RUSTFLAGS='--cfg getrandom_backend="wasm_js"' ./trunk build --release=true`
  - Build was started successfully but took a long time in the sandbox; stop/retry if it exceeds your time budget.

**Docs:**
- `cargo doc` (used in CI build workflow).
  - In this environment it took several minutes and was stopped before completion; expect a long run.

## Code-style / contributor conventions
- Use the `foo.rs` + `foo/submodule.rs` pattern; **do not** use `foo/mod.rs`.
- Avoid `bevy::prelude::*` and avoid prelude re-exports when direct imports are possible.
- Place unit tests in `tests.rs` and include with `#[cfg(test)] mod tests;`.
- For vector norm comparisons, use `distance_cmp` / `magnitude_cmp` instead of exact or squared comparisons.
- Prefer `Vec::from([..])` for literals; use `Vec::new()` for empty vectors.
- Use the `QueryExt`/`WorldExt` traits (re-exported as `QueryTryLog`/`WorldTryLog`) and the `try_log!`/`try_log_return!` macros from `omniatc-core/src/try_log.rs` for `World`/`Query` component access when missing components would be a bug. The `None` branch should abort processing for the current entity unless aggregating results.
- When updating a pull request branch, prefer amending commits if the previous commit is also authored by Copilot and not in the base branch.

## README notes
- The README’s “Compile from source” steps match the `omniatc-maps` assets build and `cargo run --release -p omniatc-client` for the game.

**Trust these instructions first and only search the repo when they are incomplete or incorrect.**
