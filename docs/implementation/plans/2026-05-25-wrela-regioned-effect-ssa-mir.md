# Wrela Regioned Effect SSA MIR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Wrela's Regioned Effect SSA MIR as the production bridge from `check` semantic artifacts to dev codegen, certified release optimization, and persistent optimization memory.

**Architecture:** This is the parent roadmap. The work is split into five child plans so each delivery is production-quality and junior-executable without pretending a small slice is the entire optimizer. MIR 01 builds, verifies, dumps, parses, and counts the first W-MIR artifact. MIR 02 adds correct dev-mode AArch64 codegen and first-class performance tooling. MIR 03 adds the first scalar aegraph-backed release rewrite path plus the Trident certificate/event contract from rewrite one. MIR 04 proves that contract on data-plane mask algebra without claiming runtime data-plane codegen. MIR 05 adds the durability layer: certified table-loop fusion, stable identity and hashing for optimizer memory, authority-algebra queries, certificate versioning, hardened measurement, a bounded persistent ledger, failure containment, a MIR reducer, and measured cost feedback.

**Tech Stack:** Rust 2024, standard library only, existing Wrela `check`, `syntax`, `diagnostic`, and `command` modules, handwritten IR arenas, handwritten textual formats, handwritten AArch64 assembly output.

---

## Source Design

- Design doc: [`docs/design/2026-05-25-wrela-regioned-effect-ssa-mir-design.md`](../../design/2026-05-25-wrela-regioned-effect-ssa-mir-design.md)
- Existing semantic entry point: `wrela::check::check_root`
- Existing local verifier: `./scripts/quality-gate.sh`

## Locked Decisions

- W-MIR consumes only successful `CheckResult` semantic artifacts. If `check` produces error-severity diagnostics, MIR build does not invent a partial compiler artifact.
- W-MIR is one IR used by both dev and release modes. Release optimization is an overlay pipeline, not a separate semantic IR.
- W-MIR uses block arguments, not phi nodes.
- W-MIR preserves Wrela facts directly: effects, ownership modes, capability paths, table/mask provenance, frame handles, row tokens, capacity tokens, trap provenance, and state edges.
- Dev mode never depends on equality machinery.
- Release mode may build bounded region-local aegraph overlays over pure subgraphs only.
- Release rewrites follow the **Trident doctrine** from MIR 03 onward:
  - **Provable:** every applied rewrite emits a versioned certificate with the rule name, pass name, region/op identity, stable before/after hashes, explicit legality facts, and verifier outcome. Missing or stale certificate facts are hard blockers: the rewrite does not fire.
  - **Measured:** when generated-code support exists, A/B evidence requires matching checksums, at least seven samples per side, `p90 / p10 <= 1.10`, and at least a 3% median movement. Otherwise the result is reported as `inconclusive`.
  - **Recorded:** every run stores certificates in a bounded in-memory event log, and MIR 05 also stores bounded persistent ledger entries per target profile so wins and failures can be triaged later.
- Rewrite logs, ledgers, and reducer attempts are bounded from day one. When a bound is reached, Wrela skips later rewrites or marks later measurements unrecorded; it never grows memory or disk usage without limit.
- Trident infrastructure failures are fail-closed: a bad certificate, bad hash, bad ledger entry, bad measurement, or bad reducer result disables the affected optimization path, emits a structured report, and preserves the artifact needed for triage.
- Performance tooling is required for compiler telemetry and generated-code A/B comparisons for codegen-supported passes. Perf scripts are not part of the default quality gate.
- No external crate dependencies are introduced.

## Child Plans

