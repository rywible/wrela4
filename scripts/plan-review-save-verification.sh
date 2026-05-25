#!/usr/bin/env bash
# Save verification output before running plan-review.sh
# Usage: scripts/plan-review-save-verification.sh <plan-path> <repo-root>
set -euo pipefail

PLAN="${1:?plan path required}"
WORKTREE="${2:?repo root required}"
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
  "$SCRIPT_ROOT/scripts/quality-gate.sh" "$WORKTREE"
} > "$OUT" 2>&1 || true

echo "Wrote $OUT"
