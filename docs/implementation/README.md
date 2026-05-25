# Implementation

Plans, review workflow, and delivery status for the Wrela command center.

## Status

| Plan | Status | Notes |
|------|--------|-------|
| [Lexer and initial Rust setup](plans/2026-05-22-lexer-and-initial-rust-setup.md) | **Complete** | Merged to `main` |
| [CST parser](plans/2026-05-24-cst-parser.md) | **Complete** | Lossless CST parser + `wrela parse` |
| `wrela check` / `wrela build` / `wrela test` | Not started | Listed in ADR 0001; CLI stubs only |

Update this table when starting or finishing a plan.

## Workflow

1. **Spec / ADR** — design docs in [`../design/`](../design/); new decisions use [`decision-template.md`](../design/decision-template.md).
2. **Plan** — copy [`plans/plan-template.md`](plans/plan-template.md), fill locked decisions and tasks.
3. **Worktree** — `./scripts/plan-worktree-new.sh feat/<name>`.
4. **Implement** — task-by-task in the worktree; run `./scripts/quality-gate.sh` frequently.
5. **Review** — Phase A → Phase B → loop until pass ([`reviews/README.md`](reviews/README.md)).
6. **Land** — cleanup interim artifacts, merge to `main`, remove worktree.

## Verification

There is **no CI**. Before merge, run:

```bash
./scripts/quality-gate.sh
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh   # before final merge
```

## Review harness

| Script | Purpose |
|--------|---------|
| `scripts/plan-worktree-new.sh` | Create isolated worktree |
| `scripts/quality-gate.sh` | fmt, check, clippy, test, marker scan |
| `scripts/plan-review-check-phase-a.sh` | Verify Phase A APPROVED on disk |
| `scripts/plan-review-save-verification.sh` | Save verification log for review packet |
| `scripts/plan-review.sh` | Phase B: Claude + Codex parallel review |
| `scripts/plan-review-cleanup.sh` | Delete interim review artifacts before merge |
| `scripts/plan-review-smoke-test.sh` | Smoke-test review tooling |

Cursor skill: [`.cursor/skills/multi-model-plan-review/SKILL.md`](../../.cursor/skills/multi-model-plan-review/SKILL.md).

## Plans directory

- [`plans/plan-template.md`](plans/plan-template.md) — template for new plans
- [`plans/2026-05-22-lexer-and-initial-rust-setup.md`](plans/2026-05-22-lexer-and-initial-rust-setup.md) — reference completed plan
- [`plans/2026-05-24-cst-parser.md`](plans/2026-05-24-cst-parser.md) — CST parser plan

## Reviews directory

Templates only on `main`. Per-plan review outputs are written in the worktree during execution and deleted before merge.
