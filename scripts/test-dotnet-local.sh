#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but not found on PATH" >&2
  exit 1
fi

if ! command -v dotnet >/dev/null 2>&1; then
  echo "dotnet is required but not found on PATH" >&2
  echo "Tip: rebuild the devcontainer after enabling the dotnet feature." >&2
  exit 1
fi

echo "Building native FFI library..."
cargo build -p zpl_toolchain_ffi --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"

echo "Running .NET wrapper tests..."
(
  cd "$ROOT_DIR/packages/dotnet"
  LD_LIBRARY_PATH="$ROOT_DIR/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    dotnet test ZplToolchain.Tests/ZplToolchain.Tests.csproj -v minimal
)

echo "Done."
