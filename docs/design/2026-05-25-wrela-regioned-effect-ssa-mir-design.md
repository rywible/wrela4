# Wrela Regioned Effect SSA MIR Design

Date: 2026-05-25

## Purpose

This document specifies **W-MIR**, the mid-level intermediate representation that sits between the semantic artifacts in `CheckResult` (produced by `check`: parsed CST, summaries, resolution, signatures, body / ownership / effect / layout facts) and the AArch64-near low-level IR produced for codegen. W-MIR is the artifact every later compiler phase consumes. It is the long-lived shape on which optimization, lowering, and codegen are built.

W-MIR is **Regioned Effect SSA**: structured regions for control flow, SSA with block arguments inside regions, explicit places for memory, effect bitsets and state edges for side-effect tracking, and first-class IR values for the language primitives that mainstream IRs throw away (capability paths, table columns, masks, frame handles, row tokens).

The design has two compile modes:

- **Dev mode** never depends on an equivalence-class machinery. It verifies, canonicalizes cheaply, and lowers directly to AArch64 LIR. Target: sub-second clean builds for 10K LOC.
- **Release mode** builds bounded region-local **aegraph overlays** on top of W-MIR, applies effect-gated rewrites, performs cost-driven scoped elaboration, and lowers. Target: 5-30s clean builds for a typical appliance image.

The IR shape is the same across both modes. The release pipeline is a *mechanism* layered onto W-MIR, not a separate IR.

### Relationship to other documents

- Language semantics: [`2026-05-22-wrela-language-and-test-design.md`](2026-05-22-wrela-language-and-test-design.md). W-MIR represents the same semantic facts that document specifies (ownership, effects, capability authority, tables, masks, layouts, loop shapes). The MIR does not invent new language features.
- Compiler and toolchain principles: [`../design-principles.md`](../design-principles.md). W-MIR honors zero external dependencies, parallel immutable phases, diagnostics as data, root-driven reachability.

### Scope

This document specifies:

- The shape of W-MIR (regions, values, places, types, ops, effect attributes, state edges).
- The dev and release pipelines at the architectural level.
- The verification, textual-dump, and debugging contract every pass must honor.
- The first-class performance tooling contract for compiler speed and generated-code speed.
- The phased delivery (MIR 01 / MIR 02 / MIR 03 / MIR 04 / MIR 05 / MIR 06+) with acceptance criteria.
- The list of research extensions that are explicitly **not** in MIR 01-03.

This document does **not** specify:

- The full opcode reference (deferred to a separate `opcode-reference.md` once MIR 01 lands).
- The detailed aegraph data structure and saturation algorithm (deferred to the MIR 03 spec).
- The AArch64 LIR and instruction selection (sketched here; full spec deferred to the MIR 02 backend spec).
- The cost-model table format and per-microarch tuning data (deferred to the MIR 02 cost-model spec).

## Principles

These principles are the durable commitments. Specific opcodes, region kinds, and pass ordering may change; these should not.

- **Regioned Effect SSA.** Regions where structure buys reasoning (functions, matches, loops, frames, image roots); SSA-with-block-arguments inside regions for straightforward dump/walk/verify/lower. No goto, no implicit phi.
- **The IR carries the language's facts.** Mask provenance, capability paths, effect bitsets, ownership modes, frame lifetimes, trip counts, and layout are represented directly, not recovered.
- **One IR, two modes.** Dev mode never invokes equality machinery. Release mode adds region-local aegraph overlays. Both consume the same W-MIR shape.
- **Every pass has a verifier and a deterministic textual dump.** No graph IR ships without round-trip-parseable text, stable node IDs across runs, and a per-pass `verify()`. This is the V8 lesson, applied day one.
- **Effect-gated rewrites.** No rewrite fires without a precondition expressed in terms of effect subsets, capability paths, ownership modes, and type/bounds facts. Aliasing analysis is type-and-path analysis.
- **Trident-certified optimization.** Starting with the first MIR 03 rewrite, every applied release rewrite must be provable enough to explain, measured enough to trust when generated-code support exists, and recorded enough to debug later. The optimizer is not allowed to accumulate unrecorded magic.
- **Stable identity before optimizer memory.** Region, operation, value, fact, pass-config, target-profile, and MIR-snapshot hashes are durable compiler artifacts. Persistent optimization memory is keyed by these identities rather than by display text or source locations alone.
- **Measurement is a first-class compiler artifact.** Wrela does not rely on vibes for compile speed or generated-code quality. Every MIR/backend phase that claims performance value ships with lightweight, deterministic-enough tooling that reports directional compile-time, IR-size, code-size, runtime, and pass-effect signals.
- **No ambient infrastructure.** Zero external dependencies, handwritten, arena-allocated. The compiler builds and runs without LLVM, without MLIR, without Z3, without `egg`. Algorithms may be cribbed; libraries are not linked.

## The IR shape (W-MIR)

### Overview

A W-MIR module is a forest of `λ` regions (one per monomorphized method) and `ω` regions (one per `image` or `host image` root). Each region contains a body composed of SSA basic blocks, nested regions, and operations. The module is immutable once constructed; every pass produces a new module (sharing unchanged subtrees by reference).

### Regions

Regions are structural. They are first-class IR nodes with typed inputs, typed outputs, and a body. The region kinds are:

- **`λ` (lambda)** — a function body after monomorphization. Inputs: parameters and captured capabilities. Outputs: return value and inferred effect set. The body is a control-flow graph of basic blocks with block arguments.
- **`γ` (gamma)** — a `match`. N typed arms, exactly one of which fires. Each arm is a region body. Inputs: scrutinee + free variables. Outputs: per-arm result join (typed).
- **`θ` (theta)** — a loop. One sub-kind per Wrela loop shape, each carrying its facts:
  - `θ-repeat` — counted N-trip loop. Carries the trip count (const, dynamic, or bound).
  - `θ-for` — table-row loop. Carries the row-token type and table provenance.
  - `θ-drain` — bounded batch from a queue/ring. Carries the max-item bound.
  - `θ-reduce` — bounded accumulator. Carries the accumulator type and the accumulator op shape (associative? commutative? saturating?).
  - `θ-scan` — bounded sentinel search. Carries the byte range and the `until` predicate.
  - `θ-loop` — intentional unbounded. Body must exit via `return` or `trap`.
