---
name: multi-model-plan-review
description: Run thermo-nuclear self-review before returning from plan implementation; user provides feedback; then merge. Use when implementing plans from docs/implementation/plans/.
---

# Plan review (Phase A + user feedback)

Gate before handing a plan implementation back to the user:

1. **Phase A — Self review:** adversarial thermo-nuclear review; fix what it finds; hand off only when genuinely clean
2. **User feedback:** the user reviews the work and provides feedback in chat
3. **Post-feedback:** address feedback → quality gate → re-run Phase A if code changed → merge

---

## Branch setup

Implement on a **feature branch** in the main repo checkout:

```bash
git checkout main
git checkout -b feat/<name>
```

Stay in the same workspace. Do not use a separate git worktree unless the user explicitly asks.

---

## Completion gate (orchestrator)

**Return to the user only when all of these are true:**

- All plan tasks implemented on the feature branch
- `./scripts/quality-gate.sh` passes
- Phase A self review found **no unresolved findings** (see verdict rules below)

**Do not merge** until the user has reviewed and you have worked through their feedback.

If Phase A still has open findings, **keep working** — do not hand off yet.

---

## Phase A — Thermo-nuclear self review

See skill file at `~/.cursor/plugins/cache/cursor-public/cursor-team-kit/*/skills/thermo-nuclear-code-quality-review/SKILL.md`.

**Purpose:** find real problems — correctness gaps, missing tests, locked-decision violations, maintainability risks. This is not a formality.

**Do not write a review file to disk.** Return the review in your handoff message.

### Verdict rules

| Outcome | Meaning | Next step |
|---------|---------|-----------|
| **`Verdict: APPROVED`** | No unresolved findings, or every remaining skip is under **Explicit disagreements** with rationale | Hand off to user |
| **`Verdict: NOT APPROVED`** | One or more findings still open | Fix → quality gate → re-review. **Do not hand off.** |

**Forbidden:** rubber-stamping APPROVED while findings remain; “Deferred” / “non-blocking” lists without a documented disagreement per item; skipping P2+ because they look optional.

### Handoff format (when APPROVED)

Include a **Self review** section with:

- What you checked (diff scope, tests run, plan AC)
- Findings by category — or an honest “none found” after adversarial review
- **`Verdict: APPROVED`** or **`Verdict: NOT APPROVED`**
- **Explicit disagreements** (if any): item skipped + rationale

Use APPROVED only when you would stake the branch on it. If anything material is still open, use NOT APPROVED and keep looping.

### Phase A loop

```text
run thermo-nuclear self review
  → findings? fix them → ./scripts/quality-gate.sh → re-review
  → repeat until Verdict: APPROVED (honest) → hand off
```

### User feedback fix policy

After Phase A handoff, the user provides feedback in chat. For each feedback round:

```text
implement every suggestion (all severities, including low priority)
  → ./scripts/quality-gate.sh
  → re-run Phase A in handoff if code changed
  → return to user or proceed to merge when user says to merge
```

Implement **every** suggestion — Critical through 🟢, “maintenance smell”, “deferred”, etc. **Do not defer** because the reviewer labeled something low priority.

**Only exception:** **explicit disagreement** documented in the next Phase A handoff (item + rationale).

**Merge only when the user directs you to merge** and all non-disagreed feedback is resolved.

---

## Merge

Run **after user feedback is addressed** and the user asks to merge.

```bash
git checkout main
git merge feat/<branch> --no-ff -m "feat: ..."
./scripts/quality-gate.sh
git branch -d feat/<branch>
```

See `docs/implementation/reviews/README.md` for the full workflow.
