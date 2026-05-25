# Wrela Design Principles

This document is the entry point for Wrela documentation. It states the
principles that guide language design, compiler architecture, and day-to-day
implementation. Detailed specs, decisions, plans, and review templates live in
the subfolders below.

## Documentation layout

| Path | Contents |
|------|----------|
| [`design/`](design/) | Language spec, ADRs, architecture, locked decisions, supported syntax subset |
| [`implementation/`](implementation/) | Implementation plans, roadmap, plan-review harness |

Agent onboarding: [`../AGENTS.md`](../AGENTS.md).

## Language principles

Wrela is an AArch64-only systems language for building complete appliance
images. The compiler should make machine authority, memory ownership, data
layout, test execution, and hot data paths visible without turning all code into
assembly or compiler folklore.

- **Explicit ownership.** Callable behavior lives on classes, not as top-level
  functions. Modules are static namespaces with no initialization side effects.
- **Static contracts.** Interfaces are compile-time requirements, not runtime
  vtables. Generics and constraints are resolved at compile time.
- **Explicit authority.** Borrowing uses `read`, `mut`, and `own`. Privileged
  effects come from `unique class` authority, not handwritten permission slips.
- **Recoverable errors.** Failures are typed values, not exceptions.
- **Columnar data by default.** Tables, masks, and layouts are first-class;
  fixed vector types and intrinsics remain available for sharp kernels.
- **Exhaustive control flow.** Scalar branching uses exhaustive `match`; loops
  describe work shape explicitly.

See [`design/2026-05-22-wrela-language-and-test-design.md`](design/2026-05-22-wrela-language-and-test-design.md)
for the full language and test model.

## Compiler and toolchain principles

The Rust binary is the Wrela command center — CLI and compiler nucleus in one
project, not a wrapper around a separate service.

- **Zero dependencies by default.** External crates require a written decision
  doc that argues for demonstrated pain, compile cost, and exit plan.
- **Handwritten first.** CLI parsing, diagnostics, lexer, and parser start
  handwritten unless a later decision changes that.
- **Parallel, immutable phases.** Compile like a parallel graph: phases receive
  explicit inputs and return immutable artifacts plus diagnostics as data.
- **Root-driven reachability.** Source discovery starts from an explicit root
  image file and expands through imports. There are no manifests.
- **Lossless lexing.** The lexer preserves trivia for formatting, diagnostics,
  and later tooling. Lexing is file-local and mode-free.
- **Deterministic merge.** Diagnostics and phase outputs merge in stable,
  documented order.

See [`design/0001-rust-command-center-and-zero-dependency-nucleus.md`](design/0001-rust-command-center-and-zero-dependency-nucleus.md)
for the accepted toolchain decision.

Locked rules for agents: [`design/locked-decisions.md`](design/locked-decisions.md).

Pipeline overview: [`design/compiler-pipeline.md`](design/compiler-pipeline.md).

## Implementation workflow

Implementation plans live under [`implementation/plans/`](implementation/plans/).
See [`implementation/README.md`](implementation/README.md) for status and workflow.

Each plan is executed on a feature branch from `main`, passes thermo-nuclear self review
(Phase A), receives user feedback, then merges to `main` after interim review artifacts
are deleted.

There is **no CI** — `./scripts/quality-gate.sh` is the verifier.

See [`implementation/reviews/README.md`](implementation/reviews/README.md) for
the review gate and cleanup steps.
