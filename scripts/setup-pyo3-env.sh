#!/usr/bin/env bash
set -euo pipefail

PYTHON_BIN="${PYO3_PYTHON:-$(command -v python3 || true)}"
if [[ -z "${PYTHON_BIN}" ]]; then
  echo "setup-pyo3-env: python3 not found; skipping"
  exit 0
fi

PYTHON_CONFIG="$(dirname "${PYTHON_BIN}")/python3-config"
if [[ ! -x "${PYTHON_CONFIG}" ]]; then
  PYTHON_CONFIG="$(command -v python3-config || true)"
fi
if [[ -z "${PYTHON_CONFIG}" ]]; then
  echo "setup-pyo3-env: python3-config not found; skipping"
  exit 0
fi

ldflags="$("${PYTHON_CONFIG}" --ldflags --embed)"
lib_dirs="$(
  printf '%s\n' "${ldflags}" \
    | tr ' ' '\n' \
    | sed -n 's/^-L//p' \
    | awk 'NF' \
    | paste -sd':' -
)"

env_file="${HOME}/.zpl-pyo3-env"
if ! touch "${env_file}" 2>/dev/null; then
  fallback_dir="${PWD}/.tmp"
  mkdir -p "${fallback_dir}"
  env_file="${fallback_dir}/.zpl-pyo3-env"
fi
cat > "${env_file}" <<EOF
export PYO3_PYTHON="${PYTHON_BIN}"
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
export LIBRARY_PATH="${lib_dirs}\${LIBRARY_PATH:+:\$LIBRARY_PATH}"
export LD_LIBRARY_PATH="${lib_dirs}\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
EOF

for rc in "${HOME}/.bashrc" "${HOME}/.zshrc"; do
  if [[ "${env_file}" == "${HOME}/.zpl-pyo3-env" ]] && [[ -f "${rc}" ]] && ! grep -q '.zpl-pyo3-env' "${rc}"; then
    {
      echo ""
      echo "# zpl-toolchain PyO3 devcontainer environment"
      echo "source \"${env_file}\""
    } >> "${rc}"
  fi
done

echo "setup-pyo3-env: wrote ${env_file}"
echo "setup-pyo3-env: source it now with: source \"${env_file}\""
