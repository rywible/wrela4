# Plan completion reviews

Two-phase gate: thermo-nuclear self review, then Claude Code + Codex.

## Strict sequencing

```text
Turn 1  Phase A only  → write *-review-self-thermonuclear.md → stop
Turn 2  Verify Phase A → plan-review-check-phase-a.sh must pass
Turn 3+ Phase B only  → plan-review.sh (Claude + Codex in parallel)
```

## Gate

### Phase A — Self review (loop)

1. `thermo-nuclear-code-quality-review` skill
2. Write `*-review-self-thermonuclear.md` with `## Verdict: APPROVED`
3. Optional: `*-plan-amendment.md` if code-judo changes plan assumptions
4. **End turn**

### Phase B — External reviews (loop)

1. `*-review-claude.md` → **APPROVED**
2. `*-review-codex.md` → **APPROVED**

## Autonomous loops (required)

After **any** fix from Phase A or Phase B feedback, the orchestrator loops autonomously until the gate passes. Do not return to the user mid-loop.

```text
Phase A fix  → quality gate → re-write self verdict → repeat until APPROVED
Phase B fix  → commit → Phase A loop → Phase B loop → repeat until Claude AND Codex APPROVED
```

| Trigger | Action |
|---------|--------|
| Phase A REJECTED | Fix → quality gate → re-write verdict → loop |
| Either Phase B REJECTED | Fix all required items → commit → Phase A loop → re-run **both** Claude and Codex |
| Gate passed | Phase A APPROVED + Claude APPROVED + Codex APPROVED |

## Phase C — Cleanup and merge

After both Phase B reviewers APPROVED:

1. **Delete interim artifacts** from the worktree (not templates):
   - `<slug>-review-*.md`, `<slug>-plan-amendment.md`, `<slug>-verification.log`
2. **Merge** the feature branch into `main`
3. **Remove** the worktree and delete the merged branch

```bash
./scripts/plan-review-cleanup.sh .worktrees/feat-branch docs/implementation/plans/PLAN.md
git checkout main && git merge feat/branch --no-ff
git worktree remove .worktrees/feat-branch
git branch -d feat/branch
```

Interim review outputs are ephemeral — only templates and this README belong on `main`.

| Artifact | Reviewer |
|----------|----------|
| `<slug>-review-self-thermonuclear.md` | Orchestrator (Phase A) |
| `<slug>-plan-amendment.md` | Orchestrator (optional) |
| `<slug>-review-packet.md` | Shared input |
| `<slug>-review-claude.md` | Claude Code (Opus, max) |
| `<slug>-review-codex.md` | Codex CLI (GPT 5.5, xhigh) |

## Quick start

```bash
# Turn 1: Phase A (orchestrator) — write self review + optional amendment

# Turn 2: Verify
./scripts/plan-review-check-phase-a.sh .worktrees/feat-branch docs/implementation/plans/PLAN.md

# Turn 3+: Phase B
./scripts/plan-review-save-verification.sh docs/implementation/plans/PLAN.md .worktrees/feat-branch
./scripts/plan-review.sh docs/implementation/plans/PLAN.md .worktrees/feat-branch
```

## Configuration

| Variable | Default |
|----------|---------|
| `CLAUDE_REVIEW_MODEL` | `opus` |
| `CLAUDE_REVIEW_EFFORT` | `max` |
| `CODEX_REVIEW_MODEL` | `gpt-5.5` |
| `CODEX_REVIEW_REASONING` | `xhigh` |
| `PLAN_REVIEW_SKIP_CLAUDE` | `0` |
| `PLAN_REVIEW_SKIP_CODEX` | `0` |

Smoke test: `./scripts/plan-review-smoke-test.sh`

See `.cursor/skills/multi-model-plan-review/SKILL.md` for full rules.
