#!/usr/bin/env bash
set -euo pipefail

# Updates the Homebrew tap formula to match a released version.
#
# Required env:
#   TAG                 e.g. v0.1.13
#   HOMEBREW_TAP_REPO   e.g. trevordcampbell/homebrew-zpl-toolchain
#   HOMEBREW_TAP_TOKEN  PAT with contents:write on tap repo
#
# Optional env:
#   FORMULA_NAME        default: zpl-toolchain
#   HOMEBREW_TAP_FORMULA_PATH
#                       default: Formula/zpl-toolchain.rb
#                       examples: Formula/zpl-toolchain.rb, zpl-toolchain.rb
#   HOMEBREW_TAP_DRY_RUN
#                       default: false
#                       values: true|false
#   SOURCE_REPO         default: \$GITHUB_REPOSITORY

TAG="${TAG:-}"
TAP_REPO="${HOMEBREW_TAP_REPO:-}"
TAP_TOKEN="${HOMEBREW_TAP_TOKEN:-}"
FORMULA_NAME="${FORMULA_NAME:-zpl-toolchain}"
FORMULA_PATH="${HOMEBREW_TAP_FORMULA_PATH:-Formula/zpl-toolchain.rb}"
SOURCE_REPO="${SOURCE_REPO:-${GITHUB_REPOSITORY:-trevordcampbell/zpl-toolchain}}"
DRY_RUN="${HOMEBREW_TAP_DRY_RUN:-false}"

if [[ -z "$TAG" ]]; then
  echo "TAG is required (example: v0.1.13)" >&2
  exit 1
fi
if [[ -z "$TAP_REPO" ]]; then
  echo "HOMEBREW_TAP_REPO is required (example: owner/homebrew-tap)" >&2
  exit 1
fi
if [[ -z "$TAP_TOKEN" ]]; then
  echo "HOMEBREW_TAP_TOKEN is required" >&2
  exit 1
fi

if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "TAG must be vX.Y.Z (got: $TAG)" >&2
  exit 1
fi
if [[ "${FORMULA_PATH}" != *.rb ]]; then
  echo "HOMEBREW_TAP_FORMULA_PATH must end with .rb (got: ${FORMULA_PATH})" >&2
  exit 1
fi
if [[ "${DRY_RUN}" != "true" && "${DRY_RUN}" != "false" ]]; then
  echo "HOMEBREW_TAP_DRY_RUN must be true or false (got: ${DRY_RUN})" >&2
  exit 1
fi

VERSION="${TAG#v}"
RELEASE_BASE="https://github.com/${SOURCE_REPO}/releases/download/${TAG}"
LINUX_SHA_URL="${RELEASE_BASE}/zpl-x86_64-unknown-linux-gnu.tar.gz.sha256"
DARWIN_SHA_URL="${RELEASE_BASE}/zpl-aarch64-apple-darwin.tar.gz.sha256"

extract_sha() {
  local url="$1"
  local attempt
  local sha

  for attempt in 1 2 3 4 5; do
    if sha="$(curl -fsSL "$url" 2>/dev/null | awk '{print $1}' | tr '[:upper:]' '[:lower:]')"; then
      if [[ "$sha" =~ ^[0-9a-f]{64}$ ]]; then
        echo "$sha"
        return 0
      fi
    fi
    echo "Attempt ${attempt}/5 failed to fetch valid SHA from ${url}; retrying..." >&2
    sleep $((attempt * 2))
  done

  if [[ ! "$sha" =~ ^[0-9a-f]{64}$ ]]; then
    echo "Invalid SHA fetched from ${url}" >&2
    exit 1
  fi
}

LINUX_SHA="$(extract_sha "$LINUX_SHA_URL")"
DARWIN_SHA="$(extract_sha "$DARWIN_SHA_URL")"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

tap_url="https://x-access-token:${TAP_TOKEN}@github.com/${TAP_REPO}.git"
git clone "$tap_url" "$tmp/tap"

formula_path="$tmp/tap/${FORMULA_PATH}"
formula_dir="$(dirname "$formula_path")"
mkdir -p "$formula_dir"

cat >"$formula_path" <<EOF
# typed: false
# frozen_string_literal: true

# Auto-updated by zpl-toolchain release automation.
class ZplToolchain < Formula
  desc "ZPL II toolchain for parsing, validating, formatting, and printing Zebra labels"
  homepage "https://github.com/${SOURCE_REPO}"
  version "${VERSION}"
  license "MIT OR Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/${SOURCE_REPO}/releases/download/${TAG}/zpl-aarch64-apple-darwin.tar.gz"
      sha256 "${DARWIN_SHA}"
    end
    on_intel do
      odie "zpl-toolchain does not provide a pre-built binary for Intel Mac. Use: cargo install zpl_toolchain_cli"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/${SOURCE_REPO}/releases/download/${TAG}/zpl-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${LINUX_SHA}"
    end
    on_arm do
      odie "zpl-toolchain does not provide a pre-built binary for Linux ARM64. Use: cargo install zpl_toolchain_cli"
    end
  end

  def install
    bin.install "zpl"
  end

  test do
    assert_match(/zpl \d+\.\d+/, shell_output("\#{bin}/zpl --version 2>&1"))
  end
end
EOF

pushd "$tmp/tap" >/dev/null

if [[ -z "$(git status --porcelain -- "${FORMULA_PATH}")" ]]; then
  echo "Homebrew formula already up to date for ${TAG}; nothing to commit."
  exit 0
fi

git add "${FORMULA_PATH}"

if [[ "${DRY_RUN}" == "true" ]]; then
  echo "Dry run enabled. Formula changed but no commit/push was performed."
  git --no-pager diff --staged -- "${FORMULA_PATH}"
  exit 0
fi

git -c user.name="github-actions[bot]" -c user.email="github-actions[bot]@users.noreply.github.com" \
  commit -m "chore: update ${FORMULA_NAME} formula for ${TAG}" >/dev/null
git push origin HEAD >/dev/null

echo "Updated ${TAP_REPO}:${FORMULA_PATH} for ${TAG}"
popd >/dev/null
