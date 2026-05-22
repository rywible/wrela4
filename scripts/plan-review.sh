#!/usr/bin/env bash
# Build a review packet and run independent plan reviews via Claude Code and Codex.
# See .cursor/skills/multi-model-plan-review/SKILL.md.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/plan-review.sh <plan-path> [worktree-dir] [base-sha]

Arguments:
  plan-path      Path to the implementation plan markdown file
  worktree-dir   Git worktree root (default: current directory)
  base-sha       Start commit for diff (default: merge-base with main)

Environment:
  CLAUDE_BIN, CLAUDE_REVIEW_MODEL, CLAUDE_REVIEW_EFFORT
  CODEX_BIN, CODEX_REVIEW_MODEL, CODEX_REVIEW_REASONING
  PLAN_REVIEW_SKIP_CLAUDE=1  Skip Claude review
  PLAN_REVIEW_SKIP_CODEX=1   Skip Codex review
  PLAN_REVIEW_SKIP_SELF_GATE=1  Skip Phase A APPROVED check (emergency only)
  PLAN_REVIEW_TASKS          Override "tasks claimed complete" text
  PLAN_REVIEW_VERIFICATION_LOG  Inline verification output for the packet

Outputs (under docs/superpowers/reviews/):
  <slug>-review-packet.md
  <slug>-review-claude.md      (if Claude enabled)
  <slug>-review-codex.md       (if Codex enabled)
  <slug>-plan-amendment.md     (optional — included in packet when present)
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -lt 1 ]]; then
  usage
  exit 0
fi

PLAN_PATH="$1"
WORKTREE="${2:-$(pwd)}"
BASE_SHA="${3:-}"

CLAUDE_BIN="${CLAUDE_BIN:-claude}"
CLAUDE_REVIEW_MODEL="${CLAUDE_REVIEW_MODEL:-opus}"
CLAUDE_REVIEW_EFFORT="${CLAUDE_REVIEW_EFFORT:-max}"

CODEX_BIN="${CODEX_BIN:-codex}"
CODEX_REVIEW_MODEL="${CODEX_REVIEW_MODEL:-gpt-5.5}"
CODEX_REVIEW_REASONING="${CODEX_REVIEW_REASONING:-xhigh}"
PLAN_REVIEW_SKIP_CLAUDE="${PLAN_REVIEW_SKIP_CLAUDE:-0}"
PLAN_REVIEW_SKIP_CODEX="${PLAN_REVIEW_SKIP_CODEX:-0}"

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKTREE="$(cd "$WORKTREE" && pwd)"

