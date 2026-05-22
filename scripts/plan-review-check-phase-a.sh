#!/usr/bin/env bash
# Verify Phase A thermo-nuclear self review is complete before Phase B.
# Exit 0 only when *-review-self-thermonuclear.md exists and contains "## Verdict: APPROVED"
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/plan-review-check-phase-a.sh <worktree-dir> <plan-path>

Checks that Phase A self review verdict file exists and is APPROVED.
Run this (or rely on plan-review.sh's built-in gate) before starting Phase B.

Example:
  ./scripts/plan-review-check-phase-a.sh \
    .worktrees/feat-branch \
    docs/superpowers/plans/2026-05-22-feature.md
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -lt 2 ]]; then
  usage
  exit 0
fi

WORKTREE="$(cd "$1" && pwd)"
PLAN_PATH="$2"
SLUG="$(basename "$PLAN_PATH" .md)"
SELF_OUT="$WORKTREE/docs/superpowers/reviews/${SLUG}-review-self-thermonuclear.md"

if [[ ! -f "$SELF_OUT" ]]; then
  echo "PHASE_A: MISSING" >&2
  echo "Expected file: $SELF_OUT" >&2
  echo "" >&2
  echo "Complete Phase A first:" >&2
  echo "  1. Read thermo-nuclear-code-quality-review skill" >&2
  echo "  2. Audit the worktree diff" >&2
  echo "  3. Write verdict to the path above" >&2
  echo "  4. Do NOT start Claude/Codex until this file contains '## Verdict: APPROVED'" >&2
  exit 1
fi

if ! rg -q '^## Verdict: APPROVED' "$SELF_OUT"; then
  echo "PHASE_A: NOT APPROVED" >&2
  echo "File exists but is not APPROVED: $SELF_OUT" >&2
  echo "Fix Required fixes, re-review, update verdict, then run Phase B." >&2
  exit 1
fi

echo "PHASE_A: APPROVED"
echo "File: $SELF_OUT"
exit 0
