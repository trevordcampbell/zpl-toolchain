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
NUGET_CACHE_ROOT="${NUGET_CACHE_ROOT:-$ROOT_DIR/.cache/nuget}"
mkdir -p "$NUGET_CACHE_ROOT/packages" "$NUGET_CACHE_ROOT/http-cache"
(
  cd "$ROOT_DIR/packages/dotnet"
  LD_LIBRARY_PATH="$ROOT_DIR/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  NUGET_PACKAGES="$NUGET_CACHE_ROOT/packages" \
  NUGET_HTTP_CACHE_PATH="$NUGET_CACHE_ROOT/http-cache" \
    dotnet test ZplToolchain.Tests/ZplToolchain.Tests.csproj -v minimal
)

echo "Done."