- **`δ` (delta)** — a `with` frame. Inputs: arena handle + budget. Outputs: results that escape the frame. Interior values carry a frame-lifetime tag in their type; they cannot escape the region's output set.
- **`ω` (omega)** — an `image` or `host image` root. Top-level composition only. Inputs: the unique platform/host authority. Outputs: phase ordering.

Regions nest. A `λ` body may contain `γ`, `θ-*`, and `δ` regions. A `θ-*` body may contain further nested regions. A `δ` lifetime cannot be escaped by anything except explicit outputs.

### SSA with block arguments

Inside a region body, control flow is a graph of basic blocks. Blocks take **typed arguments** instead of having `phi` nodes at the top. A branch to a target block supplies the argument values; the target block reads them as its parameters. Equivalent in power to phi-style SSA, simpler to dump, debug, and pattern-match in rewrites. Inspired by Cranelift CLIF, Swift IL, rustc MIR.

Every value is produced by exactly one operation or block argument and has a fixed type. Operations consume values and produce values. There is no mutable variable; mutation is expressed through `Place` reads/writes (next section).

### Places vs values

W-MIR distinguishes two categories:

- **`Value`** — what flows through SSA. A typed, immutable result of an operation. Categories: scalars, capabilities, columns, masks, row tokens, frame handles, bytes, addresses, sums, tuples, fixed vectors.
- **`Place`** — what you read, write, or borrow. Categories: locals, fields of an owned aggregate, table columns, index slots, MMIO cells, arena-allocated objects, DMA buffers, frame-scoped scratch.

