#!/usr/bin/env bash
# Smoke test for plan-review Phase B tooling. Does NOT run full plan reviews.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLAN="${2:-docs/implementation/plans/2026-05-22-lexer-and-initial-rust-setup.md}"
SLUG="$(basename "$PLAN" .md)"
SMOKE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/plan-review-smoke.XXXXXX")"
TEMP_WORKTREE=0

pass=0
fail=0

ok() { echo "  PASS: $1"; pass=$((pass + 1)); }
bad() { echo "  FAIL: $1" >&2; fail=$((fail + 1)); }

cleanup() {
  if [[ "$TEMP_WORKTREE" == "1" && -d "${WORKTREE:-}" ]]; then
    git -C "$REPO_ROOT" worktree remove --force "$WORKTREE" 2>/dev/null || true
  fi
  rm -rf "$SMOKE_DIR"
}
trap cleanup EXIT

# Worktree: use arg1 if provided and exists; otherwise create a detached temp worktree.
if [[ $# -ge 1 && "$1" != -* && -d "$1" ]]; then
  WORKTREE="$(cd "$1" && pwd)"
  shift
  PLAN="${1:-$PLAN}"
else
  WORKTREE="$SMOKE_DIR/worktree"
  git -C "$REPO_ROOT" worktree add --detach "$WORKTREE" HEAD >/dev/null
  TEMP_WORKTREE=1
  mkdir -p "$WORKTREE/docs/implementation/reviews"
  cat > "$WORKTREE/docs/implementation/reviews/${SLUG}-review-self-thermonuclear.md" <<EOF
# Smoke test stub (Phase A)
## Verdict: APPROVED
EOF
fi

echo "=== plan-review smoke test ==="
echo "repo:     $REPO_ROOT"
echo "worktree: $WORKTREE"
echo "plan:     $PLAN"
echo "tmpdir:   $SMOKE_DIR"
echo ""

# 1. Scripts exist and are executable
echo "[1] Script presence"
for s in \
  quality-gate.sh \
  plan-worktree-new.sh \
  plan-review.sh \
  plan-review-check-phase-a.sh \
  plan-review-save-verification.sh \
  plan-review-cleanup.sh; do
  if [[ -x "$REPO_ROOT/scripts/$s" ]]; then
    ok "scripts/$s executable"
  else
    bad "scripts/$s missing or not executable"
  fi
done

# 2. Phase A gate — should pass on worktree with stub verdict
echo ""
echo "[2] Phase A gate (expect APPROVED on worktree)"
if "$REPO_ROOT/scripts/plan-review-check-phase-a.sh" "$WORKTREE" "$PLAN" >/dev/null 2>&1; then
  ok "plan-review-check-phase-a.sh"
else
  bad "plan-review-check-phase-a.sh (Phase A not APPROVED or file missing)"
fi

# 3. Phase A gate — should fail without verdict file
echo ""
echo "[3] Phase A gate rejects missing verdict"
FAKE_WT="$SMOKE_DIR/fake-worktree"
mkdir -p "$FAKE_WT/docs/implementation/reviews"
if [[ -f "$WORKTREE/.git" ]]; then
  cp "$WORKTREE/.git" "$FAKE_WT/.git"
else
  echo "gitdir: $(git -C "$WORKTREE" rev-parse --git-dir)" > "$FAKE_WT/.git"
fi
if "$REPO_ROOT/scripts/plan-review-check-phase-a.sh" "$FAKE_WT" "$PLAN" >/dev/null 2>&1; then
  bad "check-phase-a should fail without verdict file"
else
  ok "check-phase-a rejects missing verdict"
fi

# 4. Packet generation (skip external reviewers)
echo ""
echo "[4] Packet generation (skip Claude/Codex)"
if PLAN_REVIEW_SKIP_CLAUDE=1 PLAN_REVIEW_SKIP_CODEX=1 \
  "$REPO_ROOT/scripts/plan-review.sh" "$PLAN" "$WORKTREE" >/dev/null 2>&1; then
  PACKET="$WORKTREE/docs/implementation/reviews/${SLUG}-review-packet.md"
  if [[ -s "$PACKET" ]] && rg -q '^# Independent Plan Review' "$PACKET"; then
    ok "review packet generated ($PACKET)"
  else
    bad "review packet missing or empty"
  fi
else
  bad "plan-review.sh packet generation failed"
fi

# 5. plan-review.sh blocks without Phase A
echo ""
echo "[5] plan-review.sh blocks without Phase A verdict"
BLOCK_WT="$SMOKE_DIR/block-worktree"
mkdir -p "$BLOCK_WT/docs/implementation/reviews"
if [[ -f "$WORKTREE/.git" ]]; then
  cp "$WORKTREE/.git" "$BLOCK_WT/.git"
else
  echo "gitdir: $(git -C "$WORKTREE" rev-parse --git-dir)" > "$BLOCK_WT/.git"
fi
if PLAN_REVIEW_SKIP_CLAUDE=1 PLAN_REVIEW_SKIP_CODEX=1 \
  "$REPO_ROOT/scripts/plan-review.sh" "$PLAN" "$BLOCK_WT" >/dev/null 2>&1; then
  bad "plan-review.sh should fail without Phase A file"
else
  ok "plan-review.sh rejects missing Phase A"
fi

# 6. Claude CLI minimal invoke
echo ""
echo "[6] Claude CLI smoke (minimal -p)"
CLAUDE_BIN="${CLAUDE_BIN:-claude}"
CLAUDE_OUT="$SMOKE_DIR/claude-smoke.txt"
if command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  if "$CLAUDE_BIN" -p \
    --model "${CLAUDE_REVIEW_MODEL:-opus}" \
    --effort low \
    --permission-mode plan \
    --output-format text \
    "Reply with exactly the single word SMOKE_OK and nothing else." \
    > "$CLAUDE_OUT" 2>"$SMOKE_DIR/claude-smoke.err"; then
    if rg -q 'SMOKE_OK' "$CLAUDE_OUT"; then
      ok "Claude CLI responded ($CLAUDE_BIN)"
    else
      bad "Claude CLI ran but unexpected output: $(head -c 200 "$CLAUDE_OUT")"
    fi
  else
    bad "Claude CLI invocation failed: $(head -3 "$SMOKE_DIR/claude-smoke.err" 2>/dev/null)"
  fi
else
  bad "Claude CLI not found: $CLAUDE_BIN"
fi

# 7. Codex CLI minimal invoke
echo ""
echo "[7] Codex CLI smoke (minimal exec)"
CODEX_BIN="${CODEX_BIN:-codex}"
CODEX_OUT="$SMOKE_DIR/codex-smoke.txt"
if command -v "$CODEX_BIN" >/dev/null 2>&1; then
  if "$CODEX_BIN" exec \
    -C "$WORKTREE" \
    -m "${CODEX_REVIEW_MODEL:-gpt-5.5}" \
    -c 'model_reasoning_effort="low"' \
    -s read-only \
    --ephemeral \
    -o "$CODEX_OUT" \
    "Reply with exactly the single word SMOKE_OK and nothing else." \
    </dev/null \
    2>"$SMOKE_DIR/codex-smoke.err"; then
    if [[ -s "$CODEX_OUT" ]] && rg -q 'SMOKE_OK' "$CODEX_OUT"; then
      ok "Codex CLI responded ($CODEX_BIN)"
    else
      bad "Codex CLI ran but unexpected output: $(head -c 200 "$CODEX_OUT" 2>/dev/null)"
    fi
  else
    bad "Codex CLI invocation failed: $(head -3 "$SMOKE_DIR/codex-smoke.err" 2>/dev/null)"
  fi
else
  bad "Codex CLI not found: $CODEX_BIN"
fi

echo ""
echo "=== smoke test summary: $pass passed, $fail failed ==="
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
exit 0
