# Runs the same checks every commit must pass: formatting, Clippy and tests.
# Exit codes are checked explicitly: Windows PowerShell treats cargo's progress output on stderr
# as errors when redirected, so `$ErrorActionPreference = 'Stop'` would abort a passing run.
Set-Location (Split-Path $PSScriptRoot -Parent)

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --workspace
exit $LASTEXITCODE
