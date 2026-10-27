#!/usr/bin/env bash
# Prepares a Claude Code cloud session when it starts (ADR 0072): Rust if missing, the toolchain
# in rust-toolchain.toml and the crates, so the first check needn't wait. Does nothing elsewhere.
set -euo pipefail
[ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || exit 0
cd "$(dirname "$0")/.."

if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain none
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
        echo "export PATH=\"$HOME/.cargo/bin:\$PATH\"" >> "$CLAUDE_ENV_FILE"
    fi
fi

# Any cargo command installs what rust-toolchain.toml names.
cargo --version
cargo fetch --locked
