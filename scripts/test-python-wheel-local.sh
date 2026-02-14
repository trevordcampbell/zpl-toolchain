#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist/python-wheel-tests"
VENV_DIR="$ROOT_DIR/.venv-python-wheel-tests"

if ! command -v python >/dev/null 2>&1; then
  echo "python is required but not found on PATH" >&2
  exit 1
fi

if ! command -v maturin >/dev/null 2>&1; then
  echo "maturin is required but not found on PATH" >&2
  exit 1
fi

echo "Preparing build output directory..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

echo "Building Python wheel..."
maturin build -m "$ROOT_DIR/crates/python/Cargo.toml" --out "$DIST_DIR"

echo "Installing wheel..."
rm -rf "$VENV_DIR"
python -m venv "$VENV_DIR"
source "$VENV_DIR/bin/activate"
python -m pip install --force-reinstall "$DIST_DIR"/*.whl

echo "Running Python binding tests..."
python -m unittest discover -s "$ROOT_DIR/crates/python/tests" -v

echo "Done."
