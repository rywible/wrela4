---
name: multi-model-plan-review
description: Run thermo-nuclear self-review then independent plan reviews via Claude Code (Opus max) and Codex (GPT 5.5 xhigh) before marking implementation plans complete. Use when implementing superpowers plans, when the user asks for multi-model review, or before returning from plan execution.
---

# Multi-Model Plan Review

Two-phase gate before claiming any implementation plan is complete:

1. **Phase A — Self review:** thermo-nuclear code quality review (loop until PASS)
2. **Phase B — External review:** Claude Code + Codex in parallel (loop until both APPROVED)

---

## Strict sequencing (NON-NEGOTIABLE)

**Phase A and Phase B are separate turns. Never combine them.**

```text
Turn N   → Phase A only (read skill, audit diff, write verdict file)
Turn N+1 → Verify Phase A file on disk (see checklist below)
Turn N+2 → Phase B only (plan-review.sh — Claude + Codex)
```

### What Phase A means

- **Who:** the orchestrator (you), using `thermo-nuclear-code-quality-review`
- **Output:** a file on disk: `<worktree>/docs/superpowers/reviews/<slug>-review-self-thermonuclear.md`
- **Done when:** that file contains a line exactly matching `## Verdict: APPROVED`

### What Phase B means

- **Who:** Claude Code CLI + Codex CLI via `plan-review.sh`
- **When:** only after Phase A verification passes
- **Parallel inside Phase B:** Claude and Codex run together in `plan-review.sh` — **never** in the same turn as Phase A

### Forbidden

| Do NOT | Why |
|--------|-----|
| Run `plan-review.sh` in the same turn you write the Phase A verdict | Externals start before Phase A is on disk |
| Batch Phase A + Phase B in one assistant message | Same |
| Assume Phase A is done from chat draft | Verdict must be **written to the file** |
| Skip `plan-review-check-phase-a.sh` before Phase B | Confirms APPROVED on disk |
| Use `PLAN_REVIEW_SKIP_SELF_GATE=1` except emergencies | Bypasses the gate |

### Required before starting Phase B

```bash
./scripts/plan-review-check-phase-a.sh "$WORKTREE" docs/superpowers/plans/<plan>.md
```

---

## Gate (hard stop)

### Phase A — Thermo-nuclear self review

1. Read `thermo-nuclear-code-quality-review` skill
2. Review worktree diff (`BASE_SHA..HEAD`)
3. **Write** `*-review-self-thermonuclear.md`
4. **Stop turn.** Do not start Phase B in the same message.
5. Loop until **APPROVED**

### Phase B — Claude + Codex

**Start in a subsequent turn** after Phase A checklist passes.

1. Save verification log (`plan-review-save-verification.sh`)
2. Run `plan-review.sh` (Claude + Codex in parallel)
3. Both `*-review-claude.md` and `*-review-codex.md` → **APPROVED**

---

## Autonomous review loops (NON-NEGOTIABLE)

**Never return to the user or claim the plan is complete while a phase gate is failing.**

After **any** fix driven by Phase A or Phase B feedback, the orchestrator must **autonomously loop** until the gate passes. Do not ask the user to re-run reviews or commit fixes — do it yourself.

### Phase A loop

```text
fix required items
  → run quality gate (fmt, check, clippy, test, clean status)
  → re-run thermo-nuclear self review
  → write/update *-review-self-thermonuclear.md
  → repeat until ## Verdict: APPROVED
```

Stop only when Phase A verdict file contains `## Verdict: APPROVED`.

### Phase B loop

```text
Phase A APPROVED on disk
  → save verification log
  → plan-review-check-phase-a.sh
  → plan-review.sh (Claude + Codex in parallel)
  → if either REJECTED:
        fix all Required fixes (Important+ and above)
        → commit fixes
        → Phase A loop (always — any post-Phase-B fix re-opens Phase A)
        → Phase B loop again
  → repeat until Claude AND Codex both APPROVED
```

### Loop rules

