#!/usr/bin/env bash
# The checks that run on Linux, such as in a Claude Code cloud session (ADR 0072): formatting of
# everything, Clippy and tests of gracil-core, and Clippy of gracil-windows for Windows without
# building it. gracil-ui builds only on Windows; GitHub runs scripts/check.ps1 there on every push.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy -p gracil-core --all-targets -- -D warnings
cargo clippy -p gracil-windows --target x86_64-pc-windows-msvc -- -D warnings
cargo test -p gracil-core
