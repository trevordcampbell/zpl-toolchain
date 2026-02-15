#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but not found on PATH" >&2
  exit 1
fi

if ! command -v go >/dev/null 2>&1; then
  echo "go is required but not found on PATH" >&2
  echo "Tip: rebuild the devcontainer after enabling the go feature." >&2
  exit 1
fi

echo "Building native FFI library..."
cargo build -p zpl_toolchain_ffi --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"

echo "Running Go wrapper tests..."
(
  cd "$ROOT_DIR/packages/go/zpltoolchain"
  CGO_LDFLAGS="-L$ROOT_DIR/target/release -lzpl_toolchain_ffi" \
  LD_LIBRARY_PATH="$ROOT_DIR/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    go test -v ./...
)

echo "Done."
