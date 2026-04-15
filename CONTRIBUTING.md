# Contributing to wasm-actor-bridge

Thank you for your interest in contributing!

## Development Setup

```bash
git clone https://github.com/KoVal177/wasm-actor-bridge
cd wasm-actor-bridge
cargo build
```

Rust **1.94+** is required (edition 2024). For browser/WASM builds also install the target:

```bash
rustup target add wasm32-unknown-unknown
```

## Running Tests

Native tests (no browser required — all actor logic is testable natively):

```bash
cargo test
```

WASM compilation check:

```bash
cargo build --target wasm32-unknown-unknown
```

## Code Style

- Format: `cargo fmt --check` (enforced by CI)
- Lint: `cargo clippy --all-targets -- -D warnings`
- No `unsafe` code — `unsafe_code = "forbid"` is set workspace-wide.
- All public items must have at least a one-line `///` doc comment.

## Pull Request Process

1. Fork and create a feature branch from `main`.
2. Add native unit tests for new behaviour (see `src/tests.rs` for the pattern).
3. Ensure `cargo fmt`, `cargo clippy`, `cargo test`, and
   `cargo build --target wasm32-unknown-unknown` all pass.
4. Open a PR with a clear description of what changes and why.

## Bug Reports

Please open an issue on GitHub. Include a minimal reproducible example and the
output of `cargo --version` and `rustc --version`.
