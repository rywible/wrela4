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
- The compile pipeline is shaped for maximum parallelism and minimum necessary
  passes.
- The lexer is lossless and preserves trivia for formatting, diagnostics, and
  later tooling.
- Source reachability starts from an explicit root image file and expands
  through source imports. There are no manifests.

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

## Compile Pipeline Philosophy

Wrela should compile like a parallel graph, not like a ceremonial sequence of
compiler passes.

The pipeline should be as parallel as possible in as few passes as possible.
"Few passes" does not mean one giant pass. It means every phase must earn its
place by doing at least one of these jobs:

- exposing parallel work
- making an invariant checkable
- collapsing a graph boundary deterministically
- preparing code or data for execution, diagnostics, or image layout

The compiler should avoid global mutable state. Each phase should receive
explicit inputs and produce immutable artifacts plus diagnostics as data. Later
phases consume those artifacts rather than mutating them in place.

The preferred phase shape is:

- parallel local work
- immutable summary output
- deterministic merge
- more parallel local work

Examples:

- files lex into independent `LexedFile` artifacts
- parsed modules produce independent export summaries
- export summaries merge into a deterministic module/name graph
- type and effect checks run over independent modules, items, or dependency
  strongly connected components
- monomorphized methods, kernels, and root phases lower independently
- code generation emits independent object fragments
- final image layout and linking perform the main unavoidable collapse

The first implementation may execute serially. The important constraint is that
the APIs remain parallel-shaped. A serial loop over files should be replaceable
by a worker pool without changing the meaning of the compiler.

Merges must be deterministic. Diagnostics, symbol tables, object fragments, and
image sections should be ordered by stable source or graph IDs, not by thread
completion order.

## Root-Driven Source Discovery

Wrela source reachability starts from an explicit root image file. There are no
manifests. No separate file should exist to list source files, inject dependency
edges, or smuggle build behavior into the compiler.

The first source discovery loop should be:

1. Load the root image file.
2. Lex the root file.
3. Parse only the import/root summary needed for discovery.
4. Resolve directly imported source files.
5. Load newly discovered files.
6. Lex newly discovered files in parallel.
7. Parse import summaries for newly discovered files in parallel.
8. Repeat until the import frontier is empty.

This preserves Wrela's source authority model: if a file is reachable, it is
reachable because the root image or another reachable file imported it
explicitly.

The import-summary parser is intentionally small. It consumes semantic tokens
from a `LexedFile` and extracts only the information needed to continue source
discovery, such as explicit imports, declared module path if present, and root
declarations. Full parsing happens later.

The discovery scheduler is graph-aware. The lexer is not. Lexing a file must
still require only that file's immutable source text. This keeps the lexer fast,
testable, and safely parallelizable.

Source discovery results should be deterministic:

- file IDs are assigned by stable path/root-discovery order
- duplicate imports resolve to one source file
- diagnostics are sorted by stable file ID and span
- discovered files are reported independently of thread completion order

## Parallel Lexing

The unit of lexer parallelism is the source file.

Parallel lexing across files gives the compiler most of the available speedup
without complicating tokenization. The lexer should not split a single file into
parallel chunks in the initial implementation. Strings, block comments, and
future multiline constructs can cross arbitrary byte positions, making
intra-file chunking a poor first tradeoff.

The batch lexer should accept already loaded immutable source files and return
immutable lexed artifacts. It may execute serially at first, but its API should
make parallel execution natural:

```text
SourceFile -> LexedFile
Vec[SourceFile] -> Vec[LexedFile]
```

When parallel execution is added, it should use the standard library before any
dependency is considered. Results must be sorted into deterministic file order
before later phases observe them.

The expected performance posture is:

- handwritten byte-oriented lexing should be very fast
- trivia preservation should store spans, not copied text
- disk loading and later parsing/typechecking are likely to dominate before
  pure tokenization does
- root-driven frontier expansion creates parallel batches naturally as imports
  fan out
- the real compiler-wide win is establishing immutable file artifacts and
  deterministic merge points from the first phase

