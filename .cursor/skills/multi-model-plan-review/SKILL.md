---
name: multi-model-plan-review
description: Run thermo-nuclear self-review before returning from plan implementation; user provides feedback; then merge and cleanup. Use when implementing plans from docs/implementation/plans/.
---

# Plan Review (Phase A + user feedback)

Gate before handing a plan implementation back to the user:

1. **Phase A — Self review:** thermo-nuclear code quality review (loop until APPROVED on disk)
2. **User feedback:** the user reviews the work and provides feedback in chat
3. **Post-feedback:** address feedback → quality gate → re-run Phase A if code changed → **Phase C** cleanup and merge

**The orchestrator does not run Phase B** (`plan-review.sh`, Claude Code CLI, or Codex CLI). Those scripts remain available for optional manual use but are not part of the agent completion gate.

---

## Branch setup (not worktree)

Implement on a **feature branch** in the main repo checkout:

```bash
git checkout main
git checkout -b feat/<name>
```

Stay in the same workspace. Do **not** use `./scripts/plan-worktree-new.sh` unless the user explicitly requests a worktree.

---

## Completion gate (orchestrator)

**Stop and return to the user when:**

- All plan tasks implemented on the feature branch
- `./scripts/quality-gate.sh` passes
- Phase A verdict on disk: `## Verdict: APPROVED`
- `./scripts/plan-review-check-phase-a.sh . docs/implementation/plans/<plan>.md` passes

**Do not merge** until the user has reviewed and you have worked through their feedback.

Review scripts take the **repo root** as their path argument — use `.` when already on the feature branch.

---

## Phase A — Thermo-nuclear self review

See skill file at `~/.cursor/plugins/cache/cursor-public/cursor-team-kit/*/skills/thermo-nuclear-code-quality-review/SKILL.md`.

### Steps

1. Review branch diff (`BASE_SHA..HEAD`)
2. Save verification log (optional but recommended):

   ```bash
   ./scripts/plan-review-save-verification.sh docs/implementation/plans/<plan>.md .
   ```

3. **Write** `docs/implementation/reviews/<slug>-review-self-thermonuclear.md`
4. Verify:

   ```bash
   ./scripts/plan-review-check-phase-a.sh . docs/implementation/plans/<plan>.md
   ```

5. **Return to the user** for feedback

### Phase A loop (before first handoff)

```text
fix required items
  → ./scripts/quality-gate.sh
  → re-run thermo-nuclear self review
  → write/update *-review-self-thermonuclear.md
  → repeat until ## Verdict: APPROVED
```

### Phase A fix policy

When Phase A lists findings under **Required fixes**, **Missed simplification**, **Spaghetti**, **File-size**, or any other section, implement **every item at every priority and severity** before writing `## Verdict: APPROVED` — unless you **explicitly disagree** and document why in the verdict under **Explicit disagreements**.

**Forbidden:** silently skipping P2+ or “optional” items; APPROVED verdicts with “Deferred” / “non-blocking” lists unless each skipped item has a documented disagreement.

### User feedback fix policy

After Phase A handoff, the user provides feedback in chat. For each feedback round:

```text
implement every suggestion (all severities, including low priority)
  → ./scripts/quality-gate.sh
  → re-run Phase A (update verdict if code changed)
  → return to user or proceed to merge when user says to merge
```

Implement **every** suggestion from the feedback — Critical through 🟢, “maintenance smell”, “low-hanging”, “deferred”, etc. **Do not defer** because the reviewer labeled something low priority.

**Only exception:** **explicit disagreement** documented in the next Phase A verdict (item + rationale). Undocumented skips are forbidden.

**Merge only when the user directs you to merge** and all non-disagreed feedback is resolved.

### Plan amendment (conditional)

**Required when:** code-judo items changed plan assumptions.

Write `<slug>-plan-amendment.md` using `docs/implementation/reviews/plan-amendment-template.md`.

---

## User feedback loop (replaces automated Phase B)

See **User feedback fix policy** above — same fix-everything rule as Phase A.

---

## Phase C — Cleanup and merge

Run **after user feedback is addressed** and the user asks to merge.

```bash
./scripts/plan-review-cleanup.sh . docs/implementation/plans/<plan>.md
git checkout main
git merge feat/<branch> --no-ff -m "feat: ..."
./scripts/quality-gate.sh
git branch -d feat/<branch>
```

### Interim artifacts (do not merge to main)

Remove before merge (via cleanup script):

- `<slug>-review-self-thermonuclear.md`
- `<slug>-review-packet.md` (if present)
- `<slug>-verification.log`
- Any optional CLI review outputs (`*-review-claude.md`, `*-review-codex.md`)

**Keep on main:** templates and `docs/implementation/reviews/README.md`.

---

## Optional: manual external review (not orchestrator gate)

`./scripts/plan-review.sh docs/implementation/plans/<plan>.md .` can still be run manually for Claude + Codex reviews. It is **not** required for the orchestrator to return or merge.

See `docs/implementation/reviews/README.md` for script configuration.
