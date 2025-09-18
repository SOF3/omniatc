# Copilot Instructions for omniatc

## Project Overview

**omniatc** is a Rust-based Air Traffic Control (ATC) simulator game. It's a multi-platform application that runs on desktop (Windows, macOS, Linux) and web browsers via WebAssembly (WASM). The project uses the Bevy game engine and provides both 2D and 3D visualization capabilities for managing aircraft traffic.

**Repository Structure:**
- **Language:** Rust (edition 2024, MSRV 1.89)
- **Framework:** Bevy 0.16.0 game engine
- **Architecture:** Cargo workspace with 5 crates
- **Build Tools:** Cargo for native builds, Trunk for WASM builds
- **Size:** ~260 MB dependencies, moderate complexity

## Build and Test Commands

### Prerequisites
- Rust 1.89+ with `cargo` 
- For WASM builds: `wasm32-unknown-unknown` target and Trunk 0.21+

### Essential Commands (Always Run These First)
```bash
# Always run these in order before making any changes:
cargo check                           # ~2.5 minutes first time, ~10s incremental
cargo test                           # ~3 minutes first time, runs 45 tests
cargo build                          # ~3-5 minutes for dev build
```

### Asset and Map Generation (Required for Web Builds)
```bash
# Must run these commands before WASM builds:
cargo run -p omniatc-maps build-assets     # ~30 seconds, generates assets/maps
mkdir -p schema && cargo run -p omniatc-maps json-schema schema/schema.json.gz  # ~5 seconds
```

### WASM Build Commands
```bash
# Add WASM target (one-time setup):
rustup target add wasm32-unknown-unknown

# WASM build for web:
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --target wasm32-unknown-unknown --no-default-features -p omniatc-client
# Takes ~3 minutes, requires no default features flag

# Full web build with Trunk (requires trunk binary):
wget -O - https://github.com/trunk-rs/trunk/releases/download/v0.21.5/trunk-x86_64-unknown-linux-gnu.tar.gz | tar xz
./trunk build --release=true
# Requires RUSTFLAGS='--cfg getrandom_backend="wasm_js"' environment variable
```

### Linting and Formatting
```bash
cargo clippy                         # Runs clean, no warnings
cargo +nightly fmt --check          # Must use nightly for rustfmt.toml features
```

**Note:** Always use `cargo +nightly fmt` for formatting. The rustfmt.toml uses nightly-only features. Binaries are built with stable/beta but formatting requires nightly.

### Release Builds
```bash
cargo build --release -p omniatc-client     # ~10+ minutes, creates optimized binary
```

## Utility Modules and Code Reuse

### Core Utilities (omniatc-core/src/util.rs)
**Always check and reuse these utilities before implementing similar functionality:**

- **`TryLog<T>` trait:** For error handling with logging - converts `Option<T>` and `Result<T, E>` to `Option<T>` while logging errors
- **`configure_ordered_system_sets<E>()`:** Sets up ordered system execution for enum-based SystemSets
- **`EnumScheduleConfig` trait:** Provides `.after_all<E>()` and `.before_all<E>()` for scheduling systems relative to enum sets
- **`manage_entity_vec<C, X, NB>()`:** Manages dynamic lists of entities with spawn/update/despawn logic
- **`run_async<R>()` and `run_async_local<R>()`:** Async task execution with result handling via observer pattern
- **`RateLimit` SystemParam:** Throttles system execution based on time intervals

### Client Utilities (omniatc-client/src/util/)
**Reuse these for UI and rendering tasks:**

- **`util/shapes.rs`:** Provides reusable mesh handles (`Meshes` resource) for squares, circles, and lines with thickness maintenance
- **`util/billboard.rs`:** 
  - `MaintainScale` component: Keeps entities same size regardless of camera zoom
  - `MaintainRotation` component: Keeps entities same orientation regardless of camera rotation  
  - `Label` component: Smart text positioning with real-world and screen-space offsets
- **`util/anchor.rs`:** Configuration UI for `Anchor` enum with egui integration
- **`util.rs`:** Cross-platform time utilities (`time_now()`) and directional helpers (`heading_to_approx_name()`)

### Utility Patterns to Follow
- Use `try_log!` macro with these utilities for consistent error handling
- Leverage enum-based system sets with the scheduling utilities
- Prefer the async utilities over manual task management
- Use the billboard components for consistent UI scaling and rotation
- Reuse mesh handles from the shapes utility rather than creating new ones

## Project Architecture

### Workspace Structure
```
omniatc/
├── omniatc-core/          # Core game logic and simulation engine
├── omniatc-client/        # UI and rendering (main executable)  
├── omniatc-math/          # Math utilities and unit types
├── omniatc-store/         # Data schema and storage formats
├── omniatc-maps/          # Map generation tools (CLI binary)
├── assets/                # Game assets (maps generated at build time)
├── .github/workflows/build.yaml  # CI/CD pipeline
└── web.html              # Trunk config for WASM builds
```

### Key Configuration Files
- **Cargo.toml:** Workspace root with shared dependencies
- **rustfmt.toml:** Code formatting (uses nightly features)
- **.editorconfig:** Cross-editor formatting rules
- **Trunk.toml:** WASM build configuration
- **.github/workflows/build.yaml:** CI/CD with multi-platform builds

### Main Entry Points
- **omniatc-client/src/main.rs:** Desktop application entry point
- **omniatc-maps/src/main.rs:** Map generation CLI tool
- **omniatc-core/src/lib.rs:** Core game logic library

## Validation and CI Pipeline

### GitHub Workflow Steps
The repository runs these checks on every push to master:

1. **WASM Build:** Web client compilation with asset generation
2. **Asset Processing:** Map generation and JSON schema creation  
3. **Multi-platform Builds:** Desktop clients for 6 platforms
4. **GitHub Pages Deployment:** Automatic web deployment

### Pre-commit Validation
```bash
# Run these to match CI validation:
cargo check --all-targets
cargo test
cargo clippy
cargo +nightly fmt --check
cargo run -p omniatc-maps build-assets
cargo run -p omniatc-maps json-schema schema/schema.json.gz
```

### Known Build Issues and Workarounds
- **Formatting:** Always use `cargo +nightly fmt` - nightly is required for rustfmt.toml features
- **WASM Build:** Must use `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` and `--no-default-features`
- **Assets:** Asset generation must complete before WASM builds
- **Release Build:** Takes 10+ minutes due to heavy optimization

## Development Features and Flags

### Feature Flags
- **default:** `["dev"]` - enables dynamic linking for faster compile times
- **dev:** Development features and dynamic linking
- **inspect:** Debug UI with bevy-inspector-egui
- **precommit-checks:** Stricter linting and error modes

### Platform-Specific Dependencies
- **WASM targets:** Uses `idb`, `js-sys`, `wasm-bindgen`, `getrandom` with wasm_js
- **Desktop targets:** Uses `rusqlite` with bundled SQLite

## Testing Infrastructure

The project has **45 unit tests** across multiple crates:
- **omniatc-math:** 37 tests (geometry, physics, units)
- **omniatc-core:** 7 tests (game logic, pathfinding)  
- **omniatc-client:** 1 test (rendering curves)

Tests run clean and complete in ~3 minutes on first run.

## Trust These Instructions

These instructions are comprehensive and tested. Only search for additional information if:
- Commands fail with errors not mentioned here
- You need details about specific code organization within crates
- You encounter platform-specific build issues not covered

The build and test commands listed here are verified to work and follow the exact sequence used by the CI/CD pipeline.