| Trigger | Required response |
|---------|-------------------|
| Phase A REJECTED | Fix → quality gate → re-write Phase A verdict → loop Phase A |
| Phase B Claude REJECTED | Fix → commit → Phase A loop → Phase B loop |
| Phase B Codex REJECTED | Fix → commit → Phase A loop → Phase B loop |
| Either Phase B reviewer REJECTED | Fix **both** reviewers' required items before re-run |
| Fix applied | Commit before Phase B re-run (Codex audits `BASE_SHA..HEAD`) |
| Phase B pass | Both `*-review-claude.md` and `*-review-codex.md` contain `## Verdict: APPROVED` |

**Forbidden after a review-driven fix:** stopping to ask the user whether to re-run; leaving fixes uncommitted; re-running only one Phase B reviewer when the other previously passed (always re-run **both** after fixes).

---

## Phase A: Thermo-nuclear self review

See skill file at `~/.cursor/plugins/cache/cursor-public/cursor-team-kit/*/skills/thermo-nuclear-code-quality-review/SKILL.md`.

Write verdict to `<worktree>/docs/superpowers/reviews/<slug>-review-self-thermonuclear.md` with `## Verdict: APPROVED | REJECTED`.

### Step A5: Plan amendment (conditional)

**Required when:** code-judo items under `## Missed simplification / code-judo opportunities` were implemented **and** changed plan assumptions.

Write `<slug>-plan-amendment.md` using `docs/superpowers/reviews/plan-amendment-template.md`. Phase B packet auto-includes it for Claude and Codex.

---

## Phase B: External reviews

```bash
./scripts/plan-review-save-verification.sh docs/superpowers/plans/YYYY-MM-DD-feature.md .worktrees/feat-branch
./scripts/plan-review-check-phase-a.sh .worktrees/feat-branch docs/superpowers/plans/YYYY-MM-DD-feature.md
./scripts/plan-review.sh docs/superpowers/plans/YYYY-MM-DD-feature.md .worktrees/feat-branch
```

## Default models

| Channel | Default | Override |
|---------|---------|----------|
| Self (Phase A) | Orchestrator + thermo-nuclear skill | — |
| Claude Code | `opus`, effort `max` | `CLAUDE_REVIEW_MODEL`, `CLAUDE_REVIEW_EFFORT` |
| Codex | `gpt-5.5`, reasoning `xhigh` | `CODEX_REVIEW_MODEL`, `CODEX_REVIEW_REASONING` |

## Orchestrator checklist

**Phase A:** quality gate → thermo-nuclear review → verdict on disk → APPROVED → amendment if needed → **stop turn** (unless looping after a fix — then continue Phase A loop without returning)

**Phase B:** verification log → check-phase-a → plan-review.sh → if either REJECTED → fix → commit → Phase A loop → Phase B loop → **only return when both APPROVED**

**Phase C (cleanup):** delete interim review artifacts from worktree → merge branch into main → remove worktree

See also: `docs/superpowers/reviews/README.md`

---

## Phase C — Cleanup and merge

After Phase B passes (Claude **and** Codex APPROVED), clean up before landing on `main`.

### 1. Delete interim review artifacts (worktree only)

These are ephemeral audit outputs. **Do not commit or merge them.**

Remove from `<worktree>/docs/superpowers/reviews/`:

- `<slug>-review-self-thermonuclear.md`
- `<slug>-review-claude.md`
- `<slug>-review-codex.md`
- `<slug>-review-packet.md`
- `<slug>-plan-amendment.md`
- `<slug>-verification.log`

**Keep on `main`:** templates (`review-*-template.md`, `plan-amendment-template.md`) and `docs/superpowers/reviews/README.md`.

```bash
./scripts/plan-review-cleanup.sh .worktrees/feat-branch docs/superpowers/plans/PLAN.md
```

### 2. Merge into main

```bash
git checkout main
git merge feat/branch --no-ff -m "feat: ..."
```

Resolve conflicts if any; re-run quality gate on `main`.

### 3. Remove worktree

After merge succeeds:

```bash
git worktree remove .worktrees/feat-branch
git branch -d feat/branch   # safe once merged
```

**Forbidden:** leaving stale worktrees after merge; committing interim review artifacts to `main`.
