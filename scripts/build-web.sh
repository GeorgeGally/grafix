#!/usr/bin/env bash
# Repeatable browser build: Rust -> wasm32 -> wasm-bindgen -> Vite.
#
#   scripts/build-web.sh            release wasm + production web bundle
#   scripts/build-web.sh --wasm     wasm + bindings + assets only (for `npm run dev`)
#   scripts/build-web.sh --dev      use the dev cargo profile (faster compile, slower runtime)
#
# Requires rustup (the pinned toolchain in rust-toolchain.toml installs itself)
# and Node >= 22. wasm-bindgen-cli is installed into .tools/ at the exact
# version Cargo.lock pins. wasm-opt is used when on PATH (set GRAFIX_WASM_OPT=0 to skip).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITOR="$ROOT/apps/editor"
PKG="$EDITOR/src/wasm-pkg"
PROFILE=release
WASM_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --dev) PROFILE=dev ;;
    --wasm) WASM_ONLY=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

command -v cargo >/dev/null || { echo "cargo not found: install rustup (https://rustup.rs)" >&2; exit 1; }
command -v npm >/dev/null || { echo "npm not found: install Node >= 22" >&2; exit 1; }

BINDGEN_VERSION="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$ROOT/Cargo.lock")"
BINDGEN="$ROOT/.tools/bin/wasm-bindgen"
if [[ ! -x "$BINDGEN" ]] || [[ "$("$BINDGEN" --version)" != "wasm-bindgen $BINDGEN_VERSION" ]]; then
  echo "==> installing wasm-bindgen-cli $BINDGEN_VERSION into .tools/"
  cargo install wasm-bindgen-cli --version "$BINDGEN_VERSION" --locked --root "$ROOT/.tools"
fi

# Same cfg as .cargo/config.toml, plus path remapping so local absolute paths
# (home directory, cargo registry) never end up in the shipped binary.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="--cfg getrandom_backend=\"wasm_js\" --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$ROOT=/grafix"

echo "==> cargo build ($PROFILE, wasm32-unknown-unknown)"
cargo build --manifest-path "$ROOT/Cargo.toml" --locked --target wasm32-unknown-unknown \
  -p bridge-wasm $([[ "$PROFILE" == release ]] && echo --release)

OUT_DIR=$([[ "$PROFILE" == release ]] && echo release || echo debug)
WASM="$ROOT/target/wasm32-unknown-unknown/$OUT_DIR/bridge_wasm.wasm"

echo "==> wasm-bindgen"
rm -rf "$PKG"
"$BINDGEN" --target web --out-dir "$PKG" "$WASM"

if [[ "$PROFILE" == release ]]; then
  # The `name` section is about half the release binary. Removed after
  # wasm-bindgen (not via `strip`, which also drops `target_features`).
  node "$ROOT/scripts/strip-wasm-names.mjs" "$PKG/bridge_wasm_bg.wasm"
fi

if [[ "${GRAFIX_WASM_OPT:-1}" != 0 ]] && command -v wasm-opt >/dev/null && [[ "$PROFILE" == release ]]; then
  echo "==> wasm-opt -O2"
  wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    "$PKG/bridge_wasm_bg.wasm" -o "$PKG/bridge_wasm_bg.wasm"
fi

echo "==> syncing assets/ into apps/editor/public/assets"
rm -rf "$EDITOR/public/assets"
mkdir -p "$EDITOR/public"
cp -R "$ROOT/assets" "$EDITOR/public/assets"

if [[ ! -d "$EDITOR/node_modules" ]]; then
  echo "==> installing locked editor dependencies"
  (cd "$EDITOR" && npm ci --no-audit --no-fund)
fi

if [[ "$WASM_ONLY" == 0 ]]; then
  echo "==> vite build"
  (cd "$EDITOR" && npm run build)
  echo "==> done: $EDITOR/dist  (serve with scripts/serve.sh)"
fi
ls -l "$PKG/bridge_wasm_bg.wasm"
