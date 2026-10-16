#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
CARGO=${CARGO:-cargo}
WASM_BINDGEN=${WASM_BINDGEN:-wasm-bindgen}
TARGET_DIR=${CARGO_TARGET_DIR:-target}
WEB_OUT=${WEB_OUT:-dist}
# Keep CLI and Rust bindings at exactly the same version.
version=$(awk '/name = "wasm-bindgen"/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
if ! command -v "$WASM_BINDGEN" >/dev/null 2>&1; then
  echo "Install the browser bundler: cargo install --locked wasm-bindgen-cli --version $version" >&2
  exit 1
fi
if [ "$("$WASM_BINDGEN" --version)" != "wasm-bindgen $version" ]; then
  echo "wasm-bindgen-cli must match Cargo.lock ($version)." >&2
  exit 1
fi
"$CARGO" build --locked --release --target wasm32-unknown-unknown --lib -p nwyd "$@"
"$WASM_BINDGEN" "$TARGET_DIR/wasm32-unknown-unknown/release/nwyd.wasm" --target web --out-dir "$TARGET_DIR/web/pkg" --out-name nwyd
CARGO_TARGET_DIR="$TARGET_DIR" WEB_OUT="$WEB_OUT" npm --prefix web run build
cp LICENSE crates/nwyd/assets/LiberationSans-LICENSE.txt "$WEB_OUT/"
printf 'Browser bundle: %s\n' "$WEB_OUT"
