#!/usr/bin/env bash
#
# publish.sh — Publish ZPL toolchain packages to registries.
#
# Usage:
#   ./scripts/publish.sh crates           # dry-run publish 7 crates to crates.io
#   ./scripts/publish.sh crates --live    # actually publish to crates.io
#   ./scripts/publish.sh npm              # dry-run publish TS packages to npm
#   ./scripts/publish.sh npm --live       # actually publish to npm
#   ./scripts/publish.sh pypi             # dry-run publish Python wheel to PyPI
#   ./scripts/publish.sh pypi --live      # actually publish to PyPI
#   ./scripts/publish.sh all              # dry-run all registries
#   ./scripts/publish.sh all --live       # publish to all registries
#
# Tokens are loaded from .env at the project root.
# Dry-run is the default — pass --live to actually publish.

set -euo pipefail

# ── Constants ─────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Crates.io publish order (dependency-aware).
# Tier 1: leaf crates (no inter-crate deps)
TIER1_CRATES=(
    zpl_toolchain_diagnostics
    zpl_toolchain_spec_tables
    zpl_toolchain_profile
    zpl_toolchain_print_client
)
# Tier 2: depend on tier 1
TIER2_CRATES=(
    zpl_toolchain_core
    zpl_toolchain_spec_compiler
)
# Tier 3: depend on tier 2
TIER3_CRATES=(
    zpl_toolchain_cli
)

INDEX_WAIT_SECONDS=60

# ── Colors & output ──────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

STEP_NUM=0

step() {
    STEP_NUM=$((STEP_NUM + 1))
    echo -e "\n${BLUE}${BOLD}[${STEP_NUM}]${RESET} ${BOLD}$1${RESET}"
}

info() {
    echo -e "    ${CYAN}$1${RESET}"
}

success() {
    echo -e "    ${GREEN}$1${RESET}"
}

warn() {
    echo -e "    ${YELLOW}$1${RESET}"
}

error() {
    echo -e "    ${RED}$1${RESET}" >&2
}

# ── Shared utilities ─────────────────────────────────────────────────────────

load_env() {
    local env_file="$ROOT_DIR/.env"
    if [[ ! -f "$env_file" ]]; then
        error "No .env file found at $env_file"
        error "Create one with: crates_api_key, npmjs_api_key, pypi_api_key"
        exit 1
    fi
    # Source the .env file, exporting all variables
    set -a
    # shellcheck disable=SC1090
    source "$env_file"
    set +a
    success "Loaded .env"
}

preflight() {
    local missing=()
    for cmd in "$@"; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required tools: ${missing[*]}"
        exit 1
    fi
    success "Pre-flight OK: $*"
}

require_token() {
    local var_name="$1"
    local display_name="$2"
    if [[ -z "${!var_name:-}" ]]; then
        error "Token not set: $var_name (needed for $display_name)"
        error "Add it to .env: $var_name=<your-token>"
        exit 1
    fi
    success "Token present: $display_name"
}

wait_for_index() {
    local seconds="${1:-$INDEX_WAIT_SECONDS}"
    if [[ "$LIVE" != "true" ]]; then
        info "(dry-run: skipping ${seconds}s index propagation wait)"
        return
    fi
    echo -en "    Waiting ${seconds}s for crates.io index propagation "
    for ((i = seconds; i > 0; i--)); do
        echo -en "\r    Waiting ${i}s for crates.io index propagation  "
        sleep 1
    done
    echo -e "\r    ${GREEN}Index propagation wait complete.${RESET}              "
}

publish_crate() {
    local crate_name="$1"
    local flags=("--allow-dirty" "-p" "$crate_name")

    if [[ "$LIVE" != "true" ]]; then
        flags+=("--dry-run")
        info "(dry-run) cargo publish ${flags[*]}"
    else
        info "cargo publish ${flags[*]}"
    fi

    CARGO_REGISTRY_TOKEN="$crates_api_key" cargo publish "${flags[@]}"
    success "$crate_name published"
}

