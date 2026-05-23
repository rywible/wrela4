#!/usr/bin/env bash
# Save verification output before running plan-review.sh
# Usage: scripts/plan-review-save-verification.sh <plan-path> <worktree-dir>
set -euo pipefail

PLAN="${1:?plan path required}"
WORKTREE="${2:?worktree path required}"
SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKTREE="$(cd "$WORKTREE" && pwd)"
SLUG="$(basename "$PLAN" .md)"
OUT="$WORKTREE/docs/implementation/reviews/${SLUG}-verification.log"

mkdir -p "$(dirname "$OUT")"

{
  echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
  echo "plan: $PLAN"
  echo "worktree: $WORKTREE"
  echo ""

  cd "$WORKTREE"

  if [[ -f Cargo.toml ]]; then
    echo "--- cargo fmt --check ---"
    cargo fmt --check && echo "PASS" || echo "FAIL"
    echo ""

    echo "--- RUSTFLAGS=-D warnings cargo check --all-targets ---"
    RUSTFLAGS="-D warnings" cargo check --all-targets && echo "PASS" || echo "FAIL"
    echo ""

    echo "--- cargo clippy --all-targets -- -D warnings ---"
    cargo clippy --all-targets -- -D warnings && echo "PASS" || echo "FAIL"
    echo ""

    echo "--- cargo test ---"
    cargo test -- --nocapture 2>&1 | tail -30
    echo ""

    echo "--- rg todo/unimplemented/unsafe ---"
    rg -n '\b(todo!|unimplemented!)\s*\(|\bunsafe\b' src --glob '*.rs' 2>/dev/null || echo "PASS (no matches)"
    echo ""

    echo "--- cargo metadata --no-deps ---"
    cargo metadata --no-deps --format-version 1 | rg '"name":"wrela"' || true
  else
    echo "(no Cargo.toml — add project-specific verification commands)"
  fi
} > "$OUT" 2>&1

echo "Wrote $OUT"
