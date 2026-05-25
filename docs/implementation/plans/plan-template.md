# {{TITLE}} Implementation Plan

> **For agentic workers:** Execute task-by-task on a feature branch (`git checkout -b feat/<name>` from `main`). Run
> `./scripts/quality-gate.sh` after each task. Complete Phase A before handoff; merge after user feedback.

**Goal:** {{ONE_SENTENCE_GOAL}}

**Architecture:** {{ARCHITECTURE_SUMMARY}}

**Tech Stack:** {{TECH_STACK}}

---

## Locked Decisions

<!-- Rules agents must not violate. Mirror additions in AGENTS.md and docs/design/ ADRs -->

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
works in its own branch; the integration owner merges in dependency
order and re-runs verification after conflicts.

## Subagent Verification Discipline

Subagents may run focused `cargo test`, `cargo check`, or Wrela CLI commands for
their task, but every command must have a timeout in the execution tool and must
stay scoped to the task. Subagents must not run `./scripts/quality-gate.sh`,
strict clean gates, broad stress commands, or unbounded watch/server processes.
The **orchestrator** runs full verification and the quality gate sequentially
after each subagent returns and before marking a task complete.

## Review and feedback fix policy

Phase A and user feedback share one rule: implement **every** finding and **every**
suggestion at **every** priority and severity (including low priority, maintenance
smells, nice-to-have, deferred, non-blocking). **Do not defer** because a reviewer
labeled something optional.

**Only exception:** orchestrator **explicit disagreement** documented in the Phase A
verdict (**Explicit disagreements**: item + rationale).

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
- Phase A self review honestly **APPROVED** (no unresolved findings) before user handoff

## Self-Review Checklist

<!-- Copy items agents must verify before Phase A handoff -->

- {{CHECKLIST_ITEM}}

## Locked Decisions (verification)

<!-- Repeat locked decisions as checklist for reviewers -->
