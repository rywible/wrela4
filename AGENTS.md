# Agent guide

This file orients coding agents working in the Wrela repository.

## Start here

1. [`docs/design-principles.md`](docs/design-principles.md) — language and toolchain principles
2. [`docs/implementation/reviews/README.md`](docs/implementation/reviews/README.md) — plan review workflow
3. [`docs/implementation/plans/`](docs/implementation/plans/) — active and completed implementation plans

## What this repo is

Wrela is an AArch64-only systems language. This repository contains the **Rust
command center**: a zero-dependency compiler nucleus and CLI in a single Cargo
package named `wrela`.

**Current implementation status:** lexer, discovery, CST parser, and **`wrela check`**
(semantic checking for the parser-supported subset) are complete. `wrela build`
and `wrela test` are not started.

## Source layout

```text
src/
  main.rs          CLI entrypoint → command::run
  lib.rs           public module exports
  command.rs       handwritten CLI; only module that prints to stdout/stderr
  diagnostic.rs    Severity, Span, Diagnostic, rendering
  source.rs        SourceFile, Span, SourceMap, FileId
  discover.rs      root-driven import graph expansion + parallel lex batches
  check/           read-only semantic checker (`wrela check` pipeline)
  lexer/           lossless tokenizer (tokens + trivia, byte spans)
  syntax/          CST parser and import-summary scanner
tests/lexer.rs     integration tests via public crate API only
tests/check.rs     `wrela check` CLI and checker integration tests
fixtures/lexer/    .wrela fixtures for lexer tests and manual CLI runs
fixtures/check/    .wrela fixtures for check diagnostics and smoke tests
docs/
  design-principles.md
  design/          ADRs and design proposals
  implementation/  plans and review workflow
scripts/           quality gate
```

Phases: discover → lex → parse → check (`src/check/`). Only `command.rs` prints.

## Non-negotiables

- **Zero external crate dependencies** unless a new ADR in `docs/design/` approves one.
- **No `unsafe`, `todo!()`, or `unimplemented!()`** in production `src/`.
- **Diagnostics as data** — compiler phases return `Diagnostic`; only `command.rs` prints.
- **Immutable phase artifacts** — phases take explicit inputs and return new data.
- **Root-driven reachability** — no manifests; discovery starts from a root `.wrela` file.
- **Rust 2024**, `rust-version = "1.85"`, edition 2024 in `Cargo.toml`.

See [`docs/design-principles.md`](docs/design-principles.md) and ADRs under [`docs/design/`](docs/design/) for full rules.

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
3. **Subagents may run focused commands with timeouts** — subagents may run the narrow `cargo test`, `cargo check`, or Wrela CLI command needed for their task, but every command must have an execution timeout and must stay scoped to the task. Subagents must not run `./scripts/quality-gate.sh`, strict clean gates, broad stress commands, or unbounded watch/server processes. The orchestrator runs full build/test verification sequentially after each subagent returns.
4. Run `./scripts/quality-gate.sh` after each task and before review.
5. Complete **Phase A** (adversarial thermo-nuclear self review) before returning for **user feedback** — fix **every** finding; hand off only on honest **`Verdict: APPROVED`**. **`Verdict: NOT APPROVED`** means keep working.

6. After user feedback: fix **every suggestion** (including low priority, maintenance smells, and items labeled deferred/non-blocking) unless explicitly disagreed and documented → quality gate → re-run Phase A in handoff if code changed.

7. When the user directs merge: merge feature branch into `main` → delete the feature branch.

**Do not hand off** while Phase A has unresolved findings. **Do not merge** until all non-disagreed feedback is resolved. **Do not defer** review items because of priority labels. **Do not rubber-stamp APPROVED.**

See [`.cursor/skills/multi-model-plan-review/SKILL.md`](.cursor/skills/multi-model-plan-review/SKILL.md)
   and [`docs/implementation/reviews/README.md`](docs/implementation/reviews/README.md).

New plans: copy [`docs/implementation/plans/plan-template.md`](docs/implementation/plans/plan-template.md).

New architecture decisions: copy [`docs/design/decision-template.md`](docs/design/decision-template.md).

## `wrela check`

Read-only semantic validation (summaries → resolve → types → bodies → ownership → effects → layout). Not a formatter.

**CLI:** `cargo run -- check [--json|--human] <root.wrela>` — JSON default (`wrela.check.v1`); `--human` for source diagnostics. Exit `0`/`1`/`2` (ok / errors / bad usage). Flags before path; unknown `--*` rejected.

**Use when:**

- Semantic AC or fixture behavior → `cargo run -- check …` or `cargo test --test check <filter>`
- Debug output → `--human`; machine output → default JSON
- Lex/parse only → `wrela lex` / `wrela parse` (not check)
- Repo handoff → `./scripts/quality-gate.sh` (orchestrator only; subagents: focused check commands with timeout)

**API:** `wrela::check::check_root(path)` → `CheckResult`; render via `diagnostic::render_diagnostics`. Codes: `DiagnosticCode` in `src/diagnostic.rs`. Fixtures: [`fixtures/check/`](fixtures/check/).

## Useful commands

```bash
cargo run -- check <root.wrela>                              # JSON
cargo run -- check --human fixtures/check/diagnostics/unknown-type.wrela
cargo test --test check
cargo run -- lex fixtures/lexer/imports/root.wrela
cargo run -- parse fixtures/parser/declarations-top.wrela
./scripts/quality-gate.sh
```