if [[ "$PLAN_PATH" = /* ]]; then
  PLAN_ABS="$PLAN_PATH"
else
  if [[ -f "$WORKTREE/$PLAN_PATH" ]]; then
    PLAN_ABS="$WORKTREE/$PLAN_PATH"
  else
    PLAN_ABS="$SCRIPT_ROOT/$PLAN_PATH"
  fi
fi

if [[ ! -f "$PLAN_ABS" ]]; then
  echo "error: plan file not found: $PLAN_ABS" >&2
  exit 1
fi

if [[ ! -d "$WORKTREE/.git" && ! -f "$WORKTREE/.git" ]]; then
  echo "error: not a git worktree/repo: $WORKTREE" >&2
  exit 1
fi

HEAD_SHA="$(git -C "$WORKTREE" rev-parse HEAD)"
BRANCH="$(git -C "$WORKTREE" branch --show-current 2>/dev/null || echo "detached")"

if [[ -z "$BASE_SHA" ]]; then
  if git -C "$WORKTREE" rev-parse --verify main >/dev/null 2>&1; then
    BASE_SHA="$(git -C "$WORKTREE" merge-base main HEAD)"
  elif git -C "$WORKTREE" rev-parse --verify origin/main >/dev/null 2>&1; then
    BASE_SHA="$(git -C "$WORKTREE" merge-base origin/main HEAD)"
  else
    BASE_SHA="$(git -C "$WORKTREE" rev-list --max-parents=0 HEAD | tail -1)"
  fi
fi

PLAN_BASENAME="$(basename "$PLAN_ABS" .md)"
SLUG="$PLAN_BASENAME"
REVIEWS_DIR="$WORKTREE/docs/superpowers/reviews"
mkdir -p "$REVIEWS_DIR"

PACKET="$REVIEWS_DIR/${SLUG}-review-packet.md"
CLAUDE_OUT="$REVIEWS_DIR/${SLUG}-review-claude.md"
CODEX_OUT="$REVIEWS_DIR/${SLUG}-review-codex.md"
SELF_OUT="$REVIEWS_DIR/${SLUG}-review-self-thermonuclear.md"
AMENDMENT="$REVIEWS_DIR/${SLUG}-plan-amendment.md"

# Phase A gate: thermo-nuclear self review must be APPROVED before external reviews
if [[ "${PLAN_REVIEW_SKIP_SELF_GATE:-0}" != "1" ]]; then
  if [[ ! -f "$SELF_OUT" ]]; then
    echo "error: Phase A self review missing: $SELF_OUT" >&2
    echo "" >&2
    echo "STOP — Phase B must not run until Phase A is complete." >&2
    echo "Phase A is orchestrator-only (thermo-nuclear skill). Do not run this script" >&2
    echo "or invoke Claude/Codex in the same turn as Phase A." >&2
    echo "" >&2
    echo "Required order:" >&2
    echo "  Turn 1: Phase A → write *-review-self-thermonuclear.md with ## Verdict: APPROVED" >&2
    echo "  Turn 2: ./scripts/plan-review-check-phase-a.sh (optional verify)" >&2
    echo "  Turn 3+: Phase B → this script (Claude + Codex)" >&2
    echo "" >&2
    echo "See .cursor/skills/multi-model-plan-review/SKILL.md § Strict sequencing" >&2
    exit 1
  fi
  if ! rg -q '^## Verdict: APPROVED' "$SELF_OUT" 2>/dev/null; then
    echo "error: Phase A self review not APPROVED: $SELF_OUT" >&2
    echo "Loop Phase A until APPROVED, or set PLAN_REVIEW_SKIP_SELF_GATE=1 (emergency only)." >&2
    exit 1
  fi
  echo "Phase A self review: APPROVED ($SELF_OUT)"
fi

DIFF_STAT="$(git -C "$WORKTREE" diff --stat "$BASE_SHA..$HEAD_SHA" 2>/dev/null || true)"
COMMIT_COUNT="$(git -C "$WORKTREE" rev-list --count "$BASE_SHA..$HEAD_SHA" 2>/dev/null || echo 0)"
FILES_CHANGED="$(git -C "$WORKTREE" diff --name-only "$BASE_SHA..$HEAD_SHA" 2>/dev/null | sed 's/^/- /' || true)"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
TASKS="${PLAN_REVIEW_TASKS:-all tasks in plan (verify against plan AC)}"

VERIFICATION_LOG="${PLAN_REVIEW_VERIFICATION_LOG:-}"
if [[ -z "$VERIFICATION_LOG" && -f "$REVIEWS_DIR/${SLUG}-verification.log" ]]; then
  VERIFICATION_LOG="$(cat "$REVIEWS_DIR/${SLUG}-verification.log")"
fi
if [[ -z "$VERIFICATION_LOG" ]]; then
  VERIFICATION_LOG="(not provided — reviewer should spot-check verification commands from the plan)"
fi

HAS_AMENDMENT=0
if [[ -f "$AMENDMENT" ]]; then
  HAS_AMENDMENT=1
fi

# --- Build review packet ---
{
  cat <<EOF
# Independent Plan Review

<!-- Generated by scripts/plan-review.sh -->

## Plan

- **Path:** \`$PLAN_ABS\`
- **Worktree:** \`$WORKTREE\`
- **Branch:** \`$BRANCH\`
- **BASE_SHA:** \`$BASE_SHA\`
- **HEAD_SHA:** \`$HEAD_SHA\`
- **Generated:** $TIMESTAMP

## Scope

- **Tasks claimed complete:** $TASKS
- **Commits in range:** $COMMIT_COUNT

### Diff stat

\`\`\`
$DIFF_STAT
\`\`\`

### Files changed

$FILES_CHANGED

## Verification already run

The implementer claims these passed before requesting review. Re-run only if you need to verify a fix or doubt the claim.

\`\`\`
$VERIFICATION_LOG
\`\`\`

## Plan acceptance criteria

Read the full plan at \`$PLAN_ABS\`. Audit every task AC and the self-review checklist at the end of the plan.
EOF

  if [[ "$HAS_AMENDMENT" == "1" ]]; then
    cat <<EOF

## Plan amendments

The implementer applied Phase A code-judo simplifications that **changed plan assumptions**. Read this amendment **alongside** the plan — it overrides or clarifies assumptions listed below.

- **Amendment:** \`$AMENDMENT\`

\`\`\`markdown
$(cat "$AMENDMENT")
\`\`\`
EOF
  fi

  cat <<EOF

## Your job

1. Read the plan AC and self-review checklist.
EOF

  if [[ "$HAS_AMENDMENT" == "1" ]]; then
    cat <<EOF
2. Read the plan amendment above and treat changed assumptions as authoritative.
3. Read changed source in the worktree (\`$WORKTREE\`).
4. Spot-check tests and locked decisions (including amended assumptions).
5. Do **not** rubber-stamp. **REJECT** if any AC, locked decision, or amended assumption fails.
EOF
  else
    cat <<EOF
2. Read changed source in the worktree (\`$WORKTREE\`).
3. Spot-check tests and locked decisions.
4. Do **not** rubber-stamp. **REJECT** if any AC or locked decision fails.
EOF
  fi

  cat <<EOF

## Required output format

\`\`\`markdown
## Verdict: APPROVED | REJECTED

## AC audit (task-by-task)
| Task | Result | Evidence |
|------|--------|----------|

## Self-review checklist
| Item | Result |

## Bugs found
(severity + description, or "None")

## Code smells
(or "None")

## Required fixes
(actionable, file-scoped — empty if APPROVED)
\`\`\`
EOF
} > "$PACKET"

echo "Wrote review packet: $PACKET"
if [[ "$HAS_AMENDMENT" == "1" ]]; then
  echo "Included plan amendment in packet: $AMENDMENT"
fi

run_claude() {
  if [[ "$PLAN_REVIEW_SKIP_CLAUDE" == "1" ]]; then
    echo "Skipping Claude review (PLAN_REVIEW_SKIP_CLAUDE=1)"
    return 0
  fi
  if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
    echo "error: Claude CLI not found: $CLAUDE_BIN" >&2
    return 1
  fi

  echo "Running Claude Code review ($CLAUDE_REVIEW_MODEL, effort=$CLAUDE_REVIEW_EFFORT)..."
  CLAUDE_PROMPT="You are an independent plan reviewer. Read the review packet at ${PACKET} and the plan at ${PLAN_ABS}."
  if [[ "$HAS_AMENDMENT" == "1" ]]; then
    CLAUDE_PROMPT="${CLAUDE_PROMPT} Also read the plan amendment at ${AMENDMENT} (embedded in the packet under ## Plan amendments); changed assumptions are authoritative."
  fi
  CLAUDE_PROMPT="${CLAUDE_PROMPT} Audit the implementation in worktree ${WORKTREE} (branch ${BRANCH}, commits ${BASE_SHA}..${HEAD_SHA}). Follow the Required output format in the packet exactly. Be rigorous — REJECT if any AC fails. Output your complete verdict."
  "$CLAUDE_BIN" -p \
    --model "$CLAUDE_REVIEW_MODEL" \
    --effort "$CLAUDE_REVIEW_EFFORT" \
    --permission-mode plan \
    --add-dir "$WORKTREE" \
    --output-format text \
    "$CLAUDE_PROMPT" \
    > "$CLAUDE_OUT" 2>&1
  echo "Wrote Claude verdict: $CLAUDE_OUT"
}

run_codex() {
  if [[ "$PLAN_REVIEW_SKIP_CODEX" == "1" ]]; then
    echo "Skipping Codex review (PLAN_REVIEW_SKIP_CODEX=1)"
    return 0
  fi
  if ! command -v "$CODEX_BIN" >/dev/null 2>&1; then
    echo "error: Codex CLI not found: $CODEX_BIN" >&2
    return 1
  fi

  echo "Running Codex review ($CODEX_REVIEW_MODEL, reasoning=$CODEX_REVIEW_REASONING)..."
  CODEX_PROMPT="You are an independent plan reviewer. Read the review packet at ${PACKET} and the plan at ${PLAN_ABS}."
  if [[ "$HAS_AMENDMENT" == "1" ]]; then
    CODEX_PROMPT="${CODEX_PROMPT} Also read the plan amendment at ${AMENDMENT} (embedded in the packet under ## Plan amendments); changed assumptions are authoritative."
  fi
  CODEX_PROMPT="${CODEX_PROMPT} Audit the implementation in this worktree (branch ${BRANCH}, commits ${BASE_SHA}..${HEAD_SHA}). Follow the Required output format in the packet exactly. Be rigorous — REJECT if any AC fails."
  "$CODEX_BIN" exec \
    -C "$WORKTREE" \
    -m "$CODEX_REVIEW_MODEL" \
    -c "model_reasoning_effort=\"${CODEX_REVIEW_REASONING}\"" \
    -s read-only \
    --ephemeral \
    -o "$CODEX_OUT" \
    "$CODEX_PROMPT" \
    </dev/null
  echo "Wrote Codex verdict: $CODEX_OUT"
}

# Run external reviews in parallel
PIDS=()
FAIL=0

if [[ "$PLAN_REVIEW_SKIP_CLAUDE" != "1" ]]; then
  run_claude &
  PIDS+=($!)
fi

if [[ "$PLAN_REVIEW_SKIP_CODEX" != "1" ]]; then
  run_codex &
  PIDS+=($!)
fi

for pid in "${PIDS[@]}"; do
  if ! wait "$pid"; then
    FAIL=1
  fi
done

echo ""
echo "=== Review artifacts ==="
echo "  Phase A (self):  $REVIEWS_DIR/${SLUG}-review-self-thermonuclear.md  (orchestrator — run BEFORE this script)"
[[ "$HAS_AMENDMENT" == "1" ]] && echo "  Amendment:       $AMENDMENT"
echo "  Packet:          $PACKET"
[[ "$PLAN_REVIEW_SKIP_CLAUDE" != "1" ]] && echo "  Claude:          $CLAUDE_OUT"
[[ "$PLAN_REVIEW_SKIP_CODEX" != "1" ]] && echo "  Codex:           $CODEX_OUT"
echo ""
echo "Gate: Phase A thermo-nuclear self review APPROVED, then Claude and Codex both APPROVED."

exit "$FAIL"