# ── Subcommands ──────────────────────────────────────────────────────────────

cmd_crates() {
    step "Pre-flight checks (crates.io)"
    preflight cargo
    require_token crates_api_key "crates.io"

    step "Publishing tier 1 crates (leaf — no inter-crate deps)"
    for crate in "${TIER1_CRATES[@]}"; do
        info "Publishing $crate ..."
        publish_crate "$crate"
    done

    step "Waiting for crates.io index to propagate tier 1"
    wait_for_index "$INDEX_WAIT_SECONDS"

    step "Publishing tier 2 crates (depend on tier 1)"
    for crate in "${TIER2_CRATES[@]}"; do
        info "Publishing $crate ..."
        publish_crate "$crate"
    done

    step "Waiting for crates.io index to propagate tier 2"
    wait_for_index "$INDEX_WAIT_SECONDS"

    step "Publishing tier 3 crates (depend on tier 2)"
    for crate in "${TIER3_CRATES[@]}"; do
        info "Publishing $crate ..."
        publish_crate "$crate"
    done

    success "All crates published to crates.io!"
}

cmd_npm() {
    step "Pre-flight checks (npm)"
    preflight wasm-pack npm
    require_token npmjs_api_key "npm"

    local npm_flags=("--access" "public")
    if [[ "$LIVE" != "true" ]]; then
        npm_flags+=("--dry-run")
    fi

    # ── @zpl-toolchain/core (WASM) ────────────────────────────────────────────

    step "Building WASM package"
    info "wasm-pack build crates/wasm --target bundler ..."
    (cd "$ROOT_DIR" && wasm-pack build crates/wasm --target bundler --out-dir ../../packages/ts/core/wasm/pkg)
    success "WASM build complete"

    step "Installing dependencies & building @zpl-toolchain/core"
    (cd "$ROOT_DIR/packages/ts/core" && npm ci && npm run build)
    success "TypeScript build complete"

    step "Configuring npm auth (@zpl-toolchain/core)"
    local npmrc_core="$ROOT_DIR/packages/ts/core/.npmrc"
    echo "//registry.npmjs.org/:_authToken=\${NPM_TOKEN}" > "$npmrc_core"

    step "Publishing @zpl-toolchain/core to npm"
    if [[ "$LIVE" != "true" ]]; then
        info "(dry-run) npm publish ${npm_flags[*]}"
    else
        info "npm publish ${npm_flags[*]}"
    fi
    (cd "$ROOT_DIR/packages/ts/core" && NPM_TOKEN="$npmjs_api_key" npm publish "${npm_flags[@]}")
    rm -f "$npmrc_core"
    success "@zpl-toolchain/core published to npm!"

    # ── @zpl-toolchain/print (pure TS) ────────────────────────────────────────

    step "Building @zpl-toolchain/print"
    (cd "$ROOT_DIR/packages/ts/print" && npm ci && npm run build)
    success "@zpl-toolchain/print build complete"

    step "Configuring npm auth (@zpl-toolchain/print)"
    local npmrc_print="$ROOT_DIR/packages/ts/print/.npmrc"
    echo "//registry.npmjs.org/:_authToken=\${NPM_TOKEN}" > "$npmrc_print"

    step "Publishing @zpl-toolchain/print to npm"
    if [[ "$LIVE" != "true" ]]; then
        info "(dry-run) npm publish ${npm_flags[*]}"
    else
        info "npm publish ${npm_flags[*]}"
    fi
    (cd "$ROOT_DIR/packages/ts/print" && NPM_TOKEN="$npmjs_api_key" npm publish "${npm_flags[@]}")
    rm -f "$npmrc_print"
    success "@zpl-toolchain/print published to npm!"

    # ── @zpl-toolchain/cli (npx wrapper) ──────────────────────────────────────

    step "Testing @zpl-toolchain/cli"
    (cd "$ROOT_DIR/packages/ts/cli" && npm test)
    success "@zpl-toolchain/cli tests passed"

    step "Configuring npm auth (@zpl-toolchain/cli)"
    local npmrc_cli="$ROOT_DIR/packages/ts/cli/.npmrc"
    echo "//registry.npmjs.org/:_authToken=\${NPM_TOKEN}" > "$npmrc_cli"

    step "Publishing @zpl-toolchain/cli to npm"
    if [[ "$LIVE" != "true" ]]; then
        info "(dry-run) npm publish ${npm_flags[*]}"
    else
        info "npm publish ${npm_flags[*]}"
    fi
    (cd "$ROOT_DIR/packages/ts/cli" && NPM_TOKEN="$npmjs_api_key" npm publish "${npm_flags[@]}")
    rm -f "$npmrc_cli"
    success "@zpl-toolchain/cli published to npm!"
}

