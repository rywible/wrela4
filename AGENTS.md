# Agent guide

This file orients coding agents working in the Wrela repository.

## Start here

1. [`docs/design-principles.md`](docs/design-principles.md) — language and toolchain principles
2. [`docs/design/locked-decisions.md`](docs/design/locked-decisions.md) — rules you must not violate
3. [`docs/implementation/README.md`](docs/implementation/README.md) — plan status and workflow

## What this repo is

Wrela is an AArch64-only systems language. This repository contains the **Rust
command center**: a zero-dependency compiler nucleus and CLI in a single Cargo
package named `wrela`.

**Current implementation status:** lexer, import-summary parsing, root-driven
discovery, and basic CLI are complete (see implementation README). Parser and
later phases are not started.

## Source layout

```text
src/
  main.rs          CLI entrypoint → command::run
  lib.rs           public module exports
  command.rs       handwritten CLI; only module that prints to stdout/stderr
  diagnostic.rs    Severity, Span, Diagnostic, rendering
  source.rs        SourceFile, Span, SourceMap, FileId
  discover.rs      root-driven import graph expansion + parallel lex batches
  lexer/           lossless tokenizer (tokens + trivia, byte spans)
  syntax/          minimal parsers (imports.rs today; full parser later)
tests/lexer.rs     integration tests via public crate API only
fixtures/lexer/    .wrela fixtures for tests and manual CLI runs
docs/
  design-principles.md
  design/          language spec, ADRs, architecture, supported syntax subset
  implementation/  plans, review harness, roadmap
scripts/           quality gate, plan review
```

See [`docs/design/compiler-pipeline.md`](docs/design/compiler-pipeline.md) for
how phases connect.

## Non-negotiables

- **Zero external crate dependencies** unless a new ADR in `docs/design/` approves one.
- **No `unsafe`, `todo!()`, or `unimplemented!()`** in production `src/`.
- **Diagnostics as data** — compiler phases return `Diagnostic`; only `command.rs` prints.
- **Immutable phase artifacts** — phases take explicit inputs and return new data.
- **Root-driven reachability** — no manifests; discovery starts from a root `.wrela` file.
- **Rust 2024**, `rust-version = "1.85"`, edition 2024 in `Cargo.toml`.

Full list: [`docs/design/locked-decisions.md`](docs/design/locked-decisions.md).

Rust-specific conventions: [`.cursor/rules/wrela-rust.mdc`](.cursor/rules/wrela-rust.mdc).

## Quality gate

There is **no CI** for this repository. Agents must run the local quality gate
before claiming work is complete:

```bash
./scripts/quality-gate.sh
```

Strict mode (also requires clean git status):

```bash
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

## Implementing a plan

You are the **orchestrator** — accountable for full plan AC and production quality.
See [`.cursor/rules/plan-orchestration.mdc`](.cursor/rules/plan-orchestration.mdc).

1. Create a feature branch from `main` (same repo checkout — **not** a worktree):

   ```bash
   git checkout main
   git checkout -b feat/my-feature
   ```

2. Execute the plan on that branch task-by-task.
3. **Subagents edit code only** — they must not run `cargo`, `./scripts/quality-gate.sh`, or Wrela CLI commands. The orchestrator runs all build/test verification sequentially after each subagent returns.
4. Run `./scripts/quality-gate.sh` after each task and before review.
5. Complete **Phase A** (thermo-nuclear self review) before returning for **user feedback** — fix **every** Phase A finding at every priority/severity unless you explicitly disagree and document why in the verdict.

6. After user feedback: fix **every suggestion** (including low priority, maintenance smells, and items labeled deferred/non-blocking) unless explicitly disagreed and documented → quality gate → re-run Phase A if code changed.

7. When the user directs merge: Phase C cleanup → merge feature branch into `main` → delete the feature branch.

**Do not stop after implementation commits** without Phase A APPROVED on disk. **Do not merge** until all non-disagreed feedback is resolved. **Do not defer** review items because of priority labels. The orchestrator does **not** run automated Phase B (`plan-review.sh`).

   See [`.cursor/skills/multi-model-plan-review/SKILL.md`](.cursor/skills/multi-model-plan-review/SKILL.md)
   and [`docs/implementation/reviews/README.md`](docs/implementation/reviews/README.md).

New plans: copy [`docs/implementation/plans/plan-template.md`](docs/implementation/plans/plan-template.md).

New architecture decisions: copy [`docs/design/decision-template.md`](docs/design/decision-template.md).

## Useful commands

```bash
cargo run -- help
cargo run -- version
cargo run -- dump tokens fixtures/lexer/basic.wrela
cargo run -- lex fixtures/lexer/imports/root.wrela
./scripts/plan-review-smoke-test.sh    # verify review tooling (no full reviews)
```