| Order | Plan | Responsibility | Depends On |
|-------|------|----------------|------------|
| 1 | [MIR 01: Build / Verify / Dump](2026-05-25-mir-01-build-verify-dump.md) | Core W-MIR data model, semantic-fact carriers, `mir.build`, verifier, deterministic text dump, round-trip parser, `wrela dump mir`, MIR counters | `check` product |
| 2 | [MIR 02: Dev Codegen And Perf Tooling](2026-05-25-mir-02-dev-codegen-perf.md) | LIR, direct W-MIR to AArch64 lowering, ABI parameter mapping, LIR verifier, fail-hard dev register allocation, assembly emission, `wrela perf compile`, `wrela perf code` smoke path | MIR 01 |
| 3 | [MIR 03: Certified Scalar Aegraph Release Slice](2026-05-25-mir-03-release-optimizer.md) | Region-local aegraph overlays, pass controls, stable hash V0, certificate/event V0, scalar identity rewrite through the overlay, scoped elaboration back to W-MIR, generated-code A/B evidence | MIR 02 |
| 4 | [MIR 04: Certified Data-Plane Mask Optimizer](2026-05-25-mir-04-data-plane-release-optimizer.md) | Checked table/mask subset, table/mask provenance, row-token verifier facts, authority query V0, certified mask boolean identity, branch/select cost seeds, explicit generated-code unsupported boundary | MIR 03 |
| 5 | [MIR 05: Certified Optimization Memory](2026-05-25-mir-05-certified-optimization-memory.md) | Certified table-loop fusion, parameterized data-plane generated-code harness, stable identity V1, authority algebra engine, certificate versioning, measurement hardening, bounded ledger, debug/why commands, reducer, cost feedback loop | MIR 04 |

## Planned File Structure

```text
src/
  lib.rs
  command.rs
  mir/
    mod.rs
    id.rs
    effect.rs
    ty.rs
    ir.rs
    build.rs
    verify.rs
    text.rs
    parse_text.rs
    report.rs
    perf.rs
    lir.rs                  # MIR 02
    lower.rs                # MIR 02
    aarch64.rs              # MIR 02
    regalloc.rs             # MIR 02
    emit.rs                 # MIR 02
    aegraph.rs              # MIR 03
    hash.rs                 # MIR 03/MIR 05
    cert.rs                 # MIR 03/MIR 05
    rewrite.rs              # MIR 03
    pass_control.rs         # MIR 03
    cost.rs                 # MIR 04
    dataplane.rs            # MIR 04
    authority.rs            # MIR 04/MIR 05
    ledger.rs               # MIR 05
    hotness.rs              # MIR 05
    reduce.rs               # MIR 05
tests/
  mir.rs
  mir_codegen.rs            # MIR 02
  mir_perf.rs               # MIR 02
  mir_release.rs            # MIR 03
fixtures/
  mir/
    basic.wrela
    data_flow.wrela
    imports/root.wrela
    imports/lib.wrela
  perf/
    scalar_const.wrela      # MIR 02
    scalar_identity.wrela   # MIR 03
    filter_sum.wrela        # MIR 05 generated-code A/B fixture
scripts/
  perf-smoke.sh             # MIR 02
  perf-compare.sh           # MIR 02
```

## Public API Shape

```rust
pub mod mir;

pub fn mir::build_mir(check: &wrela::check::CheckResult) -> mir::MirBuildResult;

pub struct mir::MirBuildResult {
    pub fn module(&self) -> Option<&mir::MirModule>;
    pub fn diagnostics(&self) -> &[Diagnostic];
    pub fn report(&self) -> &mir::MirReport;
    pub fn ok(&self) -> bool;
}

pub fn mir::verify_module(module: &mir::MirModule) -> mir::VerifyResult;
pub fn mir::text::render_module(module: &mir::MirModule) -> String;
pub fn mir::parse_text::parse_module(text: &str) -> Result<mir::MirModule, mir::TextParseError>;
```

MIR 02 extends this API with `lower_to_lir`, AArch64 emission, verification, and perf command reports. MIR 03 extends it with release pass controls, stable MIR hashes, versioned rewrite certificates, bounded rewrite event logs, and scalar optimization reports. MIR 04 extends it with data-plane authority queries, certified mask reports, target cost profiles, and a clear unsupported generated-code result for data-plane fixtures. MIR 05 extends it with data-plane generated-code execution, persistent optimization ledger entries, stable identity V1, certificate migration/invalidation, static hotness seeds, debug/why commands, reducer artifacts, and cost-feedback decisions.

## Parallel Work Map

