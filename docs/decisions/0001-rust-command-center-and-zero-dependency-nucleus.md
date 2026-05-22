# 0001: Rust Command Center And Zero-Dependency Nucleus

Date: 2026-05-22

## Status

Accepted.

## Context

Wrela needs a first implementation that is pleasant to work on every day. The
Rust binary should become the Wrela command center: the tool users invoke for
checking, building, testing, inspecting, and eventually producing appliance
images.

The compiler should live inside that command center from the beginning. The CLI
is not a separate wrapper around a distant compiler service. It is the primary
development surface for the language.

Rust is the implementation language because Wrela's compiler will be full of
typed IR, authority graphs, ownership checks, effect inference, and bounded
memory/layout invariants. Rust helps represent those shapes directly. The main
counterweight is Rust compile time, so the project should start with a small
toolchain surface and avoid dependency growth by default.

## Decision

The initial Wrela Rust implementation starts with no external crate
dependencies.

The Rust binary is the Wrela command center. Its early command surface should
include commands such as:

- `wrela check`
- `wrela build`
- `wrela test`
- `wrela dump`
- `wrela help`
- `wrela version`

The exact file and module layout is intentionally deferred. This decision only
sets the project philosophy:

- The command center and compiler live in the same Rust project at first.
- The default answer to a new crate dependency is no.
- Pulling in any external crate requires a written decision doc.
- The first CLI parser is handwritten.
- The first diagnostics renderer is handwritten.
- The first test fixture runner is handwritten.
- The first lexer and parser are handwritten unless a later decision changes
  that.
- The first backend should avoid heavyweight compiler framework dependencies.
- Dependencies are pulled in by demonstrated pain, not by niceness.

This includes popular and high-quality crates. `clap`, `serde`, `anyhow`,
`thiserror`, `miette`, parser generators, snapshot testing crates, async
runtimes, and backend frameworks are all deferred until a decision doc argues
for one of them specifically.

## Dependency Decision Rule

A dependency decision doc should answer:

- What problem is painful enough to justify the dependency?
- Why is handwritten code no longer the right tradeoff?
- What compile-time cost does the dependency add?
- What transitive dependencies does it bring?
- Does it introduce proc macros, build scripts, async runtimes, or generated
  code?
- Does it affect compiler determinism, bootstrapping, portability, or audit
  surface?
- What is the exit plan if the dependency becomes a burden?

Small dependencies are not automatically acceptable. Nice APIs are not enough.
The dependency must buy back enough complexity, correctness, or development
time to justify its permanent weight.

## CLI Crates

The first CLI should not use a CLI framework.

The early command surface is small enough to parse with `std::env::args`.
Handwritten parsing keeps compile times low, keeps behavior obvious, and avoids
giving CLI ergonomics more architectural weight than the compiler nucleus.

A future CLI crate can be reconsidered when one of these becomes true:

- command parsing becomes noisy enough to obscure command behavior
- help output becomes expensive to keep consistent
- shell completion becomes important
- option validation starts duplicating across commands
- the handwritten parser creates user-facing bugs

Until then, CLI niceness is not a sufficient reason to add a dependency.

## Compiler Crates

The compiler nucleus should prefer simple Rust:

- explicit enums and structs
- typed IDs
- arenas backed by `Vec`
- handwritten passes
- fixture-driven tests
- direct data structures over generic abstraction towers

The project should avoid early dependencies on LLVM bindings, Cranelift,
parser generators, diagnostics frameworks, or large utility ecosystems. Wrela's
first backend is AArch64-only, so early code generation can be deliberately
narrow and direct.

## Consequences

Benefits:

- Fast clean builds for a Rust compiler project.
- Fast edit/check cycles.
- Small audit surface.
- Fewer transitive maintenance surprises.
- Compiler architecture stays visible while the language is still forming.
- The tool's implementation philosophy matches Wrela's language philosophy:
  explicit authority, explicit dependency edges, and no ambient magic.

Costs:

- More handwritten infrastructure early.
- CLI help and diagnostics will be simpler at first.
- Some polish must wait.
- The project must resist repeatedly rebuilding mature crates poorly.

The intended tradeoff is not permanent minimalism. It is delayed commitment.
When a dependency becomes obviously worth it, Wrela should add it deliberately,
with the decision recorded.
