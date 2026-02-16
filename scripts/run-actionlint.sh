#!/usr/bin/env bash
#
# Run actionlint with a deterministic version.
# Prefers a locally installed `actionlint` binary, then falls back to `go run`.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ACTIONLINT_VERSION="v1.7.11"
declare -a INPUTS=("$@")
declare -a TARGETS=()

if [ "${#INPUTS[@]}" -eq 0 ]; then
  INPUTS=(".github/workflows")
fi

for input in "${INPUTS[@]}"; do
  if [ -d "$input" ]; then
    while IFS= read -r file; do
      TARGETS+=("$file")
    done < <(printf '%s\n' "$input"/*.yml "$input"/*.yaml)
  else
    TARGETS+=("$input")
  fi
done

# Drop glob literals that did not match any file.
declare -a RESOLVED=()
for target in "${TARGETS[@]}"; do
  if [ -f "$target" ]; then
    RESOLVED+=("$target")
  fi
done

if [ "${#RESOLVED[@]}" -eq 0 ]; then
  echo "No workflow files found to lint; skipping."
  exit 0
fi

if command -v actionlint >/dev/null 2>&1; then
  exec actionlint "${RESOLVED[@]}"
fi

if ! command -v go >/dev/null 2>&1; then
  echo "error: actionlint not found and Go is unavailable." >&2
  echo "Install actionlint or Go, then retry." >&2
  exit 127
fi

CACHE_ROOT="${XDG_CACHE_HOME:-"$ROOT_DIR/.cache"}/go"
mkdir -p "$CACHE_ROOT/pkg/mod" "$ROOT_DIR/.cache/go-build"

GOPATH="$CACHE_ROOT" \
GOMODCACHE="$CACHE_ROOT/pkg/mod" \
GOCACHE="$ROOT_DIR/.cache/go-build" \
  go run "github.com/rhysd/actionlint/cmd/actionlint@${ACTIONLINT_VERSION}" "${RESOLVED[@]}"
