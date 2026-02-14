#!/usr/bin/env sh
#
# zpl-toolchain installer — Linux/macOS
#
# Downloads the pre-built CLI binary from GitHub Releases with SHA-256 checksum
# verification. Fails closed if checksum is unavailable or mismatched.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/trevordcampbell/zpl-toolchain/main/install.sh | sh
#   # or for a specific version:
#   curl -fsSL ... | sh -s -- -v 0.1.12
#
# Supported targets (aligned with packages/ts/cli/lib/platform.mjs):
#   - linux/x64   → x86_64-unknown-linux-gnu (tar.gz)
#   - darwin/arm64 → aarch64-apple-darwin (tar.gz)
#
# Unsupported: darwin/x64 (Intel Mac), linux/arm64, Windows.
# For unsupported platforms use: cargo install zpl_toolchain_cli
#
set -eu

# ─── Configuration ──────────────────────────────────────────────────────────
REPO="trevordcampbell/zpl-toolchain"
# Default to user-owned path to avoid sudo prompts.
INSTALL_DIR="${ZPL_INSTALL_DIR:-$HOME/.local/bin}"
VERSION=""
DRY_RUN=false

# Parse optional -v/--version and --dry-run
while [ $# -gt 0 ]; do
  case "$1" in
    -v|--version)
      if [ -z "${2:-}" ]; then
        echo "-v/--version requires a value (e.g. -v 0.1.12)" >&2
        exit 1
      fi
      VERSION="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    *)
      shift
      ;;
  esac
done

validate_version() {
  # Accept v-prefixed or bare semver (x.y.z).
  ver="$1"
  original="$1"
  case "$ver" in
    v*) core="${ver#v}" ;;
    *) core="$ver" ;;
  esac

  # Only digits and dots are allowed.
  case "$core" in
    *[!0-9.]*|'')
      echo "Invalid version '$original' (expected x.y.z or vX.Y.Z)" >&2
      exit 1
      ;;
  esac

  # Must be exactly 3 numeric components separated by dots.
  if [ "${core#*.}" = "$core" ]; then
    echo "Invalid version '$original' (expected x.y.z or vX.Y.Z)" >&2
    exit 1
  fi
  major="${core%%.*}"
  rest="${core#*.}"
  if [ "${rest#*.}" = "$rest" ]; then
    echo "Invalid version '$original' (expected x.y.z or vX.Y.Z)" >&2
    exit 1
  fi
  minor="${rest%%.*}"
  patch="${rest#*.}"
  if [ "${patch#*.}" != "$patch" ] || [ -z "$major" ] || [ -z "$minor" ] || [ -z "$patch" ]; then
    echo "Invalid version '$original' (expected x.y.z or vX.Y.Z)" >&2
    exit 1
  fi
  case "$major$minor$patch" in
    *[!0-9]*)
      echo "Invalid version '$original' (expected x.y.z or vX.Y.Z)" >&2
      exit 1
      ;;
  esac
}

# ─── Platform detection (aligned with platform.mjs) ──────────────────────────
detect_platform() {
  OS="$(uname -s)"
  ARCH="$(uname -m)"

  case "$OS" in
    Linux)
      case "$ARCH" in
        x86_64)
          TARGET="x86_64-unknown-linux-gnu"
          ARCHIVE_EXT="tar.gz"
          BINARY_NAME="zpl"
          ;;
        *)
          echo "Unsupported architecture: $ARCH (expected x86_64)" >&2
          echo "Use: cargo install zpl_toolchain_cli" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$ARCH" in
        arm64|aarch64)
          TARGET="aarch64-apple-darwin"
          ARCHIVE_EXT="tar.gz"
          BINARY_NAME="zpl"
          ;;
        x86_64)
          echo "Intel Mac (darwin/x64) is not supported — no pre-built binary." >&2
          echo "Use: cargo install zpl_toolchain_cli" >&2
          exit 1
          ;;
        *)
          echo "Unsupported architecture: $ARCH (expected arm64)" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "Unsupported OS: $OS (expected Linux or Darwin)" >&2
      exit 1
      ;;
  esac
}

# ─── Resolve version from GitHub API (latest if not specified) ───────────────
resolve_version() {
  if [ -n "$VERSION" ]; then
    validate_version "$VERSION"
    printf '%s' "$VERSION"
    return
  fi
  V=$(fetch_json "https://api.github.com/repos/$REPO/releases/latest")
  if [ -z "$V" ]; then
    echo "Failed to resolve latest version from GitHub" >&2
    exit 1
  fi
  validate_version "$V"
  V="${V#v}"
  printf '%s' "$V"
}

