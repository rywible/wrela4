# Plan completion reviews

**Orchestrator gate:** adversarial self review (Phase A) → hand off when genuinely clean → user feedback → merge.

Work happens on a **feature branch** in the main repo checkout — not in a separate git worktree.

Phase A output lives **in the handoff message**, not on disk. Its job is to find problems, not to stamp approval.

## Strict sequencing (orchestrator)

```text
git checkout main && git checkout -b feat/<name>
Implement plan tasks → ./scripts/quality-gate.sh
Phase A  → adversarial self review → fix findings → re-review until clean → hand off
User     → feedback in chat
Fix loop → ./scripts/quality-gate.sh → re-run Phase A if code changed
Merge    → when user directs
```

## Gate

### Phase A — Self review

1. Run `thermo-nuclear-code-quality-review` (see `.cursor/skills/multi-model-plan-review/SKILL.md`)
2. Report findings honestly in the handoff message
3. **`Verdict: APPROVED`** — no unresolved findings (or only **Explicit disagreements** with rationale)
4. **`Verdict: NOT APPROVED`** — open findings remain; fix and re-review. **Do not hand off.**

**Forbidden:** rubber-stamping APPROVED; deferring findings without documented disagreement.

### User feedback

The user reviews the feature branch and provides feedback. Implement **every suggestion** — all severities, including low priority, maintenance smells, nice-to-have, and items labeled deferred or non-blocking.

**Only exception:** explicit disagreement documented in the next Phase A handoff (item + rationale).

Re-run Phase A after code changes. Do not hand off or merge with open non-disagreed items.

### Fix-everything policy (Phase A + feedback)

| Source | Rule |
|--------|------|
| Phase A self-review | Fix every finding before honest `Verdict: APPROVED` |
| User / reviewer feedback | Fix every listed suggestion before next handoff or merge |
| Exception | Explicit disagreement only — must be documented |

### Merge

After user feedback is resolved and the user asks to merge:

```bash
git checkout main && git merge feat/branch --no-ff
./scripts/quality-gate.sh
git branch -d feat/branch
```

## Quick start

```bash
git checkout main && git checkout -b feat/my-feature
# implement tasks, then:
./scripts/quality-gate.sh
# Phase A: adversarial self review — fix findings until Verdict: APPROVED is honest
# → user feedback → fix → merge when directed
```

There is **no CI** — run `./scripts/quality-gate.sh` locally before merge.

See `.cursor/skills/multi-model-plan-review/SKILL.md` for full rules.
