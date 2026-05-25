# Plan completion reviews

**Orchestrator gate:** thermo-nuclear self review (Phase A) → user feedback → merge (Phase C).

Work happens on a **feature branch** in the main repo checkout — not in a separate git worktree.

Automated external review via Claude Code + Codex (`plan-review.sh`) is **optional** and not part of the agent completion gate.

## Strict sequencing (orchestrator)

```text
git checkout main && git checkout -b feat/<name>
Implement plan tasks → quality gate
Phase A  → write *-review-self-thermonuclear.md → APPROVED → return to user
User     → feedback in chat
Fix loop → quality gate → re-run Phase A if code changed
Phase C  → cleanup artifacts → merge to main → delete branch (when user directs)
```

## Gate

### Phase A — Self review (loop until handoff)

1. `thermo-nuclear-code-quality-review` skill
2. Write `docs/implementation/reviews/<slug>-review-self-thermonuclear.md` with `## Verdict: APPROVED`
3. Optional: `*-plan-amendment.md` if code-judo changes plan assumptions
4. `./scripts/plan-review-check-phase-a.sh . docs/implementation/plans/PLAN.md` must pass
5. **Return to user** for feedback

### User feedback (replaces automated Phase B)

The user reviews the feature branch and provides feedback. Implement **every suggestion** — all severities, including low priority, maintenance smells, nice-to-have, and items labeled deferred or non-blocking.

**Only exception:** explicit disagreement documented in the next Phase A verdict (**Explicit disagreements** section: item + rationale).

Re-run Phase A after code changes. Do not hand off or merge with open non-disagreed items.

### Fix-everything policy (Phase A + feedback)

| Source | Rule |
|--------|------|
| Phase A self-review | Fix every finding in every section before `## Verdict: APPROVED` |
| User / reviewer feedback | Fix every listed suggestion before next handoff or merge |
| Exception | Explicit disagreement only — must be documented |

**Forbidden:** deferring because of priority labels; “Deferred” / “non-blocking” sections in APPROVED verdicts without per-item documented disagreement.

### Phase C — Cleanup and merge

After user feedback is resolved and the user asks to merge:

```bash
./scripts/plan-review-cleanup.sh . docs/implementation/plans/PLAN.md
git checkout main && git merge feat/branch --no-ff
./scripts/quality-gate.sh
git branch -d feat/branch
```

Interim review outputs are ephemeral — only templates and this README belong on `main`.

| Artifact | Who |
|----------|-----|
| `<slug>-review-self-thermonuclear.md` | Orchestrator (Phase A) |
| `<slug>-plan-amendment.md` | Orchestrator (optional) |
| `<slug>-verification.log` | Orchestrator (optional) |
| `<slug>-review-claude.md`, `<slug>-review-codex.md` | Optional manual CLI only |

## Quick start

```bash
git checkout main && git checkout -b feat/my-feature
./scripts/quality-gate.sh
./scripts/plan-review-save-verification.sh docs/implementation/plans/PLAN.md .
# Phase A: write self review
./scripts/plan-review-check-phase-a.sh . docs/implementation/plans/PLAN.md
# → user feedback → fix → merge when directed
```

Review scripts accept the **repo root** as path — use `.` on the feature branch.

## Optional: manual external review

```bash
./scripts/plan-review.sh docs/implementation/plans/PLAN.md .
```

Not required for orchestrator handoff or merge. See script env vars and `./scripts/plan-review-smoke-test.sh`.

There is **no CI** — run `./scripts/quality-gate.sh` locally before merge.

See `.cursor/skills/multi-model-plan-review/SKILL.md` for full rules.