cmd_pypi() {
    step "Pre-flight checks (PyPI)"
    preflight maturin
    require_token pypi_api_key "PyPI"

    step "Building and publishing Python wheel"
    if [[ "$LIVE" != "true" ]]; then
        info "(dry-run) maturin build --release (skipping upload)"
        (cd "$ROOT_DIR/crates/python" && maturin build --release)
        success "Python wheel built (dry-run — not uploaded)"
    else
        info "maturin publish --token <redacted>"
        (cd "$ROOT_DIR/crates/python" && maturin publish --token "$pypi_api_key")
        success "zpl-toolchain published to PyPI!"
    fi
}

cmd_all() {
    echo -e "${BOLD}Publishing all packages${RESET}"
    if [[ "$LIVE" == "true" ]]; then
        warn "*** LIVE MODE — packages will be published for real ***"
        echo -en "    Press Enter to continue or Ctrl-C to abort... "
        read -r
    fi

    cmd_crates
    cmd_npm
    cmd_pypi

    echo ""
    echo -e "${GREEN}${BOLD}All packages published!${RESET}"
}

# ── Main ─────────────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage: $(basename "$0") <command> [--live]

Commands:
  crates    Publish Rust crates to crates.io (7 crates, dependency-ordered)
  npm       Build WASM + publish @zpl-toolchain/core, @zpl-toolchain/print, @zpl-toolchain/cli to npm
  pypi      Build Python wheel + publish zpl-toolchain to PyPI
  all       Publish to all registries (crates -> npm -> pypi)

Options:
  --live    Actually publish (default is dry-run)
  --help    Show this help

Environment:
  Tokens are loaded from .env at the project root.
  Required variables: crates_api_key, npmjs_api_key, pypi_api_key
EOF
}

COMMAND="${1:-}"
LIVE="false"

# Parse flags
for arg in "$@"; do
    case "$arg" in
        --live) LIVE="true" ;;
        --help|-h) usage; exit 0 ;;
    esac
done

if [[ -z "$COMMAND" || "$COMMAND" == "--help" || "$COMMAND" == "-h" ]]; then
    usage
    exit 0
fi

# Banner
echo -e "${BOLD}╔════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}║    ZPL Toolchain — Package Publisher           ║${RESET}"
echo -e "${BOLD}╚════════════════════════════════════════════════╝${RESET}"
if [[ "$LIVE" == "true" ]]; then
    echo -e "  Mode: ${RED}${BOLD}LIVE${RESET} — packages will be published"
else
    echo -e "  Mode: ${GREEN}${BOLD}DRY-RUN${RESET} — no packages will be uploaded"
fi
echo ""

# Load tokens
step "Loading environment"
load_env

# Dispatch
case "$COMMAND" in
    crates) cmd_crates ;;
    npm)    cmd_npm ;;
    pypi)   cmd_pypi ;;
    all)    cmd_all ;;
    *)
        error "Unknown command: $COMMAND"
        echo ""
        usage
        exit 1
        ;;
esac