Every place has a **type** (what's stored), an **owner region** (where its lifetime is rooted), and an **ownership mode** at the use site (`read`, `mut`, `own`). A `Place` is *not* an SSA value — it is a designator. Operations like `place.load`, `place.store`, `place.borrow_read`, `place.borrow_mut`, `place.borrow_own` convert between places and values. This split makes ownership and borrow analysis local: the IR types tell you what a place's mode is, without recovering it from def-use chains.

This is the rustc-MIR `Place` distinction, kept because it pays for itself.

### Types

The W-MIR type system is post-monomorphization and post-`check`. No generics, no interfaces, no parametric types remain. The categories:

- **Scalars** — `U8`, `U16`, `U32`, `U64`, `USize`, `I8`, `I16`, `I32`, `I64`, `ISize`, `F32`, `F64`, `Bool`, `None`.
- **Closed sums** — `Enum{name, variant}`, `Error{name, variant}`. Match exhaustiveness is checked at construction.
- **Logical records** — `Data{name}` with compiler-laid-out fields.
- **Physical records** — `LayoutData{name, layout=C|packed|mmio}`. ABI-stable; field offsets fixed.
- **Fixed vectors** — `Vec[N, T]` for kernel-shaped values.
- **Capability handles** — `Capability{class, path}`. The path is a sequence of derivations from a unique root, baked into the type. Two `Capability` values with the same class but different paths are **statically non-aliasing**. Capability paths are compile-time only; they lower to nothing at runtime.
- **Frame handles** — `FrameHandle{δ}`. The δ tag identifies which `δ` region the handle came from. Values produced inside a δ region carry the same tag and cannot escape regions that don't include δ in their lifetime context.
- **Arena references** — `ArenaRef{identity}`. Typed by authority origin.
- **Memory views** — `Buffer[T]`, `ReadBuffer[T]`, `Bytes`, `Address`, `PhysicalAddress`, `Mmio[T]`, `DmaBuffer[T]`. These are values describing bounded views; the bound is part of the type.
- **Columnar primitives**:
  - `Table[T, N]` — a typed table of `N` rows of `Data{T}` records.
  - `Column[T, table, N]` — a typed column view (one field across all rows). The `table` parameter identifies the source table — masks/columns from different tables cannot be combined.
  - `Mask[table, N]` — a typed predicate over rows. The `table` provenance is in the type; mask algebra on masks of different tables is a type error.
  - `RowToken[table]` — a scoped capability for a single row of `table`. Cannot escape the `θ-for` body that produced it, cannot be stored in `Place`, cannot be converted to a pointer.
  - `Index[K, table, N]` — typed lookup metadata.
- **Synchronization** — `Atomic[T]` with explicit memory-order.
- **Bottom** — `Never`. Inhabited only by control flow that does not return (`trap`, `return`, infinite `loop`).

Type-level provenance is the central trick. `Mask[A, 256]` and `Mask[B, 256]` for different tables `A` and `B` are different types. A rewrite that combines them is a type error, not an alias-analysis question. Similarly, `Capability{Foo, p1}` and `Capability{Foo, p2}` with different paths are distinguishable by the IR; downstream uses can be analyzed for non-aliasing without ad-hoc passes.

### Operations

Operations are the leaves of regions. Every operation has:

- An **opcode** (typed).
- **Operands** (typed values, each carrying an ownership mode where applicable).
- **Results** (typed values).
- **Attributes** — at minimum an effect bitset (see below) and source-location metadata.
- A **state-edge slot** if the operation is side-effectful (see below).

The full opcode catalog will be specified in a separate `opcode-reference.md` after MIR 01 lands and the categories stabilize. The categories anticipated:

- **Scalar arithmetic** with explicit overflow semantics (trap, wrap, sat, check).
- **Comparisons** producing `Bool` or `Mask` depending on operand type.
- **Bitwise** including count/leading-zeros/trailing-zeros.
- **Conversions** (trapping/checked for narrowing).
- **Select** — pure ternary `(cond, a, b)`. Equivalent to a pure `γ` region with two arms.
- **Place operations** — `place.load`, `place.store`, `place.borrow_*`, `place.frame_place`, `place.frame_reserve`, `place.arena_child`.
- **Volatile/MMIO** — `mmio.load`, `mmio.store` with explicit ordering. Gated by capability witness.
- **Atomics** — `atomic.load`, `atomic.store`, `atomic.rmw` with explicit memory order.
- **Columnar / data plane** — `table.column`, `table.scan_mask`, `table.gather`, `table.scatter`, `table.insert`, `table.insert_or_trap`, `mask.and`/`or`/`not`/`xor`/`count`/`any`/`all`/`combine`/`filter`.
- **Capability operations** — `cap.derive` (narrowing), `cap.witness` (proving authority at a use site).
- **Calls** — `call` (monomorphized; carries the callee's inferred effect set), `call.asm` (gated by capability witness).
- **Termination** — `trap` (Never, with TrapCode + source location), `return`, `yield` (reduce), `break`, `continue`.

### Effect bitsets

Every operation carries an **effect attribute** — a bitset over the language's effect kinds:

- `Trap` — may leave normal control flow via `trap`.
- `Io` — touches a device or host IO capability.
- `Volatile` — performs MMIO or volatile memory access.
- `Time` — reads a clock.
- `Entropy` — reads randomness.
- `Arena` — consumes arena or frame capacity.
- `Mutate` — mutates owned state through `mut self` or a mutable capability.
- `Dma` — hands memory to a device.
- `Block` — may wait for an external event.
- `AddressArithmetic` — manipulates raw addresses or pointer-shaped values.
- `Assembly` — enters an assembly function.

The bitset is **monotonic under composition**. A region's effect set is the union of its body's effect sets. A `λ`'s signature exposes its effect set as part of the function type, used for `vectorize require` enforcement, hosted determinism checks, and interrupt-context safety analysis.

Effect bitsets are the primary precondition for rewrite rules. A rule like "fuse two `θ-for` loops over the same table" fires only if the bodies' effect sets are subsets of `{Mutate, Arena}` and the mutations don't overlap.

### State edges

Effect bitsets describe **what** kinds of effects an op has. **Ordering** between effectful ops is encoded by explicit **state-edge values**. A state edge is a typed `StateToken{kind}` value produced by one op and consumed by the next, threading effectful operations in their required order. This is the RVSDG state-edge mechanism.

State edges partition by effect kind: an MMIO op consumes/produces an `Mmio[device]` state token; a table mutation consumes/produces a `TableState[table]` state token; a relaxed-order atomic op consumes/produces an `Atomic[location]` state token. Disjoint state tokens of the same partitionable kind (different devices, different tables, different relaxed-order atomic cells) do not order with each other — they can be reordered freely.

**Cross-location ordering exists and is encoded explicitly.** Acquire / release / sequentially-consistent atomics and explicit `Fence(order)` operations can establish happens-before relationships across otherwise-disjoint state tokens. These ops consume/produce a `SyncToken{order}` value in addition to their location-specific state token. Rewrites must preserve `SyncToken` dataflow. The verifier checks both per-location state-edge connectivity and `SyncToken` ordering. Sequentially-consistent ops in particular thread a `SyncToken{Sequential}` value through a total order; reordering across them is rejected at rule registration.

State edges make effect ordering a SSA-value-flow problem. Rewrites must preserve the per-location state-edge dataflow and the cross-location `SyncToken` dataflow; the verifier checks this mechanically.

### Trap-producer provenance

Operations that may trap (effect `Trap`) carry a **TrapCode** + source location in their attributes. A `trap` operation produces a `Never`-typed value that flows to a region terminator. Movement of trapping ops is constrained: a rewrite cannot move a may-trap op past another op that observes state in a way that would change which trap fires first.

The verifier enforces: every `Never` value flows to a terminator; every reachable terminator has a unique provenance; rewrites preserve the partial order of trap-producers.

### Capacity and bounds tokens

Operations on bounded structures (tables, arenas, frames, queues) consume and produce **capacity witness tokens**. A `Table.insert` consumes a capacity token (which encodes "row N is free") and produces a new one (encoding "row N+1 is free"). A `frame.reserve(bytes)` consumes a budget token and produces a smaller one.

This makes capacity-respecting rewrites local. A rewrite that reorders inserts must preserve the capacity-token dataflow; if it can't, the rewrite doesn't fire.

### Ownership modes on operands

Every operand of an operation carries an **ownership mode**: `read`, `mut`, or `own`. The mode is part of the operand's type (e.g., `read Buffer[U8]` vs `mut Buffer[U8]`). The verifier checks the standard borrow rules:

- Multiple `read` borrows of the same place may coexist.
- A `mut` borrow excludes all other borrows.
- An `own` operand consumes the place.
- Borrowed values cannot escape the region in which the borrow was created.

These rules are checked at the operand level, not as a separate dataflow pass. Most of the work is done in `check`; W-MIR just preserves the modes.

## Pipeline

### Dev mode

```
CheckResult → mir.build → W-MIR
              │
              ▼
          verify (per pass)
              │
              ▼
          canonicalize (cheap)
              │   constant fold, dead code via mark-sweep,
              │   trivial peeps, region simplification
              ▼
          lower (W-MIR → AArch64 LIR)
              │
              ▼
          single-pass register allocation
              │
              ▼
          emit AArch64
```

**No hash-cons. No e-graph. No equality classes. No rewriting.** Direct lowering of every region kind to its corresponding LIR pattern. Target: sub-second clean builds for 10K LOC programs on M1-class hardware.

### Release mode

```
CheckResult → mir.build → W-MIR
              │
              ▼
          verify (per pass)
              │
              ▼
          canonicalize (cheap)
              │
              ▼
          ┌── for each λ region: ──┐
          │   build region-local   │
          │   aegraph overlay      │   (acyclic equality structure
          │       │                │    over pure subgraphs only;
          │       ▼                │    side-effect skeleton stays
          │   apply rewrites       │    in W-MIR)
          │       │                │
          │       ▼                │
          │   scoped elaboration   │
          │   back to W-MIR        │
          └────────────────────────┘
              │
              ▼
          lower (W-MIR → AArch64 LIR)
              │
              ▼
          smarter register allocation (backtracking)
              │
              ▼
          emit AArch64
```

Release-mode rewrites operate on **pure subgraphs within regions**. Side-effectful ops (those with non-empty effect sets, with state-edge dataflow, with trap producers, with capacity tokens) stay in W-MIR's skeleton and are not lifted into the equality structure. This is the Fallin/Cranelift aegraph discipline.

Rewrites are **effect-gated**: a rule fires only if its precondition (effect subset, capability path constraints, ownership modes, type/bounds facts) holds on the matched subgraph. Rewrites that would weaken the effect set or violate ownership are rejected at rule registration.

Saturation is **bounded**: per-region fuel budget, phase-scheduled (canonicalization → algebraic → mask/table → lowering-friendly), deterministic across runs. Extraction is **greedy single-pass scoped elaboration** with a cost model. Exact extraction is not in MIR 03.

### Lowering ladder

`W-MIR → LIR → AArch64 machine code`

LIR (Lowered IR) is a separate, much simpler representation: scalar SSA with explicit register classes, fixed-vector ops, and AArch64-near instruction templates. LIR drops Wrela-level abstractions — no regions, no places, no capability paths, no Wrela effect bitsets, no Mask/Column/RowToken types. Those have been lowered into machine instructions, addressing modes, and ordering fences.

**LIR still carries target-level instruction metadata** sufficient for sound scheduling and register allocation:

- **Clobbered registers** per instruction (call-clobbered set, condition flags clobber, vector register clobber).
- **May-trap** flag and the trap kind (e.g., load-from-unmapped, divide-by-zero, overflow when trapping arithmetic was emitted).
- **Memory ordering** (relaxed / acquire / release / seq-cst / fence with explicit order).
- **Volatile / MMIO** flag on memory ops, preventing reordering across volatile boundaries.
- **Side-effect class** (pure / memory-read / memory-write / volatile / synchronization / control-side-effect).

Without this metadata, a scheduler could legally reorder a volatile load past an acquire fence, which would be unsound. LIR's metadata is the minimum target-level subset of the Wrela-level effect bitsets and state edges, lowered into a form codegen passes can consume directly. LIR is detailed in a separate spec under MIR 02.

## Verification, debugging, and tooling

This section is intentionally elevated. Graph IRs lose maintainability quickly without it. V8 abandoned Sea of Nodes in 2025 primarily for debuggability reasons. The contract here is day-one.

### Per-pass verifier

Every pass that produces W-MIR must come with a `verify()` function that checks the invariants:

- All operands are dominated by their definitions.
- **Linear resources are consumed exactly once.** State tokens (`Mmio[device]`, `TableState[table]`, `Atomic[location]`, `SyncToken{order}`), capacity tokens, frame handles, row tokens, and other linear types must be threaded through dataflow with exactly one producer and one consumer.
- **Ordinary SSA values may have any number of uses.** They must satisfy standard dominance and type-compatibility checks; multi-use is the norm.
- Every region's signature matches its body.
- Every `θ-for` body uses only `RowToken` values that originated in the same `θ-for`.
- Every `δ` region's interior values do not appear in non-output positions outside the region.
- Every `Capability` value flowing into a privileged op has a path that proves the required authority.
- Every state-edge dataflow is connected.
- Every `Never` value flows to a terminator.
- The effect bitset on each op accurately summarizes its body (computed bottom-up; cross-checked against the attribute).

The verifier runs after every pass in dev mode and after every rewrite phase in release mode. In CI / quality-gate runs, it runs unconditionally. The verifier is an immutable check, not a transformer.

### Deterministic textual dump

W-MIR has a canonical textual format. The dump:

- Is **deterministic across runs**: the same input produces the same text byte-for-byte. No `HashMap` iteration leaking into order. No timestamps. No memory addresses.
- Uses **stable node IDs** that survive across passes when the underlying node didn't change. New nodes get fresh IDs; passes do not renumber.
- Is **round-trip parseable** with a textual parser that produces a W-MIR module equivalent to the original. The pair `parse(dump(m)) == m` is a test invariant.

The dump format is intended to be human-readable. It uses indentation for region nesting, named operands, explicit types, and effect-set annotations. It is not optimized for parsing speed; it is optimized for debuggability.

### Stable node IDs

Every region, block, operation, and value has a stable ID. IDs are assigned at construction; passes that don't modify a node preserve its ID. This makes diff-based debugging tractable.

### Stable hashes and identities

Stable IDs make dumps readable, but optimizer memory needs content-derived identity. W-MIR therefore defines stable, deterministic hashes for:

- Module and region snapshots after textual canonicalization.
- Operation kinds, operands, results, effect sets, source spans, and semantic facts.
- Rewrite certificate facts.
- Pass configurations.
- Target profiles.

MIR 03 introduces `StableHash` V0 as a handwritten FNV-1a 64-bit hash over canonical text fragments. It is not a security hash and is not used for trust boundaries. MIR 05 upgrades this to stable identity V1 by including certificate schema version, target profile, pass config, and compiler version in ledger keys. Hash algorithm or schema changes invalidate old optimizer-ledger entries unless a migration is explicitly implemented.

### Trident rewrite certificates

Every applied release rewrite emits a `RewriteCertificate`. The V1 shape is locked as:

```text
certificate_version: u16
rule: stable rule name
pass: stable pass name
region: RegionId
source_ops: [OperationId]
before_hash: StableHash
after_hash: StableHash
required_facts: [RewriteFact]
verifier: passed | rejected
```

Certificates are not proofs in a theorem-prover sense. They are structured receipts that state exactly which compiler facts justified the rewrite and whether the post-rewrite verifier accepted the candidate MIR. A missing fact, stale certificate version, stale hash schema, or failed verifier result means the rewrite cannot be recorded as applied.

`RewriteFact` is intentionally closed and versioned. MIR 03 starts with scalar facts such as `pure-operation`, `literal-zero`, `effect-absent`, and `verifier-passed`. MIR 04 adds data-plane facts such as `same-mask-domain`, `same-table`, and `same-row-count`. MIR 05 adds authority-algebra facts such as `disjoint-capabilities`, `independent-ordering-domain`, `state-edges-preserved`, and `trap-order-preserved`.

### Authority algebra engine

Legality checks must not become scattered helper functions. MIR 04 introduces an authority query layer and MIR 05 hardens it into an engine. The engine answers these questions from W-MIR facts:

```text
same_table(a, b)
same_mask_domain(a, b)
effect_absent(region, Volatile)
disjoint_capabilities(a, b)
independent_ordering_domain(a, b)
state_edges_preserved(before, after)
trap_order_preserved(before, after)
```

Rewrites call the engine to obtain facts, then copy those facts into their certificates. If the query cannot answer yes with current MIR facts, the rewrite does not fire.

### Trident failure handling

The optimizer fails closed. A rewrite, certificate, hash, ledger, measurement, or reducer failure disables the affected optimization path for that run, emits a structured report, and preserves the smallest artifact Wrela can produce. It never silently trusts broken infrastructure and never substitutes a speed claim for a failed checksum or verifier result.

### Minimal-counterexample reduction (planned, post-MIR-01)

When a verifier check fails or a differential test catches a regression, a reduction tool shrinks the input to a minimal W-MIR module that still triggers the failure. Reduction uses structural shrinking (drop ops, collapse regions, simplify constants) preserving the failure mode. Likely surfaced later as `wrela dump mir-reduced`.

This is a planned addition, not a MIR 01 hard requirement. MIR 05 implements the first reducer for rewrite verification failures, checksum mismatches, and measurement sanity failures.

## Performance tooling

Performance tooling is part of the compiler product, not an external lab
project. It must be cheap enough to run during normal development, structured
enough to compare runs, and honest enough to guide tuning without pretending to
produce publication-grade statistics.

The performance contract has two halves:

- **Compiler telemetry** answers "did this compiler change make Wrela slower to
  compile, or did it change the size/shape of the IR?"
- **Generated-code benchmarks** answer "did this lowering, cost model, or
  research optimization make the emitted program faster, smaller, or worse?"

Both halves are directional. They should make obvious wins and losses visible.
They are not a replacement for deep benchmarking when a result is subtle.

### Compiler telemetry

`wrela perf compile <root.wrela> --mode dev|release --repeat N` measures the
compiler pipeline itself. It reports human-readable output by default and JSON
with `--json`.

The command records:

- Wall-clock time per phase: discover, lex, parse, check, `mir.build`, verify,
  canonicalize, release rewrite phases, lower, register allocation, emit.
- W-MIR counters: modules, regions, blocks, operations, values, places, state
  tokens, capacity tokens, effectful ops, maximum region nesting.
- LIR/codegen counters once MIR 02 exists: LIR instructions, blocks, register
  classes used, spills/reloads, emitted bytes.
- Release optimizer counters once MIR 03 exists: aegraph regions visited,
  rewrite count, fuel used, elaborated nodes, inlining count, loop fusions,
  mask rewrites.

Default runs should be short: one warmup when useful, then five to seven timed
iterations. The summary reports min, median, max, and optional relative delta
against a baseline JSON file. The timer uses `std::time::Instant`; no external
benchmark dependency is permitted.

Compiler telemetry is not part of `./scripts/quality-gate.sh` by default. Perf
is too environment-sensitive for the regular correctness gate. MIR 02 should add
lightweight scripts such as `scripts/perf-smoke.sh` and `scripts/perf-compare.sh`
for intentional local checks.

### Generated-code benchmarks

`wrela perf code <bench-root.wrela>` measures emitted code. This is the tool that
keeps research optimizations honest.

The command supports A/B comparisons:

```text
wrela perf code benches/data/filter_sum.wrela \
  --baseline "release --disable-pass mask-fusion" \
  --candidate "release --enable-pass mask-fusion" \
  --repeat 7 \
  --json
```

Equivalent comparisons may use dev vs. release, release with a pass disabled,
release with only one pass enabled, or named pass sets. Every release optimizer
pass that claims speed must be independently switchable enough for this kind of
comparison.

Generated-code benchmark output records:

- Runtime summary: min, median, max, and relative delta against baseline.
- Output checksum or explicit observable result, checked before timing is
  trusted.
- Emitted code size.
- LIR instruction count and spill/reload count where available.
- W-MIR operation count before and after the candidate pass set.
- Release optimizer counters: rewrite count, fuel used, fusions, selected
  vector width, selected branch/select strategy.
- Target CPU profile, compiler git revision, pass set, and benchmark input
  identity.

Benchmarks are tiered:

- **Microbenchmarks** isolate one optimization: mask fusion, column elision,
  branch-vs-select, bounds-token elimination, `reduce`, `scan`, table scatter.
- **Kernel benchmarks** model realistic data-plane kernels with deterministic
  inputs and enough work to rise above timer noise.
- **Image benchmarks** arrive later, once hosted or QEMU roots can execute whole
  programs repeatably.

Benchmarks must be anti-cheat by construction. They consume deterministic input
and return a checksum or observable result. Dev and release outputs, or baseline
and candidate outputs, must agree before runtime numbers count.

### Directional significance

The tooling is designed to steer engineering decisions, not to settle fine
statistical arguments. A targeted generated-code pass result is
**directionally significant** when baseline and candidate checksums match, each
side has at least seven samples, `p90 / p10 <= 1.10`, and median runtime moves
by at least 3%. Broader architectural claims, compile-time claims, and changes
that affect many compiler phases still require a larger 10% movement across two
local runs. Smaller changes are reported, but they do not justify architectural
decisions by themselves.

Every benchmark result should be easy to save as JSON and compare later. The
format must be stable enough for agents and scripts to produce concise before /
after notes in implementation plans and review packets.

## Worked example

A simple Wrela method through dev and release.

### Source

```wrela
class PacketClassifier {
    fn mark_jumbo(read self, packets: mut Table[Packet, 256]) -> None {
        let valid = packets.flags.has(PacketFlag.Valid)
        let large = packets.len > 1200
        let jumbo = valid & large
        packets.flags[jumbo].set(PacketFlag.Jumbo)
        return None
    }
}
```

### W-MIR (after construction)

```text
λ PacketClassifier.mark_jumbo (
    self:     read Capability{PacketClassifier, ...}
    packets:  mut Place(Table[Packet, 256], owner=caller)
) -> (None, effects={Mutate})

  block entry(self, packets):
    %st0 : TableState[packets]    = state.token packets
    %col_flags : Column[U8,  packets, 256] = table.column packets, .flags
    %col_len   : Column[U16, packets, 256] = table.column packets, .len

    %valid : Mask[packets, 256] = mask.from_has  %col_flags, PacketFlag.Valid
    %large : Mask[packets, 256] = mask.cmp.gt    %col_len,   1200
    %jumbo : Mask[packets, 256] = mask.and       %valid,     %large

    %st1 : TableState[packets] = table.scatter_set
                                    state=%st0,
                                    %col_flags, %jumbo, PacketFlag.Jumbo
                                    effects={Mutate}

    return None
```

### After release-mode aegraph overlay

The pure-subgraph aegraph contains the three `mask.*` ops. Algebraic identities fire (in this case there are no further simplifications because the source mask expression is already irredundant), but CSE shares `%valid` between `mask.and` and the later mask used by `scatter_set`. Cost model picks NEON-128 vectorization based on table layout, alignment, and the effect set permitting fusion of the gather + AND + scatter into a single mask-write pass.

### Lowered (sketch)

```asm
PacketClassifier.mark_jumbo:
  mov   x2, #1200
  dup   v3.8h,  w2
  mov   w2,     #(PacketFlag.Valid)
  dup   v4.16b, w2

  mov   x3, #0
.Loop:
  ld1   {v0.16b}, [x_flags, x3]
  ld1   {v1.8h},  [x_len,   x3, lsl 1]
  and   v0.16b,   v0.16b,   v4.16b
  cmgt  v1.8h,    v1.8h,    v3.8h
  and   v2.16b,   v0.16b,   v1.16b
  orr   v0.16b,   v0.16b,   v2.16b
  st1   {v0.16b}, [x_flags, x3]
  add   x3, x3, #16
  cmp   x3, #256
  b.lt  .Loop
  ret
```

The mask algebra never went through scalar code on the way down. The `Mask[packets, 256]` type is the IR-level fact that licenses the lowering directly to NEON predicates.

## Phased delivery

The phasing puts a rock-solid IR artifact in place before any backend or optimizer work touches it. Each phase is an independent implementation plan with its own acceptance criteria.

### MIR 01 — Build / verify / dump

The IR artifact and the day-one debugging contract. No codegen, no LIR, no regalloc, no AArch64 emission.

**Scope:**

- W-MIR shape: regions (`λ`, `γ`, `θ-*`, `δ`, `ω`), SSA-with-block-args, values vs places, types (including capability paths, masks/columns/tables, frame handles), effect bitsets, state edges (per-location + `SyncToken`), trap-producer provenance, capacity tokens, ownership modes on operands.
- `mir.build`: lowering from `CheckResult` to W-MIR.
- Per-pass verifier covering every invariant in the [Verification](#per-pass-verifier) section.
- Deterministic textual dump and round-trip parser. `parse(dump(m)) == m` test invariant.
- Stable node IDs preserved across passes when a node is unchanged.
- Lightweight compiler telemetry scaffolding for `mir.build`: phase timing and
  W-MIR counters emitted through a future-compatible internal report structure.
- `wrela dump mir <root.wrela>` CLI subcommand.

**Acceptance criteria:**

- Every existing `wrela check` fixture lowers to a valid W-MIR module.
- The per-pass verifier runs after `mir.build` and produces no diagnostic on the regression suite.
- `parse(dump(m)) == m` holds across the regression suite.
- `wrela dump mir` produces byte-for-byte identical output across runs on the same input.
- `mir.build` telemetry reports deterministic W-MIR counters for every fixture
  and does not require external dependencies.
- No external crate dependencies beyond what `wrela` already uses.

**Out of scope:** LIR, lowering to AArch64, register allocation, codegen, cost tables, generated-code benchmarks, equality machinery, aegraph, release-mode rewrites, minimal-counterexample reducer, slotted e-classes, SMT validation, SVE, image-graph PGO, incremental caching.

### MIR 02 — Dev codegen (LIR + AArch64)

The dev-mode lowering path. Take MIR 01's verified W-MIR and produce correct AArch64 quickly.

**Scope:**

- LIR (Lowered IR) — scalar SSA with target-level metadata (clobbers, may-trap, memory order, volatile flags, side-effect class). LIR shape detailed in its own backend spec under this phase.
- `lower`: W-MIR → LIR. Direct lowering for every language feature the parser and `check` support.
- Cheap canonicalization (constant fold, dead code via mark-sweep, trivial peeps, region simplification).
- Single-pass register allocation.
- AArch64 emission.
- One target microarch with hand-curated cost-table seed (Apple Firestorm-class or Cortex-A78 — chosen during implementation based on dev hardware).
- `wrela perf compile` for dev-mode compiler telemetry.
- `wrela perf code` for generated-code smoke benchmarks in dev mode.
- Smoke benchmark suite + regression harness comparing dev-mode output
  bit-for-bit across builds and checking benchmark output checksums.

**Acceptance criteria:**

- Every existing `wrela check` fixture compiles to AArch64 in dev mode.
- A 10K LOC Wrela program builds in under 1 second clean on an M1-class machine.
- LIR has its own per-pass verifier covering the target-level metadata invariants.
- `wrela perf compile` reports phase timings and MIR/LIR/codegen counters in
  human and JSON forms without external dependencies.
- `wrela perf code` runs at least one deterministic generated-code
  microbenchmark, checks its output checksum, and reports runtime, code-size,
  and instruction-count summaries.
- No regressions in MIR 01 artifacts (every fixture still produces the same W-MIR dump).
- No external crate dependencies beyond what `wrela` already uses.

**Out of scope:** any equality machinery, release-mode rewrites, aegraph, cost-model-driven decisions, backtracking regalloc, slotted e-classes, SMT validation, SVE, image-graph PGO, incremental caching, pass-level A/B optimizer comparisons.

### MIR 03 — Certified scalar release optimizer slice

The first release-mode pipeline slice. Aegraph overlays + scoped elaboration are introduced here, but the delivered rewrite surface is intentionally scalar. This phase also introduces Trident certificate/event scaffolding before the first rewrite lands, so the optimizer never has an undocumented pre-doctrine rewrite family.

**Scope:**

- Release pipeline: verify → region-local aegraph overlays → scoped elaboration → MIR 02 LIR / codegen.
- Hash-cons + acyclic e-class data structure for pure subgraphs within regions.
- Stable hash V0 for module and region snapshots.
- Rewrite certificate/event V0 with schema version, rule, pass, region, source ops, before/after hashes, explicit legality facts, and verifier result.
- Bounded in-memory rewrite log with deterministic truncation behavior.
- Phase-scheduled rewriting with per-region fuel budget.
- First rewrite families: scalar arithmetic identities through the overlay, plus pass-control scaffolding for dead-code and copy-prop names.
- Greedy single-pass scoped elaboration. No ILP/exact extraction.
- Differential generated-code smoke harness for the scalar fixture: dev-mode output vs release-mode output produce the same checksum before runtime deltas are interpreted.
- Pass-level A/B controls for release optimizations (`--enable-pass`,
  `--disable-pass`, `--only-pass`, or named pass sets), with left-to-right deterministic precedence.
- Generated-code performance corpus covering scalar peepholes.

**Acceptance criteria:**

- Release mode produces correct AArch64 for the MIR 03 scalar supported subset.
- A 10K LOC program builds in under 10 seconds in release mode on M1-class hardware.
- The scalar identity fixture shows matching checksums between dev and release.
- Each MIR 03 scalar release pass has a `wrela perf code` A/B benchmark result or a written explanation that the pass is correctness/canonicalization-only in this slice.
- Each applied scalar rewrite emits a versioned certificate and event. A missing fact prevents the rewrite from firing.
- Rewrite event output is bounded and deterministic.
- Directionally significant scalar generated-code claims follow the 3% / seven-sample rule from
  the [Performance tooling](#directional-significance) section.
- No regressions in dev-mode performance or compile speed.

**Out of scope:** mask algebra, branch/select conversion, table loop fusion, data-plane cost tables, backtracking regalloc, slotted e-classes, SMT validation, SVE, image-graph PGO, multi-microarch cost tables, incremental caching.

### MIR 04 — Certified data-plane mask optimizer

The release-mode data-plane IR slice. MIR 04 makes table/mask/capability facts concrete enough that rewrites can be verified, dumped, round-tripped, certified, and reported. Executable data-plane AArch64 loop codegen and table-loop fusion are intentionally MIR 05 work.

**Scope:**

- Checked `Table[T,N]` and `Mask[table,N]` subset, including parameter-scoped mask provenance.
- MIR table provenance, mask provenance, row-token facts, and verifier rules for mismatched masks and escaping row tokens.
- Authority query V0 for `same_table`, `same_mask_domain`, `same_row_count`, and `effect_absent`.
- Certified mask boolean identity rewrite guarded by identical `(table, rows)` provenance.
- First target cost seeds and branch/select conversion rules.
- IR-level rewrite reports and certificates for mask algebra and branch/select.
- `wrela perf code` returns a clear unsupported generated-code result for the data-plane fixture until MIR 05 adds loop codegen.

**Acceptance criteria:**

- The data-plane fixture checks, lowers to MIR, verifies, round-trips through W-MIR text, and optimizes at the IR level.
- Each MIR 04 data-plane rewrite has stable pass-control wiring, certificate output, rewrite-count/detail reporting, authority query evidence, and verifier evidence.
- `wrela perf code --mode release fixtures/perf/filter_sum.wrela` exits `2` with a data-plane codegen unsupported message rather than producing bogus generated-code numbers.

**Out of scope:** table loop fusion, data-plane AArch64 loop codegen, parameterized generated-code runtime A/B evidence, persistent optimization memory, slotted e-classes, offline SMT validation, SVE2 lowering, image-graph PGO, incremental cache, and broad database-style query optimization.

### MIR 05 — Certified optimization memory

The first durability and high-impact optimization phase. MIR 05 turns certified rewrites into a research-grade optimizer foundation by adding stable identity V1, a real authority-algebra engine, certificate versioning, hardened generated-code measurement, bounded persistent ledger storage, auto-reduction, failure containment, and measured cost feedback. Its flagship rewrite is certified table-loop fusion with parameterized data-plane generated-code A/B evidence.

**Scope:**

- Stable identity V1 for region/op/value/fact/pass-config/target-profile and before/after MIR snapshots.
- Authority algebra engine queries for disjoint capabilities, same table, independent ordering domain, effect absence, trap-order preservation, and state-edge preservation.
- Certificate schema versioning with explicit invalidation for stale certificates and stale ledger entries.
- Parameterized generated-code harness for the data-plane ABI used by `fixtures/perf/filter_sum.wrela`.
- Certified table-loop fusion using row-token, mask/table provenance, effect, state-edge, trap-order, and ordering-domain facts.
- Measurement hardening: parameter sweeps, target profiles, noise classification, checksum discipline, code-size/runtime/compile-time tradeoff reporting, and inconclusive-result handling.
- Bounded persistent ledger keyed by `region_hash + target_profile + pass_config + compiler_version`, with corruption handling, eviction, and statuses `known-winner`, `known-loser`, and `needs-remeasure`.
- Debug/why commands that render certificate facts and before/after hashes for selected rewrites.
- Auto-reducer for rewrite verifier failures, checksum mismatches, measurement sanity failures, and Trident infrastructure failures.
- Cost-model feedback loop where a stable measured ledger result can override static estimates for matching region shapes and target profiles.

**Acceptance criteria:**

- Table-loop fusion emits a valid certificate containing authority-algebra facts and verifier evidence.
- `wrela perf code --ab-pass table-loop-fusion --repeat 7 fixtures/perf/filter_sum.wrela` executes baseline and candidate generated code, checks matching checksums, classifies measurement stability, and records a bounded ledger entry.
- Corrupt, stale-version, or mismatched-ledger entries are ignored with a structured report and never trusted.
- A deliberately invalid rewrite candidate produces a reducer artifact under `target/wrela/reductions/` and leaves the optimized module unchanged.
- Ledger-informed extraction can choose a known winner for a matching region/target/pass key and reports the reason.

**Out of scope:** whole-program e-graph saturation, ML-based schedule search, SVE2 lowering, offline SMT validation, cross-image cloud profiles, and unbounded optimizer databases.

### MIR 06+ — Research extensions

Each item below is its own design + implementation plan, sequenced based on what MIR 03 and MIR 04 measurement reveal as the highest-leverage next investment.

- **Slotted e-classes** — for row-iteration and binder-aware rewrites. Sequenced if MIR 04 measurement shows row-iteration rewrites are blocked by binder issues.
- **Offline SMT rule validation** — Z3-based rule prover that runs at compiler build time and produces a checked-in rule table or proof notes. Compiler does not link Z3.
- **Additional microarch cost tables** — Neoverse N2/V2, Apple Avalanche-class, Cortex-A720, Cortex-A55.
- **SVE2 codegen** — for Neoverse-V2 / N2 / A720+ targets when bench data justifies.
- **Image-graph PGO** — using the static image graph as a hotness signal for cost-driven extraction and CSEL-vs-branch decisions.
- **Per-λ content-addressed incremental cache** — Salsa-style, keyed by `(λ_hash, effect_set, capability_path_set)`.
- **Parallel optimization** — region-level parallel rewriting using Rayon, with deterministic-output guarantees.
- **Mask-algebra rule expansion** — Hydra/Souper-style rule synthesis on observed Wrela code.
- **Data-plane AArch64 loop-code expansion** — more table/mask kernels after MIR 05's scalar data-plane loop path proves the ABI and harness.

## Non-goals

- **Whole-program e-graph saturation.** The release optimizer is region-local aegraph overlays. We don't saturate across functions or across the whole module.
- **Pure RVSDG academic conventions.** We use the structural ideas (regions, state edges, dataflow boundaries) without binding to a particular RVSDG paper's encoding.
- **Generality across non-AArch64 targets.** W-MIR is AArch64-only. The IR can assume AArch64 calling convention, memory ordering, and register families.
- **A general MLIR-style multi-dialect infrastructure.** W-MIR is one IR. Dialect frameworks are not added until a concrete need demonstrates value.
- **Speculative + rollback rewriting.** Aegraph saturation with bounded budget subsumes this.
- **Heavy auto-tuning / search-based optimization.** Compile budget forbids long
  search loops. Lightweight generated-code A/B benchmarking is required for
  codegen-supported pass families, but it measures fixed pass sets rather than
  searching large schedule spaces.
- **Effect rows.** Wrela's effect bitsets are intentionally coarser than Koka-style algebraic effect rows. Bitsets are O(1) to union and gate rules; that simplicity is the feature.

## Open questions

These need answers, but they don't block MIR 01.

- **Exact textual dump format.** Round-trip parseability is the constraint; specific syntax is open. Proposal during MIR 01 prototyping.
- **Cost-model table expansion.** MIR 04 uses handwritten Rust cost seeds. Future multi-microarch expansion may move those seeds into a line-oriented text table if Rust source becomes unwieldy.
- **Perf JSON schema.** The schema must cover compile telemetry and generated-code
  A/B results without becoming a second compiler data model. MIR 02 should lock
  the first version once `wrela perf compile` and `wrela perf code` exist.
- **Mask-algebra rule expansion.** MIR 04 locks the first identity family. Additional rules are future vertical slices that must obey Trident certificates and measurement/reducer gates.
- **State-token granularity.** How fine-grained should state-edge typing be? Per-table is obviously right; per-MMIO-device is obviously right; per-arena and per-frame are open. Trade-off between expressiveness and verifier complexity.
- **Frame-handle escape rules.** The interaction between `δ` lifetime and `return` from nested `λ` regions needs a clear rule. Likely: frame-handle types carry a δ tag; values with the tag cannot appear in the output set of a region that doesn't contain the δ.
- **`Place` aliasing.** When two `Place` values are derived from the same root with different paths, what aliasing analysis applies? The capability-path discipline gives us a starting point; the formalization is open.
- **Row-token formalization.** The "row tokens cannot escape" rule is enforced by the verifier today; a small operational semantics for row tokens would let us prove fusion rewrites unconditionally safe. Sequenced as a research extension, not blocking.

## Acceptance for this spec

This document is the W-MIR design. It is approved when:

- The Regioned Effect SSA shape is committed.
- The dev/release pipeline split is committed.
- The phased delivery (MIR 01 / MIR 02 / MIR 03 / MIR 04 / MIR 05 / MIR 06+) and per-phase acceptance criteria are committed.
- The day-one debugging contract (per-pass verifier, deterministic textual dump, stable IDs) is committed.
- The first-class measurement contract for compiler telemetry and generated-code
  A/B benchmarks is committed.

Implementation plans for MIR 01, MIR 02, MIR 03, MIR 04, MIR 05, and the research extensions are separate documents under [`../implementation/plans/`](../implementation/plans/), following the wrela plan-template.