# Minimal JSON field extraction (avoids jq for portability)
fetch_json() {
  URL="$1"
  RES=$(fetch_url "$URL")
  # Extract tag_name: flatten newlines, then sed for "tag_name":"v0.1.12"
  echo "$RES" | tr -d '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1
}

# ─── HTTP fetch (curl or wget) ────────────────────────────────────────────────
fetch_url() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  else
    echo "Need curl or wget to download" >&2
    exit 1
  fi
}

fetch_to_file() {
  URL="$1"
  OUT="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$OUT" "$URL"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$OUT" "$URL"
  else
    echo "Need curl or wget to download" >&2
    exit 1
  fi
}

# ─── Checksum verification (fail closed) ─────────────────────────────────────
verify_checksum() {
  ARCHIVE="$1"
  EXPECTED_FILE="$2"
  # Expected format: "hash  filename" or "hash  " (sha256sum output)
  if [ ! -f "$EXPECTED_FILE" ]; then
    echo "Checksum file not found — aborting (fail closed)" >&2
    exit 1
  fi
  EXPECTED=$(head -1 "$EXPECTED_FILE" | awk '{print $1}' | tr '[:upper:]' '[:lower:]')
  if [ -z "$EXPECTED" ] || [ ${#EXPECTED} -ne 64 ]; then
    echo "Checksum format invalid or missing — aborting (fail closed)" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$ARCHIVE" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
  else
    echo "Need sha256sum or shasum to verify checksum" >&2
    exit 1
  fi
  if [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "Checksum mismatch — aborting (fail closed)" >&2
    echo "  expected: $EXPECTED" >&2
    echo "  actual:   $ACTUAL" >&2
    exit 1
  fi
}

# ─── Main ────────────────────────────────────────────────────────────────────
main() {
  detect_platform
  VER=$(resolve_version)
  TAG="v${VER}"
  ARCHIVE_NAME="zpl-${TARGET}.${ARCHIVE_EXT}"
  BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"
  ARCHIVE_URL="${BASE_URL}/${ARCHIVE_NAME}"
  CHECKSUM_URL="${ARCHIVE_URL}.sha256"

  echo "zpl-toolchain installer"
  echo "  version: $VER"
  echo "  target:  $TARGET"
  echo "  install: $INSTALL_DIR"
  echo

  if [ "$DRY_RUN" = true ]; then
    echo "[dry-run] Would download: $ARCHIVE_URL"
    echo "[dry-run] Would verify:    $CHECKSUM_URL"
    exit 0
  fi

  TMPDIR=$(mktemp -d)
  trap 'rm -rf "$TMPDIR"' EXIT

  ARCHIVE_PATH="$TMPDIR/$ARCHIVE_NAME"
  echo "Downloading $ARCHIVE_NAME..."
  if ! fetch_to_file "$ARCHIVE_URL" "$ARCHIVE_PATH"; then
    echo "Download failed" >&2
    exit 1
  fi

  echo "Downloading checksum..."
  CHECKSUM_PATH="$TMPDIR/${ARCHIVE_NAME}.sha256"
  if ! fetch_to_file "$CHECKSUM_URL" "$CHECKSUM_PATH"; then
    echo "Checksum download failed — aborting (fail closed)" >&2
    exit 1
  fi

  echo "Verifying checksum..."
  verify_checksum "$ARCHIVE_PATH" "$CHECKSUM_PATH"

  echo "Extracting..."
  EXTRACT_DIR="$TMPDIR/extract"
  mkdir -p "$EXTRACT_DIR"
  tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"

  BINARY_PATH="$EXTRACT_DIR/$BINARY_NAME"
  if [ ! -f "$BINARY_PATH" ]; then
    echo "Archive did not contain $BINARY_NAME" >&2
    exit 1
  fi

  mkdir -p "$INSTALL_DIR"
  INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"
  if [ -w "$INSTALL_DIR" ]; then
    cp "$BINARY_PATH" "$INSTALL_PATH"
    chmod 755 "$INSTALL_PATH"
  else
    echo "Installing to $INSTALL_PATH (may require sudo)"
    sudo cp "$BINARY_PATH" "$INSTALL_PATH"
    sudo chmod 755 "$INSTALL_PATH"
  fi

  echo
  echo "Installed zpl to $INSTALL_PATH"
  "$INSTALL_PATH" --version 2>/dev/null || true
}

main
