#!/usr/bin/env bash
# Validates install.sh without network access.
# Run from repo root: ./scripts/validate-install-sh.sh
set -euo pipefail
cd "$(dirname "$0")/.."

echo "Validating install.sh..."

# 1. Syntax check (no execution)
echo "  - syntax check (sh -n)"
sh -n install.sh

# 2. Dry-run with explicit version (no API or download)
# Only on supported platforms (Linux x64, Darwin arm64); skip on others
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64|Darwin-arm64|Darwin-aarch64)
    echo "  - dry-run with -v 0.1.12"
    out=$(sh ./install.sh -v 0.1.12 --dry-run 2>&1)
    case "$(uname -s)-$(uname -m)" in
      Linux-x86_64)
        echo "$out" | grep -q "zpl-x86_64-unknown-linux-gnu.tar.gz" || {
          echo "Expected output to mention linux artifact" >&2
          exit 1
        }
        ;;
      Darwin-arm64|Darwin-aarch64)
        echo "$out" | grep -q "zpl-aarch64-apple-darwin.tar.gz" || {
          echo "Expected output to mention darwin artifact" >&2
          exit 1
        }
        ;;
    esac
    ;;
  *)
    echo "  - skip dry-run (platform $(uname -s)/$(uname -m) not in supported set)"
    ;;
esac

echo "install.sh validation passed"
