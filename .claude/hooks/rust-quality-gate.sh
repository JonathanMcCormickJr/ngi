#!/bin/bash
# .claude/hooks/rust-quality-gate.sh
#
# Runs after every file edit. Enforces:
#   cargo test --workspace
#   cargo tarpaulin --workspace
#   cargo fmt
#   cargo audit
#
# Exit 2 feeds the combined output back to Claude as an error,
# forcing it to iterate until all checks pass.

set -euo pipefail

ERRORS=""
PASSED=""

run_check() {
  local label="$1"
  shift
  local output
  if output=$("$@" 2>&1); then
    PASSED="${PASSED}✅ ${label}\n"
  else
    ERRORS="${ERRORS}❌ ${label} FAILED:\n${output}\n\n"
  fi
}

# ── 1. Tests ────────────────────────────────────────────────────────────────
run_check "cargo test --workspace" cargo test --workspace

# ── 2. Coverage (tarpaulin) ─────────────────────────────────────────────────
run_check "cargo tarpaulin --workspace" cargo tarpaulin --workspace

# ── 3. Formatting ───────────────────────────────────────────────────────────
# --check exits non-zero when files are not already formatted.
# We run fmt to fix, then verify with --check so the output is meaningful.
cargo fmt --all 2>&1  # auto-fix first (silent)
run_check "cargo fmt --check" cargo fmt --all -- --check

# ── 4. Security audit ───────────────────────────────────────────────────────
run_check "cargo audit" cargo audit

# ── Report ──────────────────────────────────────────────────────────────────
if [ -n "$ERRORS" ]; then
  printf "Rust quality gate FAILED. Fix all issues before proceeding.\n\n"
  printf "%b" "$ERRORS"
  printf "Passed checks:\n%b" "$PASSED"
  exit 2   # blocking: Claude sees this on stderr and must keep iterating
fi

printf "%b" "$PASSED"
exit 0
