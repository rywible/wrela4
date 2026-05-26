# Wrela Optimizer Harness Design

Date: 2026-05-25

## Purpose

This document specifies the **Wrela Optimizer Harness** — the authoring SDK,
pass registry, dispatch model, run-trace primitive, ledger semantics,
telemetry surface, and tooling that make adding a compiler optimization to
Wrela a small, bounded, repeatable operation.

The harness sits atop W-MIR's Regioned Effect SSA shape and the Trident
doctrine (certificates, authority algebra, bounded ledger, reducer,
fail-closed infrastructure) introduced across MIR 01-05. MIR 01-05 prove that
**one** rewrite can be provable, measured, and recorded. The harness phases
(MIR 06-18) scale that contract from local transforms to an optimization lab:
dozens of passes, hundreds of transforms, whole-program facts, multi-region
transactions, schedule search, online-learning proposal, and emitted-code
assumption analysis without sinking under the weight of maintaining and
proofing each one.

The shipping bet: after MIR 05 lands, the next bottleneck is **author cost
per optimization**, not IR capability. The harness reduces that cost to one
transform definition plus one registry line — with the certificate, ledger
key, telemetry counter, pass-control flag, fixture binding, fuel budget,
coverage entry, and `wrela why` rendering all produced automatically by the
substrate.

### Relationship to other documents

- IR shape and Trident doctrine:
  [`2026-05-25-wrela-regioned-effect-ssa-mir-design.md`](2026-05-25-wrela-regioned-effect-ssa-mir-design.md).
  The harness consumes W-MIR's stable hashes, certificate schema, authority
  algebra engine, ledger, and reducer. It does not redefine them.
- Parent MIR roadmap:
  [`../implementation/plans/2026-05-25-wrela-regioned-effect-ssa-mir.md`](../implementation/plans/2026-05-25-wrela-regioned-effect-ssa-mir.md).
  The harness phases (MIR 06-18) are downstream of MIR 05 and do not modify
  the in-flight MIR 01-05 plans.
- Compiler principles: [`../design-principles.md`](../design-principles.md).
  Zero external dependencies, parallel immutable phases, diagnostics as
  data, root-driven reachability, handwritten arena-allocated infrastructure.

### Scope

This document specifies:

- The Transform model — `Metadata`, `Eligibility`, `CandidateSource`,
  `LegalityContract`, `Apply` — and its match-local and region-local
  scopes.
- The erasure model that lets the registry store transforms uniformly as
  `dyn` despite their associated types.
- The pass registry as the single source of truth for every optimization.
- The per-region fingerprint substrate with `any` / `all` / `forbidden`
  masks and the sound group-level eligibility check.
- Pre-classification purity gating so the aegraph only ingests ops whose
  purity is provable before saturation begins.
- The transactional dispatch model: every pass writes into a scratch
  builder and commits only after verifier, budget, certificate, and event
  emission succeed.
- **`RunTrace`** as the single append-only event log that all of ledger,
  coverage, attribution, and `wrela why` are projections of.
- The fused execution model that routes pure transforms to the aegraph
  and impure transforms to a shared worklist per pass group.
- Bidirectional ledger semantics (`known-winner`, `known-loser`,
  `needs-remeasure`, `known-no-op`) with explicit revalidation discipline,
  realized as a projection over `RunTrace`.
- Rule health states (Experimental → Observed → Candidate → Release →
  Retired) with full Trident retained across every state.
- The `wrela why` and `wrela pass scaffold` product surfaces.
- Deterministic fuel-based budget enforcement with wall-clock kept as
  telemetry only.
- Bounded delta-debugging (ddmin) for auto-bisect composed with the MIR
  05 reducer.
- Matcher determinism, fact serialization, parallel-execution stance, flag
  precedence, and ledger transition rules.
- The conceptual decomposition of `RunTrace` into Inputs, Facts,
  Decisions, Edits, and Evidence — a mental model for completeness
  checking and a forcing function for evaluating new event kinds.
- The content-addressed input blob store under
  `target/wrela/blobs/<hash>.bin` that holds pass configs, target
  profiles, experiment specs, lab suites, schedules, cost models,
  search policies, target-cost models, and benchmark corpora.
- The reserved event variants (`RegionFeatureSummary`,
  `ExperimentStarted` / `Finished`, `ScheduleDecision`,
  `CandidateConsidered` / `Scored`, `WorldFactQuery`,
  `MultiRegionTransaction*`, `LabSuiteStarted` / `Finished`,
  `SearchPolicyUpdated`, `TargetCostEstimated`,
  `OptimizationClaimRecorded`, `AssumptionDeltaRecorded`) that ship in
  the closed enum at MIR 06 so MIR 10-18 lab work composes without
  schema bumps.
- The experiment runner, immutable trace corpus, `ExperimentSpec`, and
  measurement policy that turn compiler runs into replayable experiments.
- `WorldSummary` as the read-only whole-program fact layer.
- `RegionSetPattern` and multi-region transactions for cross-region
  optimization with boundary verification.
- Inlining/specialization and cross-region data-plane fusion as the first
  multi-region optimization families.
- Schedule-as-data, automated search, the optimization lab exerciser, and
  the first online-learning proposer (`LinUCB`).
- `TargetCostTrace`, `OptimizationClaim`, and `AssumptionDelta` as the
  emitted-code feedback layer that compares predicted work to measured
  hardware behavior.
- The phased delivery across MIR 06 through MIR 18 with per-phase
  acceptance criteria.

This document does **not** specify:

