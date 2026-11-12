#!/usr/bin/env bash
# Static checks: formatting, clippy for the wasm and native targets, Svelte types.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo fmt --manifest-path "$ROOT/Cargo.toml" --all -- --check
cargo clippy --manifest-path "$ROOT/Cargo.toml" --locked --target wasm32-unknown-unknown \
  -p bridge-wasm -- -D warnings
cargo clippy --manifest-path "$ROOT/Cargo.toml" --locked -p graphics-cli -- -D warnings
(cd "$ROOT/apps/editor" && npm run check)
