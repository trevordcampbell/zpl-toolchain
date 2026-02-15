#!/usr/bin/env bash
#
# Rebuild the WASM + TypeScript core runtime artifacts consumed by the
# VS Code extension freshness guard.

set -euo pipefail

if ! command -v wasm-pack &>/dev/null; then
  echo ""
  echo "  wasm-pack is required to refresh core runtime artifacts."
  echo "  Install it with: cargo binstall wasm-pack  (or cargo install wasm-pack)"
  echo ""
  exit 1
fi

if ! command -v npm &>/dev/null; then
  echo ""
  echo "  npm is required to rebuild packages/ts/core."
  echo ""
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Keep tool temp/cache paths inside the repo to avoid host tmp permission issues.
export TMPDIR="${repo_root}/.tmp"
export CARGO_HOME="${repo_root}/.cargo-home"
export CARGO_TARGET_DIR="${repo_root}/target"
export PATH="${CARGO_HOME}/bin:${PATH}"
mkdir -p "$TMPDIR" "$CARGO_HOME/bin"

# Avoid wasm-pack attempting a global wasm-bindgen install in restricted envs.
if ! command -v wasm-bindgen &>/dev/null; then
  echo "refresh-core-runtime: installing wasm-bindgen-cli into repo-local cargo home..."
  cargo install wasm-bindgen-cli --locked --version 0.2.108 --root "$CARGO_HOME"
fi

echo "refresh-core-runtime: rebuilding WASM package..."
wasm-pack build crates/wasm --mode no-install --no-opt --target bundler --out-dir ../../packages/ts/core/wasm/pkg

echo "refresh-core-runtime: rebuilding @zpl-toolchain/core..."
(
  cd packages/ts/core
  npm run build
)

echo "refresh-core-runtime: done."
