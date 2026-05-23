#!/usr/bin/env bash
# Create an isolated git worktree for plan implementation.
# Usage: scripts/plan-worktree-new.sh <branch-name> [base-ref]
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/plan-worktree-new.sh <branch-name> [base-ref]

Creates .worktrees/<sanitized-branch> checked out at <branch-name>.
Default base-ref is main (or HEAD if main is missing).

Examples:
  scripts/plan-worktree-new.sh feat/parser
  scripts/plan-worktree-new.sh feat/parser main
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -lt 1 ]]; then
  usage
  exit 0
fi

BRANCH="$1"
BASE_REF="${2:-main}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANITIZED="$(printf '%s' "$BRANCH" | tr '/' '-')"
WORKTREE="$REPO_ROOT/.worktrees/$SANITIZED"

if [[ -e "$WORKTREE" ]]; then
  echo "error: worktree path already exists: $WORKTREE" >&2
  exit 1
fi

if git -C "$REPO_ROOT" rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_SHA="$(git -C "$REPO_ROOT" rev-parse "$BASE_REF")"
else
  BASE_REF="HEAD"
  BASE_SHA="$(git -C "$REPO_ROOT" rev-parse HEAD)"
fi

mkdir -p "$REPO_ROOT/.worktrees"
git -C "$REPO_ROOT" worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"

echo ""
echo "Worktree ready:"
echo "  path:   $WORKTREE"
echo "  branch: $BRANCH"
echo "  base:   $BASE_REF ($BASE_SHA)"
echo ""
echo "Next steps:"
echo "  cd $WORKTREE"
echo "  # implement plan tasks..."
echo "  $REPO_ROOT/scripts/quality-gate.sh $WORKTREE"
echo ""
echo "Review gate (after implementation):"
echo "  # Phase A: write docs/implementation/reviews/<slug>-review-self-thermonuclear.md"
echo "  $REPO_ROOT/scripts/plan-review-check-phase-a.sh $WORKTREE docs/implementation/plans/PLAN.md"
echo "  $REPO_ROOT/scripts/plan-review-save-verification.sh docs/implementation/plans/PLAN.md $WORKTREE"
echo "  $REPO_ROOT/scripts/plan-review.sh docs/implementation/plans/PLAN.md $WORKTREE $BASE_SHA"
echo "  $REPO_ROOT/scripts/plan-review-cleanup.sh $WORKTREE docs/implementation/plans/PLAN.md"
echo "  # merge branch to main, then:"
echo "  git -C $REPO_ROOT worktree remove $WORKTREE"
echo "  git -C $REPO_ROOT branch -d $BRANCH"
