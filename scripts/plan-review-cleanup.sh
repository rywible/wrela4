#!/usr/bin/env bash
# Delete interim plan-review artifacts from a worktree before merge.
# See .cursor/skills/multi-model-plan-review/SKILL.md § Phase C.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/plan-review-cleanup.sh <worktree-dir> <plan-path>

Removes ephemeral review outputs for the plan slug:
  *-review-self-thermonuclear.md
  *-review-claude.md
  *-review-codex.md
  *-review-packet.md
  *-plan-amendment.md
  *-verification.log

Templates under docs/superpowers/reviews/ on main are not touched.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -lt 2 ]]; then
  usage
  exit 0
fi

WORKTREE="$1"
PLAN_PATH="$2"

WORKTREE="$(cd "$WORKTREE" && pwd)"
PLAN_BASENAME="$(basename "$PLAN_PATH" .md)"
SLUG="$PLAN_BASENAME"
REVIEWS_DIR="$WORKTREE/docs/superpowers/reviews"

if [[ ! -d "$REVIEWS_DIR" ]]; then
  echo "nothing to clean: $REVIEWS_DIR does not exist"
  exit 0
fi

PATTERNS=(
  "${SLUG}-review-self-thermonuclear.md"
  "${SLUG}-review-claude.md"
  "${SLUG}-review-codex.md"
  "${SLUG}-review-packet.md"
  "${SLUG}-plan-amendment.md"
  "${SLUG}-verification.log"
)

removed=0
for name in "${PATTERNS[@]}"; do
  path="$REVIEWS_DIR/$name"
  if [[ -f "$path" ]]; then
    rm -f "$path"
    echo "removed: $path"
    removed=$((removed + 1))
  fi
done

if [[ "$removed" -eq 0 ]]; then
  echo "no interim artifacts found for slug: $SLUG"
else
  echo "cleaned $removed interim artifact(s) for $SLUG"
fi