## Build Modes

Wrela has two compile modes: dev and release.

Dev mode is for fast local work. It must emit correct code and enforce Wrela's
semantics, including ownership, authority, effects, layout legality, and
AArch64 correctness. Dev mode is not permissive. It simply avoids expensive
global work that is not required for correctness.

Dev mode should prioritize:

- fast `wrela check`
- fast `wrela test`
- rich diagnostics
- preserved source/trivia information
- minimal optimization
- early error reporting
- incremental-friendly artifacts
- correct but straightforward AArch64 output

Release mode is for production appliance images. It performs the same semantic
checks as dev mode, then spends additional budget where that budget buys
shipping value.

Release mode may perform:

- deeper reachability pruning
- stronger whole-image reports and checks
- aggressive generic specialization
- expensive vectorization and table/mask lowering
- target-specific code generation decisions
- function, section, memory, and image layout tuning
- final linker/image maps
- authority, memory, effect, and vectorization reports

Release mode should still be parallel and fast, but it is allowed to do the
expensive global work that would make dev mode feel heavy.

## Lexer Philosophy

The lexer should be lossless, file-local, deterministic, and embarrassingly
parallel.

Each source file should lex independently. The lexer should not require global
state, module graph knowledge, import expansion, string interning, or access to
other files. Its output should be a self-contained artifact containing:

- semantic tokens
- trivia
- source spans
- line-start information
- recoverable lexer diagnostics

The lexer has no dev/release mode. Lexing is semantic bedrock: the same source
file must produce the same `LexedFile` regardless of build mode. Dev and release
can choose different scheduling, optimization, and reporting policies later in
the pipeline, but they must not change tokenization.

Trivia must be preserved from the beginning. Whitespace, newlines, comments,
and doc comments are source facts needed by formatters, diagnostics, editor
tooling, and possible documentation tooling. The parser can consume a clean
semantic token stream, but the compiler must keep enough trivia information to
reconstruct source relationships later.

The lexer should store spans into the original source rather than copying token
or trivia text by default. Byte spans are the internal representation. Human
line and column positions are derived from the file's line-start table when
rendering diagnostics.

The preferred model is:

- semantic tokens in one stream
- trivia in a separate stream
- stable spans connecting both streams to the original source
- doc comments represented as a distinct trivia kind
- doc-comment attachment handled by parsing or later lowering, not by lexing

Lexer errors should be recoverable. Invalid characters, malformed numeric
literals, unterminated strings, and unterminated block comments should produce
diagnostics while allowing the lexer to continue producing tokens where
possible.

The lexer output should be immutable. Later phases may reference it, but they
should not mutate it. That keeps lexing safe to parallelize across files and
keeps formatting and diagnostics from fighting the parser over ownership of
source text.

## Consequences

Benefits:

- Fast clean builds for a Rust compiler project.
- Fast edit/check cycles.
- Small audit surface.
- Fewer transitive maintenance surprises.
- Compiler architecture stays visible while the language is still forming.
- Root image files remain the only source of reachability.
- The compiler can become parallel without redesigning its phase boundaries.
- Dev builds stay fast while release builds have room for deeper production
  work.
- Lossless lexing keeps formatting and diagnostics possible without re-lexing
  or source reconstruction.
- The tool's implementation philosophy matches Wrela's language philosophy:
  explicit authority, explicit dependency edges, and no ambient magic.

Costs:

- More handwritten infrastructure early.
- CLI help and diagnostics will be simpler at first.
- Some polish must wait.
- The project must resist repeatedly rebuilding mature crates poorly.
- More care is required to keep phase artifacts immutable and merge points
  deterministic.
- Preserving trivia increases lexer output size.
- Root-driven discovery requires an import-summary parser before the full parser
  exists.
- Dev and release modes must be tested against the same semantic rules so they
  do not drift.

The intended tradeoff is not permanent minimalism. It is delayed commitment.
When a dependency becomes obviously worth it, Wrela should add it deliberately,
with the decision recorded.