- MIR 01 Task 1 must run first because it creates module boundaries and ID/types used by every other MIR task.
- MIR 01 shared type/model tasks run serially until `src/mir/mod.rs` and `src/mir/ir.rs` are stable. After that, subagents can work in parallel on verifier tests, text parsing, CLI wiring, and docs if each subagent stays in its assigned files.
- MIR 01 integration tasks must run after the builder, verifier, and text format tasks land.
- MIR 02 starts only after MIR 01 is merged and the quality gate passes.
- MIR 03 starts only after MIR 02 is merged and the quality gate passes. MIR 03 must introduce stable hash V0 and certificate/event V0 before its first scalar rewrite.
- MIR 04 starts only after MIR 03 is merged and the quality gate passes. MIR 04 must use MIR 03 certificates for mask algebra rather than inventing a second reporting path.
- MIR 05 starts only after MIR 04 is merged and the quality gate passes. MIR 05 owns table-loop fusion, generated-code runtime evidence for data-plane passes, stable identity V1, persistent optimization memory, measurement hardening, failure containment, and the first reducer.

## Subagent Git Discipline

The orchestrator creates the feature branch from `main` in the same checkout:

```bash
git checkout main
git checkout -b feat/regioned-effect-ssa-mir
```

Parallel subagents must not create branches, worktrees, or commits in this repository checkout. They may work from read-only context and return a patch/diff or written handoff for their assigned files. The orchestrator applies those patches serially on the single feature branch, resolves conflicts, and re-runs focused verification after each integration.

All commits created by automation keep the repository convention of ending the commit subject with `-Codex Automated`. Human-authored commits may omit that suffix unless the branch owner asks them to preserve automation-style messages.

## Subagent Verification Discipline

Subagents may run focused `cargo test`, `cargo check`, or Wrela CLI commands for their task, but every command must have a timeout in the execution tool and must stay scoped to the task. Subagents must not run `./scripts/quality-gate.sh`, strict clean gates, broad stress commands, perf loops, or unbounded watch/server processes. The orchestrator runs full verification and the quality gate sequentially after each subagent returns.

## Review and Feedback Fix Policy

Phase A and user feedback share one rule: implement every finding and every suggestion at every priority and severity, including low priority, maintenance smells, nice-to-have, deferred, and non-blocking. Do not defer because a reviewer labeled something optional.

Only exception: orchestrator explicit disagreement documented in the Phase A verdict with item and rationale.

## Final Acceptance Criteria

- MIR 01, MIR 02, MIR 03, MIR 04, and MIR 05 child plans complete in order.
- `wrela dump mir <root.wrela>` emits deterministic round-trip W-MIR text.
- Dev codegen emits correct AArch64 for the MIR 02 supported subset: zero-arg and ABI-mapped read-parameter leaf functions over scalar integer values.
- `wrela perf compile` reports compiler telemetry in human and JSON formats.
- `wrela perf code` runs generated-code smoke benchmarks with checksums.
- MIR 03 release optimization exposes pass-level controls and A/B generated-code benchmark reports for scalar rewrites.
- MIR 03 and later release rewrites emit versioned certificates and bounded rewrite events.
- MIR 04 release optimization exposes certified data-plane mask pass controls, IR rewrite reports, verifier evidence, authority query evidence, and an explicit generated-code unsupported result for data-plane fixtures.
- MIR 05 release optimization exposes certified table-loop fusion, parameterized data-plane generated-code A/B evidence, bounded persistent ledger storage, debug/why output, reducer artifacts for rewrite failures, and ledger-informed cost feedback.
- `./scripts/quality-gate.sh` passes after every child plan.
- `QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh` passes before final merge.
- Phase A review is reported honestly in the handoff message before user handoff, per `docs/implementation/reviews/README.md`.

## Self-Review Checklist

- [ ] Child plan links resolve.
- [ ] Each child plan has atomic tasks with files, description, steps, code examples, and acceptance criteria.
- [ ] MIR 01 contains no backend, regalloc, emitted-code, or release optimizer implementation.
- [ ] MIR 02 contains generated-code perf tooling but no release optimizer pass controls.
- [ ] MIR 03 contains release pass controls, stable hash V0, certificate/event V0, and scalar A/B generated-code performance evidence.
- [ ] MIR 04 contains certified data-plane mask rewrites, authority query evidence, verifier evidence, IR rewrite reports, and an explicit unsupported generated-code boundary.
- [ ] MIR 05 contains certified table-loop fusion, generated-code runtime evidence for data-plane passes, stable identity V1, bounded persistent ledger storage, measurement hardening, failure containment, debug/why commands, cost feedback, and rewrite-failure reduction.
- [ ] No release rewrite family lands as an unrecorded transformation.
- [ ] No task introduces an external crate dependency.
- [ ] No task requires subagents to run the full quality gate.
