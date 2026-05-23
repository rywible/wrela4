# {{TITLE}} Implementation Plan

> **For agentic workers:** Execute task-by-task in an isolated worktree. Run
> `./scripts/quality-gate.sh` after each task. Complete the review gate before merge.

**Goal:** {{ONE_SENTENCE_GOAL}}

**Architecture:** {{ARCHITECTURE_SUMMARY}}

**Tech Stack:** {{TECH_STACK}}

---

## Locked Decisions

<!-- Rules agents must not violate. Mirror additions in docs/design/locked-decisions.md -->

- {{DECISION_1}}
- {{DECISION_2}}

## Planned File Structure

```text
{{FILE_TREE}}
```

## Public API Shape

```rust
// Core API after this plan
```

## Parallel Work Map

- Task 1 must run first.
- {{DEPENDENCY_NOTES}}

## Subagent Git Discipline

Parallel subagents must not commit directly to the same branch. Each subagent
works in its own branch or worktree; the integration owner merges in dependency
order and re-runs verification after conflicts.

---

### Task 1: {{TASK_TITLE}}

**Files:**
- Create/Modify: `{{PATHS}}`

**Description:** {{DESCRIPTION}}

- [ ] **Step 1: Write failing test**
- [ ] **Step 2: Run test — expect failure**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run test — expect pass**
- [ ] **Step 5: Commit**

```bash
{{VERIFICATION_COMMANDS}}
```

**Acceptance criteria:**
- {{AC_1}}

---

### Task N: Final Quality Gate

**Description:** Run the full local quality gate.

```bash
./scripts/quality-gate.sh
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

**Acceptance criteria:**
- All tasks complete
- Quality gate passes
- Phase A + Phase B review APPROVED
- Interim review artifacts deleted; worktree removed after merge

## Self-Review Checklist

<!-- Copy items agents must verify before Phase B -->

- {{CHECKLIST_ITEM}}

## Locked Decisions (verification)

<!-- Repeat locked decisions as checklist for reviewers -->