- The exact pattern-matching syntax (deferred to MIR 06 implementation;
  this design commits the matcher's determinism invariants).
- The exact fingerprint bit layout (deferred to MIR 07 implementation;
  reserved bits committed here).
- The exact fuel quantum per transform category (deferred to MIR 06
  measurement).
- The exact `ExperimentSpec`, `WorldSummary`, `Schedule`, lab-suite, and
  LinUCB on-disk serialization syntax (deferred to their implementation
  phases; this design commits the identities, boundaries, and invariants).
- The full catalog of future optimization families beyond the first
  inlining/specialization and cross-region data-plane fusion passes.

## Principles

These are the durable commitments. Specific trait shapes, fingerprint
layouts, and ledger schemas may evolve; these should not.

- **Make transforms boring to add.** The author writes the optimization.
  The harness writes the certificate, the ledger key, the telemetry, the
  pass-control flag, the fixture binding, the fuel budget, and the
  why-output rendering. The invariant is **two discovery touch points**:
  the pass module and the registry. Other supporting files (fixtures,
  fact types, helper modules) are referenced from those two, never
  discovered independently.
- **Every fact queryable, content-addressed, produced exactly once.** The
  verifier produces facts. Transforms consume facts. Certificates record
  facts. The ledger remembers facts. No fact has two sources of truth.
- **`RunTrace` is the primary event log; everything else is a
  projection.** The ledger, coverage, attribution, and `wrela why` are
  derived from the same append-only trace. They cannot disagree because
  they share their input. Schema evolution is a change in projection
  logic, not in storage.
- **Full Trident from line one.** Every Transform — including
  experimental ones — declares required facts, emits a versioned
  certificate on application, and records a bounded event on every
  dispatch. There is no provability bypass tier. Rule health states
  relax ledger persistence, fuel defaults, and coverage expectations;
  they never relax proof.
- **Transactional dispatch.** Every pass writes into a scratch region
  builder. The harness commits to a new region only after verifier passes,
  fuel budgets hold, certificates emit, and the trace event flushes. Any
  failure abandons the scratch region; the previous region remains
  authoritative. Fail-closed becomes structural rather than aspirational.
- **Behavior-preserving retrofit.** The first harness phase migrates the
  existing MIR 03/04/05 passes into the new model without changing their
  observable output. MIR 05's golden text and benchmark checksums hold
  byte-for-byte after MIR 06 lands.
- **Schema-stable trace format.** The `RunTrace` event schema lands in
  MIR 06 with reserved fields for fingerprints, fuel, attribution, and
  bisect metadata that later phases will populate. Schema changes after
  MIR 06 invalidate all traces emitted before the change.
- **Deterministic budgets.** Per-transform and per-pass budgets are
  denominated in **fuel** (pattern match attempts, worklist iterations,
  fact queries, mutations) — not wall-clock time. Fuel is reproducible
  across machines and across runs. Wall-clock time is recorded as
  telemetry and used for advisory warnings, but never as a hard build
  gate inside the ordinary quality gate.
- **Self-revalidating ledger.** A ledger that always skips can never
  discover that the skip became wrong. The harness supports explicit
  `--remeasure`, quality-gate strict mode that bypasses ledger skips,
  and deterministic probabilistic revalidation so cache rot becomes
  visible rather than silent.
- **Region-local first, region-set later.** MIR 06-09 prove the harness on
  one region at a time. MIR 10-18 do not abandon regions; they compose them.
  Cross-region optimization is expressed as explicit region-set patterns and
  multi-region transactions with boundary verification, never as ambient
  whole-module mutation.
- **Deterministic ordering even as scope grows.** Events within one region
  are totally ordered. Multi-region transactions order touched regions by
  stable region ID before emitting events. Across independent regions,
  ordering may be partial internally but projection-visible output is
  deterministic.
- **Models propose; the harness judges.** Automated search and LinUCB
  online learning may propose schedules, thresholds, and knob settings, but
  no proposal bypasses certificates, verifier checks, checksums, fuel
  budgets, measurement gates, or replayable trace evidence. Learning happens
  in the lab; production defaults change only through explicit promotion.
- **Cost models are hypotheses, not truth.** Target-side execution cost
  counters reduce dependence on noisy wall-clock results, but they do not
  replace hardware measurement. When modeled work and runtime disagree, the
  harness records the mismatch as an assumption delta instead of hiding it.
- **Accountability, not correctness.** The harness makes every
  experiment visible, not every experiment right. Authors remain
  responsible for the soundness of their transforms. The harness
  ensures that wrong things become observable, that decisions are
  recorded, and that nothing accumulates as folklore. Correctness
  flows from full Trident; the harness's job is to make sure the
  Trident evidence exists, is queryable, and is reproducible.
- **Zero external dependencies.** Std-only. No `inventory`, no `linkme`,
  no proc-macro crates beyond what the workspace already uses. Pass
  registration is manual; a CI lint enforces that every pass module
  appears in the registry.

## The harness shape

### The Transform model

A `Transform` is the harness's uniform unit of optimization. Authors
choose a **scope** for their transform (match-local or region-local) and
the harness sees one dispatch / certificate / event / fuel / why contract
for both. The Transform decomposes into five concerns:

| Concern             | What it owns                                                       |
|---------------------|--------------------------------------------------------------------|
| `Metadata`          | name, version, group, category, fuel budget, fixtures, health state |
| `Eligibility`       | `FingerprintRequirement` with `any` / `all` / `forbidden` masks    |
| `CandidateSource`   | how to enumerate candidate sites (pattern matching vs region walk) |
| `LegalityContract`  | required facts + certificate schema + verifier expectations        |
| `Apply`             | produce the new region via a scratch builder; transactional commit |

This separation is deliberate. The current Rust trait shape can live as
one trait with associated types or as five small sub-traits composed via
generics; the exact syntax is MIR 06 implementation. The architectural
commitment is that these five concerns are *separate*, declared
independently by the author, and consumed independently by the harness.

#### Transform scopes

Two scopes cover the optimization space:

- **`MatchLocal`** — formerly `Rule`. The transform's `CandidateSource`
  is a pattern matcher; each candidate is a small subgraph match within
  a region. Examples: scalar identity, mask boolean identity,
  branch/select conversion. Pure `MatchLocal` transforms route to the
  aegraph; impure ones run on the worklist.
- **`RegionLocal`** — formerly `StructuralPass`. The transform's
  `CandidateSource` is a region traversal; each candidate is "the whole
  region with some structural property." Examples: single-region
  inlining (callee body lifted into one caller region),
  devirtualization, region-internal loop fusion, table-loop fusion.
  Always runs on the worklist; never routes to the aegraph.

```rust
// Author-facing sketch. Exact syntax deferred to MIR 06.

pub trait Transform: 'static + Sync {
    type Scope: TransformScope;          // MatchLocal | RegionLocal
    type Candidate;                       // Match or Region wrapper
    type Facts: RequiredFacts;

    const METADATA:     TransformMetadata;
    const ELIGIBILITY:  FingerprintRequirement;

    fn enumerate_candidates(
        &self,
        region: &Region,
        ctx: &EnumerateCtx<'_>,
    ) -> CandidateIter<Self::Candidate>;

    fn legality(
        &self,
        candidate: &Self::Candidate,
        ctx: &FactCtx<'_>,
    ) -> Self::Facts;

    fn apply(
        &self,
        candidate: &Self::Candidate,
        builder: &mut Builder<'_>,
    ) -> ApplyResult;
}
```

`TransformMetadata` carries name, version, group, category, fuel budget,
fixtures, and health state as `const` data so the registry can read it
without instantiating the transform.

Authors do not implement `Transform` directly except in rare cases.
Helper trait aliases (`MatchLocalTransform`, `RegionLocalTransform`) and
optional macros simplify the common cases to a small declarative block.

#### `Purity` and pre-classification

Purity is a property of a candidate, not of the Transform. A
`MatchLocal` transform declares its expected purity in metadata:

- **Statically pure**: the SDK rejects candidates whose root op category
  carries effects, state edges, trap producers, or capacity tokens at
  compiler-build-time.
- **Pure pending fact check**: per-candidate fact lookups certify
  non-trapping and effect absence for the specific op instance.
- **Impure**: candidate always runs on the worklist.

Critically, **per-candidate purity facts are checked before the candidate
is admitted to the aegraph**, not during saturation. Saturation operates
only on certified-pure subgraphs. This closes the soundness gap where an
op category looks pure but an instance may trap.

The harness performs pre-classification as a separate dispatch phase:

```text
phase 1: enumerate candidates from all eligible transforms in group
phase 2: classify each candidate (statically pure / pending / impure)
phase 3: for pending candidates, run fact checks → pure or rejected
phase 4: pure candidates → aegraph build; impure → worklist
phase 5: saturation (aegraph) + worklist dispatch
phase 6: scoped extraction back to W-MIR through the transactional builder
phase 7: verifier on the scratch region; commit or abandon
```

Fact-check cost in phase 3 is amortized: results are cached per `(op
identity, fact kind)` for the rest of the dispatch.

#### Erasure model

`Transform` has associated types (`Scope`, `Candidate`, `Facts`) and
const metadata, which makes it not directly object-safe. The registry
stores transforms as `&'static dyn TransformVtable`, an object-safe
companion trait built by blanket-impl from `Transform`:

```rust
// Sketch. Exact vtable layout is MIR 06 implementation.

pub trait TransformVtable: Sync {
    fn metadata(&self) -> &'static TransformMetadata;
    fn eligibility(&self) -> FingerprintRequirement;
    fn scope_kind(&self) -> ScopeKind;          // MatchLocal | RegionLocal

    fn enumerate_erased(
        &self,
        region: &Region,
        ctx: &EnumerateCtx<'_>,
    ) -> ErasedCandidateIter;

    fn legality_erased(
        &self,
        candidate: &ErasedCandidate,
        ctx: &FactCtx<'_>,
    ) -> ErasedFacts;

    fn apply_erased(
        &self,
        candidate: &ErasedCandidate,
        builder: &mut Builder<'_>,
    ) -> ApplyResult;
}

impl<T: Transform> TransformVtable for T {
    // boilerplate that boxes candidates and facts into erased types
    // ...
}
```

`ErasedCandidate` and `ErasedFacts` are arena-allocated tagged unions
sized to the largest variant the registry contains, set at
compiler-build-time from the registered transforms. Author code never
touches the erased forms.

### The pass registry

A single `&[&'static dyn PassDescriptor]` is the source of truth for
every optimization the compiler knows about. Because the workspace
forbids external crates like `inventory` or `linkme`, registration is
**manual**:

```rust
// src/mir/passes/registry.rs
pub const PASSES: &[&'static dyn PassDescriptor] = &[
    &super::scalar_identity::PASS,
    &super::mask_algebra::PASS,
    &super::branch_select::PASS,
    &super::table_loop_fusion::PASS,
    // new passes appended here
];
```

A CI lint walks `src/mir/passes/*.rs`, identifies every `pub static PASS`
declaration, and confirms each appears exactly once in `PASSES`. The
lint catches the forgotten one.

A `Pass` is the user-facing toggle: a named bundle of one or more
transforms that share a category and a stable identity for pass-control
flags. A single-transform pass is common (`table-loop-fusion` is one
transform); a multi-transform pass is also common (`scalar-identity`
bundles a dozen related rewrites).

```rust
pub trait Pass: 'static + Sync {
    const NAME: &'static str;
    const GROUP: PassGroup;
    const DEFAULT_ENABLED: ModeSet;
    fn transforms() -> &'static [&'static dyn TransformVtable];
}
```

The registry is the *only* place that knows what passes exist. Every
other consumer reads from it:

- **CLI flag parsing.** `--enable-pass`, `--disable-pass`, `--only-pass`
  resolve names against the registry. There is no hardcoded list of
  pass names anywhere else.
- **Default pipeline.** Dev and release pipelines are derived from
  `PASSES.iter().filter(|p| p.default_enabled(mode))` ordered by
  `PassGroup`.
- **Ledger keys.** The pass-name component of a ledger key comes from
  the registry's pass descriptor, never from a string literal at a
  transform site.
- **Auto-generated docs.** A build artifact at `target/wrela/passes.md`
  enumerates every pass with its group, category, transforms, fact
  dependencies, fuel budget, health state, and exercising fixtures,
  generated from registry metadata.
- **`wrela passes`.** Human-readable registry dump.
- **`wrela passes coverage`.** Which passes fired in the last run.
- **`wrela pass scaffold <name>`.** Adds a new pass module skeleton and
  registry entry; see the [Authoring tools](#authoring-tools) section.

The descriptor trait is small — just enough to introspect uniformly:

```rust
pub trait PassDescriptor: Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> u16;
    fn group(&self) -> PassGroup;
    fn default_enabled(&self, mode: Mode) -> bool;
    fn transforms(&self) -> &'static [&'static dyn TransformVtable];
    fn fixtures(&self) -> &'static [&'static str];
}
```

#### `PassGroup`

A `PassGroup` is a phase. The unit at which transforms are fused into
one IR walk. A fixed enum, in a fixed order. Authors do not invent new
groups.

```rust
pub enum PassGroup {
    Canonicalize,    // const fold, copy prop, trivial peeps, region simplification
    Algebraic,       // scalar / mask identities, algebraic fold-through
    DataPlane,       // table / mask / row-token rewrites
    LoweringPrep,    // branch-vs-select, region cleanup, lowering-friendly shapes
    Structural,      // inlining, devirt, region-internal loop fusion
    Experimental,    // gated, longer fuel budgets, no persistent ledger writes
}
```

The release-mode pipeline walks groups in declaration order per region.
Each group's transforms are fused into a single dispatch.

The `Experimental` group is the only relaxation: transforms there still
emit certificates and use the SDK, but the persistent ledger does not
write entries for them and their fuel budgets default to larger values.
Promotion out of `Experimental` is a deliberate change to the
transform's `GROUP` const and a health-state transition.

### Rule health states

Transforms carry a `HealthState` in their metadata. The state affects
ledger persistence, fuel defaults, default-enabled posture, and coverage
expectations — but **not** provability. Every state retains full Trident.

| State          | Ledger persistence    | Fuel defaults | Default enabled | Coverage expectation |
|----------------|------------------------|----------------|------------------|------------------------|
| `Experimental` | in-memory trace only  | 4x base       | off              | may fire             |
| `Observed`    | persistent, read-only | 2x base       | off              | should fire on ≥1 fixture |
| `Candidate`    | persistent, full      | 1.5x base     | release only     | must fire on declared fixtures |
| `Release`      | persistent, full      | 1x base       | release default-on | must fire on declared fixtures; A/B win required |
| `Retired`      | persistent, frozen    | n/a           | always off       | n/a                  |

State transitions are deliberate, recorded in the registry alongside the
transform's version. Downgrading (e.g., `Release` → `Candidate`) on
discovered regression is recorded with a reason and emits a
`HealthStateChanged` event on next compile.

`Retired` transforms remain in the registry so historical traces and
ledger entries remain interpretable; they are never dispatched.

### Fingerprints

A `Fingerprint` is a 64-bit value computed per region that summarizes
"what kinds of things live here" cheaply enough to gate every dispatch
decision. Fingerprints are the cheap, deterministic filter that makes
adding the 101st transform cost zero on regions that transform cannot
fire against.

```rust
pub struct Fingerprint(u64);
```

The bit layout is finalized in MIR 07; this design commits the
partitioning, the budget per partition, and a generous reserve.

| Bits   | Partition                                         |
|--------|---------------------------------------------------|
| 0-15   | Op-category presence (16 bits)                    |
| 16-26  | Effect-set presence (mirrors Wrela's 11 effects)  |
| 27-29  | Region-kind bits (3 bits)                         |
| 30-37  | Type-presence bits (8 bits)                       |
| 38-40  | Op-count bin (3 bits)                             |
| 41-63  | Reserved (23 bits)                                |

The reserved bits are critical. Adding a new fingerprint category later
without a schema revision and ledger invalidation requires that the bit
was reserved, not invented. Lock the partitioning at MIR 07; treat
unreserved expansions as schema-version bumps.

Each Transform declares its fingerprint requirement as a constant with
three masks:

```rust
pub struct FingerprintRequirement {
    pub required_any: FingerprintMask,    // at least one of these bits set
    pub required_all: FingerprintMask,    // all of these bits set
    pub forbidden:    FingerprintMask,    // none of these bits set
}

fn eligible(fp: Fingerprint, req: FingerprintRequirement) -> bool {
    fp & req.forbidden == FingerprintMask::EMPTY
        && fp & req.required_all == req.required_all
        && (req.required_any == FingerprintMask::EMPTY
            || fp & req.required_any != FingerprintMask::EMPTY)
}
```

#### Sound group-level eligibility

The group-level skip cannot be computed as a single merged mask. A
`forbidden` mask on transform A and a `required_all` mask on transform B
can overlap on the same bit, making any merged-mask approximation
unsound — a merged check would skip the group on regions where B should
fire.

The correct group-level check is **"does any transform in this group
satisfy `eligible(fp, req)`?"**:

```rust
fn group_eligible(fp: Fingerprint, group: PassGroup) -> bool {
    group_transforms(group).any(|t| eligible(fp, t.eligibility()))
}
```

Two bounded fast paths:

- **Empty `required_any` short-circuit.** If every transform in the
  group has empty `required_any`, the `required_all` and `forbidden`
  checks alone determine eligibility, and the group check is one pass
  over transforms with no early-exit win.
- **Common-requirement bucket.** Transforms with identical
  `FingerprintRequirement` values can be deduplicated; the group check
  evaluates each unique requirement once. With ~12 transforms per group
  and ~3-5 unique requirement shapes, the per-region cost is small.

The naive iteration is the correctness baseline. The buckets are the
optimization. Neither is the merged-mask form, which is unsound and not
shipped.

Fingerprints are computed at region construction and recomputed when a
pass mutates the region (transactional dispatch makes this concrete:
the new region carries its own fingerprint, computed during commit).
Recomputation is O(ops in region) and is cheap relative to a region
walk.

### `RunTrace`: the primary event log

`RunTrace` is the architectural primitive that makes the rest of the
harness consistent. One append-only structured record per compiler run
contains every event the harness produces. The ledger, coverage,
attribution, `wrela why`, and auto-bisect are all **projections** of the
trace, not parallel stores.

This means: the ledger and coverage cannot disagree about what fired,
because they read from the same input. Schema evolution is a change in
projection logic, not in storage. Replay is trivial. Determinism is a
property of the trace; downstream surfaces inherit it.

#### Conceptual decomposition

`RunTrace` content separates into five categories. Every event the
harness emits and every input the harness consumes slots into exactly
one. The categorization is not enforced in the trace structure (it is
all one event stream) but is the mental model used to verify coverage
at a glance and to evaluate new proposed event kinds.

| Category   | What it captures                                                | Examples |
|------------|------------------------------------------------------------------|----------|
| Inputs     | hashed pointers to everything the run depends on                | compiler version, target profile, pass config, schedule, cost model, benchmark corpus |
| Facts      | what the harness learned about the program                      | candidates found, purity outcomes, fact-check results, region feature summaries, world-fact queries |
| Decisions  | what the harness chose to do or not do, with reasons            | group/pass skips, transform rejections, schedule decisions, ledger projections, health-state changes |
| Edits      | what the harness actually changed in the IR                     | transaction commits, transaction abandons, multi-region transactions |
| Evidence   | proof artifacts that justify or invalidate downstream claims    | applied certificates, fuel consumption, target-cost estimates, assumption deltas, verifier outcomes, wall-clock samples |

When a future event kind is proposed, the question "which category
does it slot into?" surfaces missing categories before schema lock and
forces consistency between authored and lab-generated events.

#### Trace shape

```text
RunTrace {
    run_id:                StableHash         # content hash of run inputs
    compiler_version:      u64
    target_profile_hash:   StableHash         # ref into blob store
    pass_config_hash:      StableHash         # ref into blob store
    experiment_spec_hash:  Option<StableHash> # ref into blob store (reserved; MIR 10)
    lab_suite_hash:        Option<StableHash> # ref into blob store (reserved; MIR 16)
    schedule_hash:         StableHash         # ref into blob store (reserved; MIR 15)
    cost_model_hash:       StableHash         # ref into blob store
    search_policy_hash:    Option<StableHash> # ref into blob store (reserved; MIR 17)
    target_cost_model_hash: Option<StableHash> # ref into blob store (reserved; MIR 18)
    benchmark_corpus_hash: Option<StableHash> # ref into blob store (perf runs)
    telemetry_hash:        Option<StableHash> # advisory sidecar, excluded from run_id
    events:                Vec<Event>         # append-only, ordered
    truncated:             bool               # set if event budget exceeded
}
```

Events are tagged structs. The MIR 06 schema includes:

```text
Event variants (locked at MIR 06; reserved variants populated later phases)

# Active in MIR 06-09 — emitted by dispatch and consumed by projections.

GroupDispatchStarted    { group, region_id, fingerprint, ts }
GroupSkipped            { group, region_id, reason: SkipReason, ts }
PassDispatchStarted     { pass_name, region_id, ts }
PassSkipped             { pass_name, region_id, reason: SkipReason, ts }
TransformCandidateFound { transform, region_id, candidate_id, ts }
TransformPurityChecked  { transform, candidate_id, outcome: PurityOutcome }
TransformFactsChecked   { transform, candidate_id, facts: FactBundle }
TransformApplied        { transform, candidate_id, certificate: Certificate }
TransformRejected       { transform, candidate_id, reason: RejectReason }
FuelConsumed            { pass_name, transform?, fuel_kind, amount }
FuelExceeded            { pass_name, transform?, fuel_kind, budget, consumed }
VerifierPassed          { region_id, after_pass }
VerifierFailed          { region_id, after_pass, evidence: VerifierEvidence }
TransactionCommitted    { region_id, before_hash, after_hash }
TransactionAbandoned    { region_id, reason: AbandonReason }
LedgerProjectionEntry   { key, status, source_events: [EventId] }
LedgerViolation         { key, expected_status, observed_status }
HealthStateChanged      { transform, from, to, reason }
WallClockSample         { phase, duration }   # advisory; sidecar/projection only

# Reserved at MIR 06 for MIR 10-18 lab work. The variants exist in
# the closed enum and serialize correctly. MIR 06-09 dispatch code
# never emits them except where noted.

RegionFeatureSummary    { region_id, features: FeatureVector }
ExperimentStarted       { experiment_id, hypothesis_hash, config_hash }
ExperimentFinished      { experiment_id, outcome: ExperimentOutcome }
ScheduleDecision        { scope: ScheduleScope, schedule_hash, reason: ScheduleReason }
CandidateConsidered     { candidate_id, kind: CandidateKind, scope: CandidateScope, proposer: Proposer }
CandidateScored         { candidate_id, score: CostScore, model_hash }
WorldFactQuery          { fact_kind, regions: [RegionId], result: WorldFactResult }
MultiRegionTransactionStarted   { transaction_id, regions: [RegionId] }
MultiRegionTransactionCommitted { transaction_id, before_hashes, after_hashes }
MultiRegionTransactionAbandoned { transaction_id, reason: AbandonReason }
LabSuiteStarted        { suite_id, suite_hash, corpus_hash }
LabSuiteFinished       { suite_id, outcome: LabSuiteOutcome }
SearchPolicyUpdated    { policy_hash, from_trace_ids: [RunId], reason }
TargetCostEstimated    { scope: CostScope, estimate: TargetCostTrace }
OptimizationClaimRecorded { claim_id, expected: OptimizationClaim }
AssumptionDeltaRecorded   { claim_id, delta: AssumptionDelta }
```

Each event carries an `EventId` (monotonic within the trace) for
cross-reference. The schema is **locked at MIR 06**; later phases
populate reserved variants without bumping the schema version. Active
variants ship with their fields filled; reserved variants exist in the
enum and serialize correctly but are never emitted by MIR 06-09
dispatch code — they wait for the lab phases (MIR 10-18).

`RegionFeatureSummary` is the one reserved variant that MIR 06 emits
a minimal version of: a `FeatureVector` of at most 20 `u32` counters
covering op-category counts, region depth, max fan-in / fan-out, loop
nesting depth, and effect richness, computed at region construction
and after every committed mutation. The feature schema is
deliberately sparse so future feature IDs add entries without schema
bump. No MIR 06-09 projection consumes the summary; the storage is
training data for future ML work, accumulating from day one rather
than starting at zero.

#### Bounded storage

`RunTrace` is bounded: events accumulate to a configurable budget (per
run, per region), and when the budget is reached the trace is marked
`truncated: true` and further events for that region are dropped. The
budget defaults are loose enough that normal compiles don't truncate;
pathological cases (an experimental transform firing 10K times) do.

Per-run traces are stored under `target/wrela/traces/<run_id>.bin` with
a retention policy (configurable; default last 50 runs). The persistent
ledger is a longer-lived rollup derived from many traces.

#### Content-addressed input blob store

Pass config, target profile, experiment spec, lab suite, schedule,
cost model, search policy, target-cost model, and benchmark corpus are
stored once per unique content under
`target/wrela/blobs/<stable-hash>.bin`. Traces reference them by hash.
This makes every input to the optimizer reproducible by hash
reference, removes inline serialization burden when inputs grow rich,
keeps ledger keys stable across renames, and makes experiment replay a
hash lookup rather than an input reconstruction.

The blob store is content-addressed and write-once. Garbage collection
is by reachability: a blob is retained as long as any retained trace or
ledger entry references it. The collection policy is conservative and
runs at the same cadence as trace retention.

A trace that references a missing blob is treated as corrupt for
projection purposes — the projection emits a structured error and
ignores the trace, consistent with the fail-closed posture. Authors
never write to the blob store directly; the harness writes when it
serializes a run's inputs at dispatch entry, and the same content
always produces the same hash so re-writes are idempotent.

#### Projections

Each downstream surface is a projection function over the trace:

```text
ledger_projection(traces) -> Ledger
    For each (region_hash, target, pass_config, compiler_version, pass_name) key,
    scan TransformApplied / TransformCandidateFound / FuelExceeded / VerifierFailed
    events to compute status (known-winner | known-loser | needs-remeasure |
    known-no-op).

coverage_projection(trace) -> CoverageReport
    Per pass, count TransformCandidateFound / TransformApplied / TransformRejected.
    Per region, count GroupSkipped events with reason.

attribution_projection(trace) -> AttributionReport
    Per pass, sum FuelConsumed events.
    Per pass, sum WallClockSample events.
    Flag wall-clock vs fuel divergence beyond threshold.

why_projection(trace, query) -> WhyReport
    Filter events by query predicate (rule | pass | region | regression | skipped | cold).
    Render before/after MIR from TransactionCommitted hashes.

bisect_projection(baseline_trace, candidate_trace) -> RegressionCandidate
    Diff events to find pass-set delta between baseline and candidate.
```

Projections are deterministic functions of the canonical trace core.
Two runs that produce byte-identical canonical traces produce
byte-identical projections. Advisory telemetry sidecars can differ
between runs without changing the trace identity; projections that
consume wall-clock data opt into that sidecar explicitly. Changing a
projection's logic is a one-component change with no storage migration;
the old traces re-project under the new logic.

### Transactional dispatch

The harness commits to the immutable-artifact principle: every pass
produces a new region (sharing unchanged subtrees by reference). A pass
never mutates the region it was given.

Per region per pass group, dispatch proceeds:

```text
1. start a fresh trace event: GroupDispatchStarted

2. group fingerprint check (sound per-transform iteration with buckets)
   if no eligible transform → emit GroupSkipped, return original region

3. group ledger projection check (read from in-memory ledger derived
   from persisted ledger + current run's trace)
   if every transform in group is known-no-op or known-loser for this
   region key → emit GroupSkipped, return original region

4. open a scratch builder over the current region

5. partition eligible transforms by purity (pre-classification phase):
   - statically pure → aegraph candidate
   - pending → run fact check; promote to pure or reject
   - impure → worklist

6. if any pure candidates: build aegraph overlay over the scratch
   builder; register transforms; run bounded saturation; extract back
   to builder. emit TransformApplied per applied rewrite.

7. if any impure candidates: drive worklist over the scratch builder.
   for each candidate: legality check, then apply via builder.

8. while dispatching, every transform consumes fuel from a per-rule
   meter and a per-group meter. fuel exhaustion → FuelExceeded event,
   suspend that transform for the region.

9. when transforms complete or fuel exhausts:
   - verifier runs on the scratch region
   - if verifier passes → emit TransactionCommitted with before/after
     hashes; return the new region
   - if verifier fails → emit TransactionAbandoned with reason and
     verifier evidence; return the original region untouched
```

The scratch builder is the transaction. Failure at any point —
verifier, fuel, certificate generation, event flush — abandons the
scratch and preserves the prior region. Partial mutations cannot leak
into the next pass.

This makes the fail-closed guarantee structural. There is no codepath
where a half-applied rewrite leaves the IR in an intermediate state.

### Fused execution

Within a single pass-group transaction, transforms of the same purity
class share machinery:

- **Pure transforms** all register against the same region-local
  aegraph overlay. Saturation fires every eligible transform together,
  bounded by per-region fuel. Adding the 30th pure transform to the
  algebraic group does not add a 30th IR walk — it adds one entry to
  the saturation worklist.
- **Impure transforms** share a worklist seeded from the union of their
  pattern root op categories. The worklist visits each op once per
  iteration; every transform whose root matches that op category gets
  a try. The 30th impure transform adds one entry to the
  per-op-category dispatch table.

The cost story: adding the 101st transform costs one bit in fingerprint
masks (if needed), one entry in the matcher dispatch, one ledger key
shape, one fuel budget declaration, and one registry line. Regions
whose fingerprint excludes the new transform pay zero for it.

### Authoring tools

`wrela pass scaffold <name>` generates the skeleton for a new pass and
plumbs it into the registry, the fixture corpus, and the test suite.

```text
$ wrela pass scaffold mask-or-zero --scope match-local --group algebraic

created:
  src/mir/passes/mask_or_zero.rs               (transform skeleton)
  src/mir/passes/registry.rs                   (entry added)
  fixtures/passes/mask_or_zero/positive.wrela  (fires here)
  fixtures/passes/mask_or_zero/negative.wrela  (does not fire here)
  tests/passes/mask_or_zero.rs                 (positive/negative harness)

next steps:
  - fill TransformMetadata.fixtures
  - implement enumerate_candidates
  - implement legality
  - implement apply
  - update fixtures/passes/mask_or_zero/{positive,negative}.wrela
```

The generated transform is a no-op (enumerates zero candidates) but is
fully wired: it appears in `wrela passes`, the lint passes, and the
test suite runs against the (empty) fixtures. Author fills in the
optimization; harness handles the rest.

`wrela pass scaffold` lands in MIR 06 alongside the SDK. It is the
forcing function that proves the SDK ergonomics are good — if the
scaffolded output requires more than a few minutes of filling in to
become a useful transform, the SDK has friction the design did not
catch.

### Bidirectional ledger

The ledger is a **projection** over `RunTrace`. There is no parallel
write path; the harness writes events to the trace, and the ledger
projection rebuilds (or incrementally updates) from the events at
end-of-run or on-demand.

#### The four statuses

```text
Status              Meaning                                   Harness behavior
------              -------                                   ----------------
known-winner        measurement showed median improvement     dispatch (informs cost)
                    above threshold across ≥7 samples with
                    checksum match

known-loser         measurement showed median regression      skip; emit GroupSkipped
                    or no improvement above threshold

needs-remeasure     prior measurement inconclusive            dispatch; update on
                                                              new evidence

known-no-op  (NEW)  pass has been dispatched N consecutive    skip; emit GroupSkipped
                    times against this region key and
                    matched zero candidates
```

#### Status transitions

Transitions are deterministic functions of the trace projection:

```text
needs-remeasure → known-winner
    if last K runs all show: checksum match, ≥7 samples per side,
    p90/p10 ≤ 1.10, median improvement ≥ 3%

needs-remeasure → known-loser
    if last K runs all show: checksum match, ≥7 samples per side,
    p90/p10 ≤ 1.10, median regression or no improvement

* → needs-remeasure
    if compiler_version, pass_config, target_profile, or
    fingerprint_schema_version changes
    OR
    if a LedgerViolation event is emitted

unknown → known-no-op
    if N consecutive runs show zero TransformCandidateFound events
    for this key (default N = 3)

known-no-op → needs-remeasure
    if a TransformCandidateFound event for this key appears under
    probabilistic revalidation
```

These transition rules live in the ledger projection function, not
scattered across dispatch sites. Changing them is a change to one
component.

#### Revalidation discipline

A ledger that always skips can never discover that the skip became
wrong. Three mechanisms keep it honest:

- **`--remeasure`** forces re-execution of all ledger-skipped passes
  for one run and updates ledger entries from the resulting trace.
- **Quality-gate strict mode** (`QUALITY_GATE_STRICT_CLEAN=1`)
  bypasses ledger skips entirely. CI catches drift even when developer
  runs trust the ledger.
- **Probabilistic revalidation.** Every Nth run (deterministic by
  `region_hash mod N` to avoid synchronized re-runs across the
  corpus), dispatch the skipped pass anyway. If the pass fires when
  ledger said it would not, emit a `LedgerViolation` event,
  transition the entry to `needs-remeasure`, and trigger auto-bisect
  as a regression candidate.

The default revalidation rate `N = 50` is tuned by measurement during
MIR 07.

#### Schema versioning

Changing fingerprint partitioning, transform version, pass version,
target profile, compiler version, or certificate schema **invalidates
ledger entries keyed by them**. The projection treats mismatched
entries as `needs-remeasure`, never trusting them silently. Corrupt
entries are also `needs-remeasure` with a structured report.

### Coverage and attribution

Both are projections over the trace. Authors do not instrument; the SDK
emits events at dispatch boundaries, and the projections aggregate.

#### Coverage

Answers "what fired, what didn't, and why." Per run, per fixture, per
pass, per transform:

```text
fixture: filter_sum.wrela
  pass: mask-algebra (data-plane)
    regions_visited:                  4
    regions_skipped_fingerprint:     11
    regions_skipped_ledger_no_op:     2
    regions_skipped_facts_failed:     1
    transforms:
      mask-and-self:    candidates=4 facts_ok=2 applied=2
      mask-or-zero:     candidates=4 facts_ok=0 applied=0
      mask-xor-self:    candidates=4 facts_ok=0 applied=0
        fact_failure: same_mask_domain (4)
```

CI baseline diffing flags:

- A transform with zero applications across the corpus for N
  consecutive runs ("dead transform").
- A fixture with zero release-pass applications ("uncovered fixture").
- A pass's applied count changed beyond threshold without an authored
  change.

#### Attribution

Answers "where did fuel and time go." Per run, per pass:

```text
release pipeline total: 2.3s (wall-clock, telemetry)
  canonicalize:        180ms   fuel=1240/4000
  algebraic:           420ms   fuel=2840/8000
    scalar-identity:   140ms   candidates=1240  applied=44
    mask-algebra:      280ms   candidates=380   applied=8
  data-plane:          610ms   fuel=4900/12000
  lowering-prep:       190ms   fuel=920/4000
  structural:          900ms   fuel=18400/40000
    inliner:           780ms   regions_inlined=12
    devirt:            120ms   sites_resolved=3
```

Wall-clock is telemetry only. Fuel is the deterministic budget. CI
diffing flags:

- A pass's fuel-share changed beyond threshold without an authored
  change (probable upstream regression producing more work).
- Aggregate group fuel exceeded its declared budget.
- Wall-clock and fuel diverge unexpectedly (likely a non-deterministic
  cost source the SDK should investigate).

### Fuel budgets

Wall-clock budgets as hard errors are flaky across machines and CI
runners. The harness uses **fuel** as the hard invariant.

Fuel is denominated in countable deterministic operations:

```rust
pub struct FuelBudget {
    pub candidates_fuel:  u32,   // pattern match / enumeration attempts
    pub fact_fuel:        u32,   // authority engine queries
    pub mutation_fuel:    u32,   // IR node creations + removals
    pub worklist_fuel:    u32,   // worklist pops
}
```

Each transform declares a per-region budget across these denominations.
The harness meters dispatch; exceeding any budget suspends the
transform for that region and emits `FuelExceeded`.

`PassGroup` carries an aggregate budget across all its transforms.
Exceeding the group budget skips remaining transforms in the group with
a structured report.

#### Enforcement policy

- **Dev mode**: exceeding fuel is a hard build error. Forces the author
  to tighten or move to `Experimental`.
- **Release mode**: exceeding fuel suspends the transform for the
  region, emits a certificate with `fuel-exceeded` provenance, and
  continues. Three consecutive fuel violations on the same region key
  transition the ledger entry to `known-loser`.
- **Quality gate**: exceeding aggregate group fuel fails the gate.
  Per-transform fuel violations are reported but do not fail the
  ordinary gate.

Wall-clock telemetry continues to be recorded. When wall-clock and fuel
diverge unexpectedly, the harness emits an advisory event so the SDK
can investigate the missing cost source.

### `wrela why`

The human face of the trace projection. Several modes, all backed by
the same trace store:

- **`wrela why <fixture> --transform <name>`** — every application of
  that transform on that fixture. Before/after MIR text, facts queried
  with results, verifier outcome, ledger status, A/B benchmark result
  if one exists.
- **`wrela why <fixture> --pass <pass-name>`** — aggregated by pass.
  Regions touched, regions skipped with reasons, transform firing
  counts, ledger statuses, fuel consumption.
- **`wrela why <fixture> --region <region-id>`** — every pass that
  dispatched on the region, every transform that fired, every fact
  queried, every certificate emitted, in order.
- **`wrela why --regression <baseline> <candidate>`** — diff between
  two traces. Transforms that fired in baseline but not candidate (or
  vice versa). Measurement deltas. Fact-query result deltas. Ledger
  status changes.
- **`wrela why <fixture> --skipped`** — what *didn't* fire and why.
  Fingerprint-miss, ledger-no-op, facts-failed, fuel-exceeded,
  purity-rejected — categorized.
- **`wrela why --cold`** — transforms that have never fired on any
  fixture in the corpus history. The "dead transform" query.

For `RegionLocal` transforms, `wrela why` renders the structural diff
(regions inlined, sites devirtualized, loops fused) with before/after
MIR snippets. Cross-region structural diffs are deferred along with
cross-region transforms.

A minimal `wrela why --pass` ships in MIR 06 as the smoke test that the
trace schema carries the right data; the full mode set lands in MIR 08.

### Auto-bisect

When a regression is detected, the harness searches for the minimal
pass set that triggers it via **bounded delta debugging** (ddmin), then
hands the result to the MIR 05 reducer for input minimization.

#### Trigger conditions

- A `wrela perf code` A/B benchmark regresses beyond noise threshold.
- A fixture's output checksum drifts between baseline and candidate
  traces.
- A `VerifierFailed` event appears in candidate but not baseline.
- A ledger `known-winner` flips to `known-loser`.
- A `LedgerViolation` event appears.
- An aggregate group fuel overrun appears in candidate but not
  baseline.

#### Algorithm: bounded ddmin

Naive binary split does not find interaction bugs (two passes
regressing together where neither alone does). The harness uses
**Zeller's delta debugging** (ddmin) with an iteration cap:

```text
given:
  baseline pass set P, candidate P' with P ⊆ P'
  regression on fixture F

ddmin:
  delta = P' \ P
  granularity = 2
  while |delta| ≥ 2 and iterations < cap:
    partition delta into `granularity` equal chunks
    for each chunk C:
      if regression(P ∪ C, F): delta = C; granularity = 2; break inner
    else:
      for each chunk C:
        if regression(P ∪ (delta \ C), F): delta = delta \ C; break inner
      else:
        granularity = min(2 * granularity, |delta|)
        if granularity > |delta|: break outer

result = P ∪ delta
invoke reducer on F with pass set = result
```

For a single-pass culprit, ddmin terminates in O(log |P'|) compiles.
For interaction bugs, ddmin terminates at a 1-minimal subset (every
proper subset stops reproducing) in O(|P'|²) compiles in the worst
case. The iteration cap bounds this; if ddmin fails to converge under
the cap, the harness reports the smallest subset found so far and
flags it as `interaction-suspected`.

#### Output

`target/wrela/bisects/<timestamp>/` contains the minimal pass set, the
minimized input from the reducer, before/after MIR dumps, certificate
chain for the regressing region, and the attribution snapshot. The
artifact has the shape of a bug report a human would assemble manually
in 30 minutes.

Bisect runs in CI on failure, not on every build.

### Matcher determinism

The pattern matcher is deterministic across runs and across machines.
Specifically:

- **Visitation order** is determined by stable region IDs, then stable
  block IDs, then stable op IDs — never by heap addresses or hashmap
  iteration order.
- **Transform attempt order within a candidate** is registration order
  in the registry, not alphabetical or hashed.
- **Tie-breaking** when multiple transforms could fire at the same
  candidate is registry-order: the first-registered transform wins,
  applied first; subsequent transforms see the post-application IR.
- **Fact lookups** are deterministic functions of the IR snapshot
  identity; the authority engine caches by stable hash, not by
  pointer.

These invariants are tested via a trace-equality test: the same input
compiled twice must produce byte-identical canonical traces (including
event ordering and event IDs, excluding advisory wall-clock telemetry).

### Pass flag precedence

CLI flags resolve in this order (left-to-right within each category,
then category order top-to-bottom):

```text
1. --only-pass <name>     (whitelist; overrides everything below)
2. --disable-pass <name>  (overrides --enable-pass)
3. --enable-pass <name>   (overrides default-enabled)
4. transform default-enabled state for current mode
```

When a flag refers to a pass name not in the registry, the CLI errors
out (no silent ignore). When `--only-pass` is specified, the
default-enabled mechanism is bypassed entirely.

### Fact serialization

Facts produced by `LegalityContract` checks are serialized into
certificate `required_facts` fields with a stable, deterministic
encoding:

- Each `FactKind` is a `u16` opcode.
- Each fact's payload is a tagged structured value (boolean, region ID,
  op ID, hash, or string from a closed enum).
- Encoding order is `FactKind` opcode order, not declaration order.

The serialization format is part of the certificate schema and is
versioned alongside it. A certificate produced under schema V0 cannot
be read against schema V1 without a migration.

## Worked example

A small `MatchLocal` transform landing through the harness end to end.

### Source

The optimization: `mask.and(m, m) → m`. A mask AND-ed with itself is
the mask itself. Pure local rewrite; routes to the aegraph after
pre-classification.

### Transform definition

```rust
// src/mir/passes/mask_algebra/and_self.rs

pub struct AndSelf;

impl Transform for AndSelf {
    type Scope = MatchLocal;
    type Candidate = MaskAndMatch;
    type Facts = MaskSameDomainFacts;

    const METADATA: TransformMetadata = TransformMetadata {
        name:        "mask-and-self",
        version:     1,
        group:       PassGroup::Algebraic,
        category:    Category::Algebraic,
        purity:      PurityClass::PurePendingFacts,
        health:      HealthState::Candidate,
        fuel_budget: FuelBudget {
            candidates_fuel: 256,
            fact_fuel:        64,
            mutation_fuel:    16,
            worklist_fuel:   128,
        },
        fixtures: &[
            "fixtures/passes/mask_and_self/positive.wrela",
            "fixtures/passes/mask_and_self/negative.wrela",
        ],
    };

    const ELIGIBILITY: FingerprintRequirement = FingerprintRequirement {
        required_all: FingerprintMask::HAS_MASK_OP,
        required_any: FingerprintMask::EMPTY,
        forbidden:    FingerprintMask::EMPTY,
    };

    fn enumerate_candidates(
        &self,
        region: &Region,
        ctx: &EnumerateCtx<'_>,
    ) -> CandidateIter<MaskAndMatch> {
        ctx.match_pattern(MaskAndPattern::same_operands())
    }

    fn legality(
        &self,
        candidate: &MaskAndMatch,
        ctx: &FactCtx<'_>,
    ) -> MaskSameDomainFacts {
        MaskSameDomainFacts {
            same_mask_domain: ctx.same_mask_domain(candidate.lhs(), candidate.rhs()),
        }
    }

    fn apply(
        &self,
        candidate: &MaskAndMatch,
        builder: &mut Builder<'_>,
    ) -> ApplyResult {
        builder.replace(candidate.root(), candidate.lhs())
    }
}

pub static PASS: MaskAlgebraPass = MaskAlgebraPass::with_transforms(&[&AndSelf]);
```

### Registry entry

```rust
// src/mir/passes/registry.rs
pub const PASSES: &[&'static dyn PassDescriptor] = &[
    // ... other passes
    &super::mask_algebra::PASS,
];
```

### What the harness produces automatically

On the first dispatch against `filter_sum.wrela`, with no further
author work:

- A versioned certificate per application embedded in a
  `TransformApplied` event:
  ```text
  certificate_version: 1
  transform: mask-and-self
  pass:      mask-algebra
  region:    Region(42)
  source_ops: [Op(118)]
  before_hash: 0x9af2...
  after_hash:  0x4b81...
  required_facts:
    same_mask_domain: yes
  verifier: passed
  ```
- A `TransactionCommitted` event recording the before/after region
  hashes.
- `TransformCandidateFound` and `TransformFactsChecked` events for the
  candidate.
- `FuelConsumed` events recording match-fuel, fact-fuel, and
  mutation-fuel consumed.
- A ledger projection entry keyed by `(region_hash, target_profile,
  pass_config, compiler_version, "mask-algebra")` recording the
  outcome.
- A `wrela passes coverage` projection showing the transform fired.
- A `wrela why filter_sum.wrela --transform mask-and-self` query
  renders the before/after MIR, facts, and certificate.

The author wrote one struct and one trait impl. The harness wrote
everything else.

## Experiment lab and whole-program harness

MIR 06-09 make local optimization accountable. MIR 10-18 extend the
same accountability to experiments, whole-program facts, multi-region
edits, schedule search, online-learning proposal, target-cost tracing,
and assumption-delta analysis. The design rule is unchanged: every
fact, decision, edit, and measurement becomes traceable evidence.

### ExperimentSpec

An `ExperimentSpec` is the unit of empirical optimization. It is a
content-addressed input blob that tells the harness exactly what to
run, what to compare, and what evidence is required before a result
can update projections or be promoted.

```text
ExperimentSpec {
    name:                  String
    hypothesis:            String
    corpus_hash:           StableHash
    target_profile_hash:   StableHash
    baseline:              ExperimentArm
    candidates:            [ExperimentArm]
    measurement:           MeasurementPolicy
    promotion_policy:      PromotionPolicy
}

ExperimentArm {
    name:             String
    pass_config_hash: StableHash
    schedule_hash:    StableHash
    cost_model_hash:  StableHash
    search_policy_hash: Option<StableHash>
}

MeasurementPolicy {
    repeat:                  u16
    warmup:                  u16
    checksum_required:       bool
    min_runtime_movement_ppm: i32
    max_noise_ratio_ppm:     u32
    max_compile_fuel:        FuelBudget
    max_code_size_growth_ppm: u32
    independent_runs:        u16
}
```

`wrela experiment <spec>` runs every arm, emits
`ExperimentStarted` / `ExperimentFinished`, and stores a `RunTrace`
for each run. Results are not trusted because an experiment says so;
they are trusted only if the resulting traces satisfy the measurement
policy and all Trident gates.

### WorldSummary

`WorldSummary` is the read-only whole-program fact layer. It lets
passes see the whole image without granting them ambient permission to
mutate the whole image.

```text
WorldSummary {
    root_hash:          StableHash
    call_graph:         CallGraphSummary
    effect_summaries:   [RegionEffectSummary]
    capability_graph:   CapabilityGraphSummary
    layout_inventory:   LayoutInventory
    escape_summaries:   [EscapeSummary]
    hotness_seeds:      [StaticHotnessSeed]
    region_features:    [RegionFeatureSummary]
    region_costs:       [RegionCostSummary]
}
```

Transforms query `WorldSummary` through the same fact interface as
region-local legality checks. Every lookup emits `WorldFactQuery` with
the fact kind, touched regions, and result. A missing or stale
`WorldSummary` fact is a failed legality query, not permission to
guess.

World facts are inputs to legality and profitability. They are not
edits. This keeps whole-program knowledge from becoming whole-program
mutation until MIR 12 introduces transactions that can prove their
boundaries.

### RegionSetPattern

`RegionSetPattern` is the cross-region analog of a match-local
candidate source. It recognizes an optimizable constellation of
regions and returns a candidate with explicit membership.

```text
pub trait RegionSetPattern: 'static + Sync {
    const NAME: &'static str;
    const FINGERPRINT: RegionSetFingerprintRequirement;

    type Candidate: RegionSetCandidate;
    type Facts: RequiredFacts;

    fn enumerate(
        &self,
        world: &WorldSummary,
        module: &MirModule,
        ctx: &RegionSetCtx<'_>,
    ) -> RegionSetCandidateIter<Self::Candidate>;

    fn legality(
        &self,
        candidate: &Self::Candidate,
        ctx: &WorldFactCtx<'_>,
    ) -> Self::Facts;
}
```

The first pattern families are deliberately concrete:

- `caller + callee` for inlining and specialization.
- `producer + consumer` for table/mask pipeline fusion.
- sibling `theta-for` regions over the same table for loop fusion.
- `delta frame + users` for allocation sinking and escape cleanup.

No pass receives a mutable module because it found a pattern. It
receives a candidate and must propose a transaction.

### MultiRegionTransaction

`MultiRegionTransaction` is the only way MIR 12+ transforms modify
more than one region.

```text
MultiRegionTransaction {
    transaction_id: StableHash
    regions:        [RegionId]          # sorted by stable region ID
    before_hashes:  [StableHash]
    edits:          [RegionEdit]
    boundary_facts: BoundaryFactBundle
    certificates:   [Certificate]
    fuel_budget:    FuelBudget
}
```

Commit checks:

- every touched region verifies after edits
- call edges still type-check and effect-check
- ABI-visible shapes are unchanged unless the transform explicitly
  produces a private clone
- capability authority is preserved or narrowed
- state-token and sync-token ordering is preserved
- trap-producer ordering is preserved
- frame, row, capacity, and other linear tokens do not escape
- untouched neighboring regions still agree with touched-region
  signatures
- generated-code checksums match when codegen evidence is available

Failure abandons the transaction and preserves every prior region.
`MultiRegionTransactionStarted`, `Committed`, and `Abandoned` events
make cross-region edits visible to projections and `wrela why`.

### Inlining and specialization

Inlining is the first multi-region pass because it exercises the
whole callgraph while relying on familiar compiler mechanics.

The pass considers every reachable call edge from `WorldSummary`,
classifies recursion and SCCs, and creates candidates of the form
`caller region + callee region + call site`. It may clone a callee
before inlining when call-site facts make a smaller specialized body:

- constant arguments
- known enum variants
- fixed capability paths
- narrower effect sets
- fixed table/layout identities
- non-escaping frame lifetimes
- disjoint mutation places

The pass is selective, not "inline everything." Profitability uses
size budgets, fuel, static hotness seeds, target profile, and ledger
evidence. A rejected call edge still emits a decision event so
`wrela why --call-edge` can explain why it stayed a call.

### Cross-region data-plane fusion

Cross-region data-plane fusion is the first Wrela-specific
multi-region optimization family. It fuses producer/consumer regions
when Wrela facts prove that table/mask work can be performed as one
lowerable pipeline.

Legality depends on:

- same table or compatible table provenance
- compatible mask domains and row counts
- row tokens that do not escape their originating loop
- capacity-token dataflow preservation
- disjoint or ordered mutation places
- effect subsets that allow fusion
- preserved state-token, sync-token, and trap order

The expected payoff is not just fewer operations in W-MIR. The fused
shape should expose tighter LIR/AArch64 lowering: chunked NEON masks,
single-pass gather/filter/scatter loops, fewer bounds checks, and
better memory locality.

### Schedule-as-data

A `Schedule` is a content-addressed policy object. It makes pass order
and repetition explicit so the harness can compare, bisect, search,
and replay scheduling decisions.

```text
Schedule {
    name:             String
    groups:           [ScheduleGroup]
    fixed_point:      FixedPointPolicy
    fuel_policy:      FuelPolicy
    ledger_policy:    LedgerPolicy
    fallback_policy:  FallbackPolicy
}

ScheduleGroup {
    group:       PassGroup
    passes:      PassSelection
    rounds:      RoundPolicy
    after_round: CanonicalizationPolicy
}
```

MIR 15 keeps production deterministic: a build uses one explicit
schedule hash. Search may generate many schedules, but each is just
another blob referenced from a trace. `ScheduleDecision` records the
reason a schedule or per-region schedule branch was chosen.

### Automated search

Search proposes candidates; it does not certify them. The first search
strategies are intentionally simple:

- grid search over small knobs
- random search over broad knobs
- successive halving to discard losers early

Search candidates can vary pass enablement, schedule order,
fixed-point rounds, fuel budgets, inline thresholds, vector-width
choices, fusion aggressiveness, and cost-model weights. Each proposal
emits `CandidateConsidered`; after measurement it emits
`CandidateScored`. Invalid candidates (verifier failure, checksum
drift, certificate failure, budget overrun) are scored as invalid, not
slow-but-acceptable.

### Optimization lab exerciser

MIR 16 proves that MIR 15's search machinery finds real signal rather
than noise. A lab suite is a content-addressed collection of corpora,
seeded known-good configurations, seeded known-bad configurations,
regression traps, and noise-only cases.

Tracks:

- scalar: identities, branch/select, trap-check elimination
- callgraph: inlining thresholds, specialization, code-size tradeoffs
- data-plane: mask fusion, table loop fusion, vector width choices
- memory/authority: bounds and capacity-token cleanup
- schedule: pass ordering and fixed-point policy
- negative: tempting but wrong or slower configurations

The exerciser must demonstrate that search can rediscover wins, reject
losses, classify noise as inconclusive, replay winners, and produce
bisect artifacts when a seeded regression trips. It is the harness's
scientific-instrument test.

### LinUCB online-learning proposer

MIR 17's first ML model is LinUCB, a small contextual bandit suitable
for std-only implementation and explainable enough for `wrela why`.
It is deliberately not deep RL.

```text
context features:
  target profile
  region feature summaries
  world-summary features
  schedule fingerprint
  code-size estimate
  hotness seed
  ledger history

actions:
  candidate schedule
  pass enablement set
  inline threshold
  fusion aggressiveness
  aegraph fuel tier
  vector width choice
  cost-model weight bucket

reward:
  runtime movement
  - code size penalty
  - compile time penalty
  - fuel penalty
  invalid if verifier/checksum/certificate gates fail
```

LinUCB updates happen only in lab runs. `SearchPolicyUpdated` records
the new policy hash and the trace IDs used for the update. Normal
production builds consume explicit, promoted policy blobs; they do
not silently learn from user builds.

### TargetCostTrace and AssumptionDelta

MIR 18 adds the emitted-code counterpart to compiler fuel. Compiler
fuel tells Wrela how much work the optimizer spent. `TargetCostTrace`
tells Wrela what structural work the emitted program is expected to
perform, before noisy wall-clock measurement enters the picture.

```text
TargetCostTrace {
    scope:                       CostScope
    static_instructions:         u32
    estimated_dynamic_instrs:    u64
    scalar_ops:                  u64
    vector_ops:                  u64
    loads:                       u64
    stores:                      u64
    estimated_bytes_read:        u64
    estimated_bytes_written:     u64
    branches:                    u64
    unpredictable_branches:      u64
    calls:                       u64
    traps_or_checks:             u64
    atomics:                     u64
    fences:                      u64
    spills:                      u64
    reloads:                     u64
    code_bytes:                  u32
    register_pressure_peak:      u16
}
```

The trace is derived from W-MIR, LIR, AArch64 metadata, loop trip-count
facts, table row-count facts, vector width, and target profile. It is
not a claim that runtime must move by a specific percentage. It is a
structural prediction: "this candidate should reduce these kinds of
emitted work."

Transforms and experiments can also record an `OptimizationClaim`:

```text
OptimizationClaim {
    transform_or_candidate: StableHash
    expected_cost_delta:    TargetCostDelta
    expected_runtime:       Faster | Slower | Unchanged | Inconclusive
    rationale_facts:        [FactId]
}
```

After measurement, the harness compares compiler fuel, target-cost
movement, and hardware result. The result is an `AssumptionDelta`:

```text
AssumptionDelta {
    claim_id:              StableHash
    cost_prediction_match: bool
    runtime_prediction_match: bool
    observed_cost_delta:   TargetCostDelta
    observed_runtime_delta: RuntimeDelta
    likely_missing_costs:  [MissingCostKind]
}
```

This is the "which part of our mental model was wrong?" artifact. If
instructions drop but runtime worsens, the missing cost may be
register pressure, code size, cache locality, dependency depth,
alignment, vector setup cost, branch predictability, or memory
behavior. If modeled cost worsens but runtime improves, the cost model
over-penalized something. Either way, failed expectations become
training data instead of vibes.

## Phased delivery

Each phase is an independent implementation plan with its own
acceptance criteria. The plans are downstream of MIR 05 and do not
modify the MIR 01-05 plans. MIR 06-09 establish the local optimizer
harness. MIR 10-18 extend the same trace/projection/transaction model
to the optimization lab, whole-program facts, multi-region transforms,
schedule search, exercising, online-learning proposal, and target-cost
assumption analysis.

### MIR 06 — Optimizer Harness SDK, RunTrace, Registry, Transactional Dispatch

The harness spine. The SDK Transform model, the registry, the retrofit
of existing passes, the unified `RunTrace` event log with locked
schema, transactional dispatch with the scratch builder, and the
authoring tool.

**Scope:**

- The `Transform` trait with `MatchLocal` and `RegionLocal` scopes.
- `TransformMetadata`, `Eligibility`, `CandidateSource`,
  `LegalityContract`, and `Apply` as the five Transform concerns,
  realized as associated types and methods on `Transform` (exact
  syntax decided during implementation).
- `TransformVtable` object-safe companion with blanket-impl erasure.
- `Pass`, `PassDescriptor`, and `PassGroup`.
- `src/mir/passes/registry.rs` as the single source of truth.
- CI lint that confirms every `pub static PASS` declaration appears in
  `PASSES`.
- `RunTrace` data structure with schema locked at this phase. Reserved
  variants and reserved fields for the MIR 10-18 lab phases exist in
  the closed enum and serialize correctly but are not emitted by MIR
  06-09 dispatch code (except `RegionFeatureSummary` per below).
- Content-addressed input blob store under
  `target/wrela/blobs/<stable-hash>.bin` for pass config, target
  profile, experiment spec, lab suite, schedule, cost model, search
  policy, and benchmark corpus. Trace fields reference blobs by hash.
  Garbage collection is conservative (reachability from retained
  traces and ledger entries).
- `RegionFeatureSummary` minimal emission at MIR 06: a feature vector
  of at most 20 `u32` counters computed at region construction and
  after every committed mutation. No MIR 06-09 projection consumes the
  summary; the storage accumulates training data for future ML work.
- Persistent trace storage under `target/wrela/traces/<run_id>.bin`
  with a default retention policy (last 50 runs).
- Transactional dispatch: scratch builder, verifier-on-commit,
  abandon-on-failure. `Builder` API replaces any prior `&mut Region`
  signatures.
- Pre-classification phase for purity: statically pure, pending,
  impure. Pending candidates run fact checks before aegraph admission.
- Retrofit of MIR 03 scalar-identity transforms, MIR 04 mask-algebra
  and branch/select transforms, and MIR 05 table-loop-fusion into the
  new model. Behavior-preserving against MIR 05 golden text and
  benchmark checksums.
- Ledger reframed as a projection over `RunTrace`. The MIR 05
  persistent ledger format remains; the write path becomes "rebuild
  from trace at end of run."
- `wrela passes` CLI subcommand from registry.
- `wrela pass scaffold <name>` CLI subcommand generating module
  skeleton, registry entry, fixture skeletons, and test skeleton.
- Minimal `wrela why <fixture> --pass <pass-name>` rendering
  aggregated pass results from the trace projection.
- `target/wrela/passes.md` auto-generated from registry metadata.
- Rule health states declared in metadata (`Experimental`,
  `Observed`, `Candidate`, `Release`, `Retired`); retrofitted
  transforms start at `Release`.
- Matcher determinism invariants enforced by trace-equality tests.
- Flag precedence (`--only-pass` > `--disable-pass` >
  `--enable-pass` > default) implemented.

**Acceptance criteria:**

- Every retrofitted pass produces byte-for-byte identical W-MIR text
  and benchmark checksums to MIR 05's baselines.
- Adding a (test) pass via `wrela pass scaffold` requires no manual
  edits to satisfy the CI lint, and the no-op pass appears in `wrela
  passes` and `target/wrela/passes.md`.
- The same input compiled twice produces byte-identical canonical trace
  files under `target/wrela/traces/<run_id>.bin`; advisory telemetry
  sidecars may differ.
- Pass config, target profile, and cost model blobs are written
  exactly once per unique content; re-running a compile that uses an
  already-stored config produces a trace that references the existing
  blob without rewriting it.
- `RegionFeatureSummary` events are emitted at region construction
  and after every `TransactionCommitted`; the feature vector is
  deterministic across runs.
- The closed `Event` enum compiles with every reserved variant
  present; a test asserts that no MIR 06-09 dispatch code path emits
  any reserved variant except `RegionFeatureSummary`.
- Verifier failure during a pass abandons the scratch region and
  preserves the prior region; a test fixture proves this.
- Fuel exhaustion mid-mutation abandons the scratch region; a test
  fixture proves this.
- `wrela passes` lists every pass and matches the build-time
  `target/wrela/passes.md`.
- `wrela why <fixture> --pass <name>` renders coverage and
  certificate data for at least one retrofitted pass.
- The `RunTrace` schema is locked, documented, and version-1; the
  event enum is closed.
- MIR 05 ledger entries are either re-keyed or invalidated explicitly
  with a structured migration report; the new ledger derives from the
  trace projection.
- `./scripts/quality-gate.sh` passes.
- `QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh` passes.

**Out of scope:** fingerprint computation and gating, `known-no-op`
ledger status, ledger revalidation, fuel enforcement, full `wrela
why` modes, coverage / attribution drift alerts, auto-bisect. (Fields
exist in the schema; they are filled in later phases.)

### MIR 07 — Fingerprint Substrate, Sound Group Eligibility, Bidirectional Ledger

The dispatch-gating substrate. Cheap deterministic skips and the
`known-no-op` ledger status with revalidation discipline.

**Scope:**

- Fingerprint computation per region, stored on the region, recomputed
  during transactional commit.
- The fingerprint bit layout finalized (16 op-category + 11 effect + 3
  region-kind + 8 type + 3 op-count + 23 reserved).
- `FingerprintRequirement` (any / all / forbidden masks) populated on
  every retrofitted transform.
- **Sound group-level eligibility** via per-transform iteration with
  the common-requirement bucket optimization. Merged-mask
  approximations are explicitly prohibited.
- `known-no-op` ledger projection status.
- `--remeasure` CLI flag for forced re-execution of ledger-skipped
  passes.
- Quality-gate strict mode bypasses ledger skips entirely.
- Probabilistic revalidation (`N = 50`, tunable) with
  `LedgerViolation` events on discrepancy.
- Ledger schema version bump and migration from MIR 06 keys.

**Acceptance criteria:**

- Region fingerprints are computed at region construction and during
  every transactional commit; recomputation is O(ops in region).
- A test fixture proves a transform with `required_all = HAS_MASK_OP`
  is skipped on a region with no mask ops; a transform with
  `forbidden = HAS_MMIO` is skipped on a region with MMIO ops; a
  transform with overlapping `forbidden` and `required_all` masks
  against other transforms in the same group dispatches correctly
  (validating sound group eligibility).
- Aggregate dispatch work measurably reduces on the corpus compared
  to MIR 06's attribution baseline.
- `--remeasure` re-runs ledger-skipped passes and updates ledger
  entries.
- Strict quality-gate mode dispatches all passes regardless of
  ledger status.
- Probabilistic revalidation samples skipped passes at the configured
  rate and produces `LedgerViolation` events when discrepancies are
  found.
- MIR 06 retrofitted passes continue to produce byte-for-byte
  identical output.

**Out of scope:** fuel enforcement, full `wrela why` modes, full
coverage + attribution, auto-bisect.

### MIR 08 — Projections: Coverage, Attribution, Full `wrela why`

The human-facing surfaces. Turns the trace projections into reports
and into the `wrela why` product.

**Scope:**

- Full coverage projection generated per run, stored at
  `target/wrela/coverage/<timestamp>.json`.
- Full attribution projection generated per run, stored at
  `target/wrela/attribution/<timestamp>.json`.
- CI baseline diffing for both reports with drift alerts for dead
  transforms, uncovered fixtures, fuel-share regressions, and
  applied-count changes.
- `wrela passes coverage` subcommand.
- `wrela perf attribution` subcommand.
- Full `wrela why` mode set: `--transform`, `--pass`, `--region`,
  `--regression`, `--skipped`, `--cold`.
- `RegionLocal` transform rendering for `wrela why` (single-region
  only).
- Documentation: `target/wrela/passes.md` extended with last-run
  coverage and attribution summary per pass.
- Trace retention policy refinement: per-run traces retained up to
  budget; coverage and attribution snapshots retained per CI baseline
  pinning.

**Acceptance criteria:**

- Coverage and attribution projections are produced for every
  release-mode run.
- A test fixture proves CI drift alerts fire for: a transform with
  zero applications across N consecutive runs, a fixture with zero
  release-pass applications, a pass with fuel-share change above
  threshold.
- `wrela why` renders all six modes for at least one fixture; output
  is deterministic across runs.
- `wrela perf attribution` reports per-pass fuel and wall-clock with
  divergence flagged.
- MIR 06/07 IR output is preserved byte-for-byte.

**Out of scope:** fuel enforcement, auto-bisect.

### MIR 09 — Fuel Enforcement and Bounded Delta-Debugging Bisect

The hard-invariant phase. Per-transform fuel becomes a build-affecting
signal; regressions trigger automatic ddmin.

**Scope:**

- Per-transform fuel budgets enforced per region per dispatch.
- Per-group aggregate fuel budgets enforced per region.
- Mode-dependent enforcement policy (hard error in dev; suspend +
  emit in release; gate failure for aggregate violations in CI).
- Three-consecutive-fuel-violations automatic transition to ledger
  `known-loser`.
- Wall-clock vs fuel divergence advisory events.
- Auto-bisect on the six trigger conditions (perf regression,
  checksum drift, verifier-failure delta, winner-to-loser flip,
  ledger violation, group-fuel overrun).
- Bounded delta-debugging (ddmin) with iteration cap; reports
  `interaction-suspected` when convergence fails.
- Composition with the MIR 05 reducer: bisect produces minimal pass
  set; reducer produces minimal input; combined under
  `target/wrela/bisects/<timestamp>/`.
- `wrela bisect <baseline-trace> <candidate-trace> <fixture>` manual
  invocation.

**Acceptance criteria:**

- A fuel-exceeding test transform in dev mode produces a hard build
  error.
- A fuel-exceeding transform in release mode is suspended for the
  region and emits a `fuel-exceeded` event; the rest of the dispatch
  completes.
- Aggregate group fuel overrun fails `./scripts/quality-gate.sh`.
- Three consecutive per-transform fuel violations on the same region
  key flip the ledger entry to `known-loser`.
- A test fixture with a deliberately-regressing candidate pass
  triggers auto-bisect, the minimal pass set is identified within
  the iteration cap, and the reducer artifact is produced.
- Interaction-bug fixtures (two passes regressing together where
  neither alone does) terminate at a 1-minimal subset or report
  `interaction-suspected` with the smallest subset found.
- `wrela bisect` produces deterministic output across runs.

**Out of scope:** cross-region transforms, ML-driven pass scheduling,
parallel cross-region dispatch, additional specific transforms beyond
what was retrofitted.

### MIR 10 — Experiment Runner and Immutable Trace Corpus

The empirical layer. Compiler runs become named, replayable
experiments with baseline/candidate arms, measurement policy, and
trace-backed conclusions.

**Scope:**

- `ExperimentSpec`, `ExperimentArm`, `MeasurementPolicy`, and
  `PromotionPolicy` content-addressed blobs.
- `wrela experiment <spec>` CLI command.
- Experiment trace corpus under `target/wrela/experiments/`.
- `ExperimentStarted` and `ExperimentFinished` emission.
- Baseline/candidate trace comparison across runtime, code size,
  compile fuel, emitted bytes, verifier outcome, and checksum.
- Ledger updates cite the trace IDs and experiment IDs that justified
  them.
- `wrela why --experiment <id>` minimal rendering.

**Acceptance criteria:**

- A baseline/candidate scalar experiment runs from a checked-in spec
  and emits deterministic traces.
- A checksum mismatch marks the experiment invalid and prevents ledger
  winner/loser updates.
- Re-running the same experiment with unchanged inputs references the
  same input blobs and produces replayable trace IDs.
- `wrela why --experiment <id>` shows arms, measurements, and decision.
- MIR 06-09 normal builds remain unchanged when no experiment is run.

### MIR 11 — WorldSummary and Read-Only Whole-Program Facts

The visibility layer. Passes can query whole-program facts without
receiving whole-module mutation privileges.

**Scope:**

- `WorldSummary` construction after W-MIR build and before release
  optimization.
- Call graph, type closure, effect summaries, capability graph,
  layout inventory, escape summaries, hotness seeds, region features,
  and region cost summaries.
- `wrela dump world-summary <root.wrela>` deterministic textual dump.
- `WorldFactCtx` and `WorldFactQuery` events.
- Versioned world-summary hash included in experiment traces.

**Acceptance criteria:**

- The same root produces byte-identical world-summary dumps across
  runs.
- A pass can query callgraph and effect-summary facts through
  `WorldFactCtx`; the trace records each lookup.
- Missing or stale world facts fail closed as legality failures.
- Region-local optimizer output remains byte-for-byte identical unless
  a transform explicitly consumes world facts.

### MIR 12 — RegionSetPattern and Multi-Region Transactions

The composition layer. Regions remain the atoms, but transforms can
match and edit explicit region sets transactionally.

**Scope:**

- `RegionSetPattern`, `RegionSetCandidate`, `RegionSetCtx`, and
  `WorldFactCtx` integration.
- `MultiRegionTransaction` scratch module editing.
- Boundary verifier for call edges, ABI-visible shape, authority,
  state-token order, trap order, frame/row/capacity-token escape, and
  touched/untouched region signature agreement.
- Multi-region certificates with touched-region hashes and boundary
  facts.
- `wrela why --transaction <id>` rendering.

**Acceptance criteria:**

- A deliberately failing multi-region transaction rolls back all
  touched regions and emits `MultiRegionTransactionAbandoned`.
- A no-op transaction over two regions commits and records deterministic
  before/after hashes.
- Boundary verifier rejects a transaction that changes a public call
  signature without producing a private clone.
- `wrela why --transaction <id>` renders regions, facts, certificates,
  and commit/abandon reason.

### MIR 13 — Inlining and Specialization

The first real multi-region optimization. Whole-callgraph visibility
drives selective, transactional caller/callee specialization and
inlining.

**Scope:**

- `caller + callee + call-site` `RegionSetPattern`.
- SCC/recursion classification and policy.
- Private callee cloning and call-site specialization for constants,
  enum variants, capability paths, table/layout identities, narrowed
  effects, non-escaping frames, and disjoint mutation facts.
- Transactional inlining into caller regions.
- Post-inline local optimization on touched regions.
- Per-call-edge ledger projection and `wrela why --call-edge`.

**Acceptance criteria:**

- A simple leaf call inlines transactionally and preserves output.
- A recursive or mutually recursive call edge is rejected with a
  deterministic reason.
- A specialization fixture removes a dead branch or authority check
  after cloning.
- Code-size and fuel budgets prevent an intentionally explosive inline.
- `wrela why --call-edge` explains inline, specialize-only, and reject
  outcomes.

### MIR 14 — Cross-Region Data-Plane Pipeline Fusion

The first Wrela-specific multi-region optimization family. It fuses
table/mask producer-consumer regions into lowerable data-plane
pipelines when Wrela facts prove legality.

**Scope:**

- Producer/consumer and sibling-loop `RegionSetPattern` families.
- Legality for table provenance, mask domains, row counts, row-token
  non-escape, capacity-token preservation, disjoint/ordered mutation
  places, effect subsets, state-token order, sync-token order, and
  trap order.
- Fused W-MIR shape that exposes single-pass lowerable data-plane
  loops.
- Generated-code A/B benchmark fixtures for at least one fused
  pipeline.
- Multi-region reducer support for failed fusion candidates.

**Acceptance criteria:**

- A two-stage table/mask pipeline fuses and produces matching
  checksums.
- A fixture with mismatched table provenance is rejected by legality
  facts.
- A fixture with row-token escape is rejected and explains the escape.
- A generated-code A/B fixture shows directional movement or
  inconclusive classification; it never records a winner without
  matching checksums.
- Reducer output includes the minimal region set needed to reproduce a
  fusion failure.

### MIR 15 — Schedule-As-Data and Automated Search

The search layer. Pass order, fixed-point policy, knobs, thresholds,
and cost-model weights become explicit artifacts that can be compared
and replayed.

**Scope:**

- `Schedule`, `ScheduleGroup`, `FixedPointPolicy`, `FuelPolicy`,
  `LedgerPolicy`, and `FallbackPolicy` blobs.
- Search spaces over pass enablement, pass order, fixed-point rounds,
  fuel tiers, inline thresholds, vector widths, fusion aggressiveness,
  and cost-model weight buckets.
- Grid search, random search, and successive halving.
- `CandidateConsidered`, `CandidateScored`, and `ScheduleDecision`
  events.
- `wrela search <experiment-spec>` CLI command.
- Search result summary and replay command.

**Acceptance criteria:**

- Search over a small scalar schedule space finds the seeded known-good
  schedule and records every considered candidate.
- Successive halving stops measuring candidates that fail verifier,
  checksum, or fuel gates.
- The winning candidate replays from its schedule and experiment spec.
- Search cannot update production defaults directly; it can only write
  a promotion proposal artifact.

### MIR 16 — Optimization Lab Exerciser

The scientific-instrument test. The lab proves that search can find
real signal, reject bad ideas, and say "inconclusive" when the data is
noise.

**Scope:**

- `LabSuite` blobs for scalar, callgraph, data-plane,
  memory/authority, schedule, and negative tracks.
- Seeded known-good, known-bad, regression-trap, and noise-only cases.
- `wrela lab run <suite>`, `wrela lab summarize <run>`, and
  `wrela lab replay <winner>`.
- `LabSuiteStarted` and `LabSuiteFinished` events.
- Lab summary artifacts under `target/wrela/lab-runs/<suite-id>/`.

**Acceptance criteria:**

- The scalar track rediscovers at least one seeded known-good schedule.
- The negative track rejects at least one tempting but wrong or slower
  configuration.
- The noise-only track reports inconclusive rather than a fake winner.
- A seeded regression trap produces an auto-bisect artifact.
- A winner can be replayed exactly from its lab suite and experiment
  spec.

### MIR 17 — LinUCB Online-Learning Candidate Proposal

The first learned proposer. Online learning remains inside the lab and
proposes candidates; the harness remains the judge.

**Scope:**

- Std-only LinUCB implementation with bounded model storage.
- Feature extraction from `RegionFeatureSummary`, `WorldSummary`,
  target profile, schedule hash, hotness seeds, code-size estimates,
  and ledger history.
- Action encoding for schedule choices and knob buckets.
- Reward function with runtime movement, code-size penalty, compile
  time/fuel penalty, and invalid-candidate handling.
- `SearchPolicyUpdated` event and content-addressed policy blobs.
- `wrela lab train <suite>` and `wrela lab propose <policy>`.

**Acceptance criteria:**

- LinUCB proposes a candidate from a fixed trace corpus
  deterministically.
- Model updates are replayable from recorded trace IDs.
- The learned proposer beats random search on at least one lab suite
  within a bounded evaluation budget, or reports inconclusive.
- Invalid candidates never produce positive reward.
- Normal production builds do not update model state.

### MIR 18 — TargetCostTrace and AssumptionDelta

The emitted-code feedback layer. The harness compares compiler effort,
modeled emitted work, and measured hardware behavior so failed
optimizations explain which assumption failed.

**Scope:**

- `TargetCostTrace`, `TargetCostDelta`, `OptimizationClaim`, and
  `AssumptionDelta` data structures.
- LIR/AArch64 metadata extraction for static instruction count,
  estimated dynamic instruction count, loads/stores, bytes read/written,
  branches, unpredictable branches, calls, traps/checks, vector/scalar
  ops, atomics/fences, spills/reloads, code bytes, and peak register
  pressure.
- `TargetCostEstimated`, `OptimizationClaimRecorded`, and
  `AssumptionDeltaRecorded` events.
- `wrela perf cost <root.wrela>` and `wrela why --assumption <id>`.
- Integration with experiments, search scoring, lab summaries, and
  LinUCB rewards.

**Acceptance criteria:**

- A fixture that removes instructions records the expected
  `TargetCostDelta` before runtime measurement.
- A fixture where modeled cost improves but runtime does not produces
  an `AssumptionDeltaRecorded` event with a deterministic missing-cost
  classification or `unknown`.
- `wrela why --assumption <id>` renders the optimization claim,
  observed cost delta, runtime delta, and likely missing costs.
- Search and LinUCB rewards can include target-cost movement without
  treating it as a substitute for checksum-verified runtime evidence.
- Re-running the same compile produces byte-identical target-cost
  traces.

## Non-goals and boundaries

- **Ambient whole-module mutation.** MIR 10-18 introduce whole-program
  facts and multi-region edits, but every edit still flows through an
  explicit transaction over a declared region set. There is no pass
  that receives arbitrary mutable access to the whole module.
- **Unbounded whole-program saturation.** Region-local aegraph overlays
  remain the equality mechanism. Multi-region transforms may clone,
  inline, specialize, and fuse explicit region sets; they do not build
  one global equality graph over the image.
- **Implicit cross-region legality.** Cross-region transforms must
  declare region-set candidates, world facts, boundary facts, and
  multi-region certificates. The harness does not infer legality from
  a pass author's mutation after the fact.
- **Unspecified parallel cross-region dispatch.** The design commits
  deterministic event ordering and transaction semantics. Parallel
  execution may be an implementation detail later, but visible traces
  and projections stay deterministic.
- **Dynamic pass loading.** Passes are statically linked, statically
  registered, statically dispatched. No plugin system, no
  shared-object loading.
- **External SDK.** The pass SDK is internal to the Wrela compiler.
  There is no public Transform author API consumed by third parties.
- **Silent production learning.** MIR 17's LinUCB model learns only in
  lab commands. Normal production builds consume explicit promoted
  policy blobs; they do not update model state.
- **ML as proof.** ML scores are proposal signals, never legality
  evidence. Certificates, verifier checks, checksum agreement, fuel
  budgets, and measurement gates remain mandatory.
- **Target-cost estimates as proof.** `TargetCostTrace` reduces
  dependence on wall-clock noise and improves diagnosis, but it is not
  proof of speed. Runtime claims still require checksum-verified
  measurement evidence.
- **Provability bypass tier.** Every transform — including
  `Experimental` and `Observed` — declares facts, emits certificates,
  and writes trace events. Health states relax ledger persistence,
  fuel defaults, default-enabled posture, and coverage expectations;
  they never relax proof.
- **Wall-clock as a hard build gate.** Wall-clock time is telemetry.
  Hard build outcomes use fuel.
- **Merged-mask group fingerprint approximations.** Group-level skip
  computes per-transform eligibility (with bucket dedup), not a
  merged mask.

## Open questions

These need answers in their owning phases but do not block MIR 06.

- **Fingerprint partitioning finalization.** The bit layout in this
  document is the proposed shape. MIR 07 finalizes it based on a
  survey of the retrofitted transforms' actual needs. Reserved bits
  remain a hard 23.
- **Fuel quantum sizing per transform category.** The example budget
  in the worked example is illustrative. MIR 06 measures the
  retrofitted transforms and selects per-category defaults from
  observed consumption plus headroom.
- **Probabilistic revalidation rate (`N`).** MIR 07 ships with
  `N = 50`; measurement tunes.
- **Trace retention budget.** MIR 06 defaults to last 50 runs. The
  policy may need bucket-by-category retention (long-tail
  `known-loser` traces vs short-tail `Experimental` traces) once
  measurement shows real corpus sizes.
- **Cross-target ledger keys.** The ledger key includes
  `target_profile`. When multiple target profiles ship, the ledger
  fans out. Bounded per-target storage policy is MIR 08+ work.
- **CI baseline format.** Coverage and attribution baselines are
  stored somewhere — checked into the repository, separate snapshot,
  computed from main? MIR 08 decision.
- **Transform health-state automation.** Whether the harness can
  promote `Observed` → `Candidate` automatically based on enough
  fires + non-regression, or whether the transition is always a
  deliberate human change. Likely the latter for MIR 08-09; reconsider
  later.
- **Ledger projection incrementality.** Whether the projection is
  fully rebuilt at end-of-run or incrementally updated as the trace
  appends. Performance question; MIR 07 measurement decides.
- **ExperimentSpec syntax.** MIR 10 needs a concrete text syntax for
  experiment specs that stays hand-written, deterministic, and easy to
  diff.
- **WorldSummary fact budget.** MIR 11 must choose how much whole-
  program information belongs in the initial summary before it becomes
  too large or too expensive for ordinary release builds.
- **Boundary fact catalog.** MIR 12 must finalize the exact closed set
  of boundary facts for multi-region transactions and their serialized
  certificate representation.
- **Inlining profitability defaults.** MIR 13 needs initial size,
  hotness, and fuel thresholds before the ledger has enough evidence
  to guide call-edge choices.
- **Data-plane fusion payoff threshold.** MIR 14 must define the
  default threshold for treating fused data-plane shapes as winners,
  especially when code size grows.
- **Search budget policy.** MIR 15 must define default candidate caps,
  early-stop rules, and wall-clock advisory warnings for lab commands.
- **Lab suite baselines.** MIR 16 needs seeded known-good,
  known-bad, regression-trap, and noise-only suites. The storage
  policy for those baselines (checked-in fixtures vs generated suite
  artifacts vs local lab corpus) is future work.
- **Online-learning promotion policy.** MIR 17's LinUCB policy updates
  happen in the lab. A later decision must define when, if ever, a
  learned policy can be promoted into production defaults and what
  trace evidence a promotion requires.
- **Target-cost feature calibration.** MIR 18 must choose the first
  target-cost counters and missing-cost classifications. The initial
  set should be small enough to be trustworthy and broad enough to
  explain common divergences between modeled work and wall-clock.
- **Dynamic estimate inputs.** MIR 18 needs a policy for loop trip
  counts, mask density, table row counts, and branch predictability
  when they are statically known, profiled, or unknown.

## Acceptance for this spec

This document is approved when:

- The Transform decomposition (Metadata, Eligibility, CandidateSource,
  LegalityContract, Apply) with match-local and region-local scopes is
  committed.
- The erasure model with `TransformVtable` and blanket-impl is
  committed.
- The registry as single source of truth is committed.
- The fingerprint substrate with sound per-transform group
  eligibility is committed.
- Pre-classification purity gating before aegraph admission is
  committed.
- Transactional dispatch with scratch builders and abandon-on-failure
  is committed.
- `RunTrace` as the single append-only event log with the ledger,
  coverage, attribution, and `wrela why` as projections is committed.
- The five-category `RunTrace` decomposition (Inputs, Facts,
  Decisions, Edits, Evidence) is committed as the mental model for
  completeness checking and event-kind admission.
- The content-addressed input blob store at
  `target/wrela/blobs/<hash>.bin` for pass config, target profile,
  experiment spec, lab suite, schedule, cost model, search policy,
  target-cost model, and benchmark corpus is committed.
- The reserved event variants (`RegionFeatureSummary`, the experiment
  bracket, schedule decisions, candidate consideration and scoring,
  world-fact queries, multi-region transactions, lab-suite brackets,
  search-policy updates, target-cost estimates, optimization claims,
  and assumption deltas) ship in the closed enum at MIR 06; the
  MIR 10-18 phases extending them are committed as part of this
  design's roadmap.
- The MIR 10-18 extension ladder is committed: experiments, world summaries,
  multi-region transactions, inlining/specialization, data-plane
  pipeline fusion, schedule/search, lab exercising, and LinUCB online
  proposal, target-cost tracing, and assumption-delta analysis.
- `ExperimentSpec`, `WorldSummary`, `RegionSetPattern`,
  `MultiRegionTransaction`, `Schedule`, `LabSuite`, LinUCB policy,
  `TargetCostTrace`, `OptimizationClaim`, and `AssumptionDelta`
  artifacts are committed as first-class harness concepts.
- The accountability-not-correctness principle is committed: the
  harness makes wrong things visible; authors remain responsible for
  the soundness of their transforms.
- Rule health states with full Trident retained across every state are
  committed.
- The bidirectional ledger (`known-winner`, `known-loser`,
  `needs-remeasure`, `known-no-op`) with explicit revalidation
  discipline is committed.
- Deterministic fuel as the hard budget invariant and wall-clock as
  telemetry is committed.
- Bounded delta-debugging (ddmin) for auto-bisect is committed.
- Matcher determinism, flag precedence, fact serialization, and
  deterministic dispatch / projection ordering are committed.
- The thirteen-phase delivery (MIR 06 through MIR 18) and per-phase
  acceptance criteria are committed.
- The non-goals are committed, including no ambient whole-module
  mutation, no unbounded whole-program saturation, no silent
  production learning, the absence of a provability bypass tier, and
  the prohibition on merged-mask group eligibility.

Implementation plans for MIR 06 through MIR 18 are separate documents
under [`../implementation/plans/`](../implementation/plans/), following
the wrela plan-template.
