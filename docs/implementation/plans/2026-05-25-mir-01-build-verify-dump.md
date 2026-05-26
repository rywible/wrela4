# MIR 01 Build / Verify / Dump Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first production W-MIR artifact: deterministic, verified, dumpable, parseable, and sourced from successful `CheckResult` semantic artifacts.

**Architecture:** MIR 01 creates `src/mir/` with dense ID-backed arenas, W-MIR types/effects/regions/ops, semantic fact carriers, a builder from `CheckResult`, a verifier, a deterministic text renderer/parser, and `wrela dump mir <root.wrela>`. It does not lower to LIR, emit AArch64, run generated-code benchmarks, or build release optimizer machinery.

**Tech Stack:** Rust 2024, standard library only, existing `check`, `syntax`, `source`, `diagnostic`, and `command` modules.

---

## Locked Decisions

- `mir.build` requires `CheckResult::ok() == true`. If check failed, CLI `dump mir` prints check diagnostics and exits `1`.
- MIR 01 lowers only the semantic subset that `check` already accepts without error. It does not silently accept unsupported source.
- MIR 01 lowers only the `+` binary operator. Any other parsed binary operator emits `CheckUnsupported` and the MIR build fails.
- MIR 01 must fail MIR build with a MIR diagnostic when it sees a checked expression it cannot lower. It must not lower unsupported source to `None`.
- `CheckResult` becomes the durable boundary artifact for MIR build: it retains the lexed files as compiler artifacts, exposes `lexed_files(&self) -> &[LexedFile]`, and does not clone source text.
- MIR 01 creates explicit carriers for block arguments, places, state edges, capacity tokens, ownership modes, trap provenance, and capability paths even if the first lowering fills only the facts available from the current `check` product.
- W-MIR text is deterministic and round-trip parseable.
- MIR IDs are dense typed wrappers over `u32`.
- MIR 01 telemetry records counters and phase elapsed time. Timing values are not part of deterministic dump text.
- `wrela dump mir <root.wrela>` follows the existing `dump tokens` CLI namespace.
- MIR 01 adds no external crates.

## Planned File Structure

```text
src/
  lib.rs
  command.rs
  mir/
    mod.rs
    id.rs
    effect.rs
    facts.rs
    ty.rs
    ir.rs
    build.rs
    verify.rs
    text.rs
    parse_text.rs
    report.rs
tests/
  mir.rs
fixtures/
  mir/
    basic.wrela
    data_flow.wrela
    imports/root.wrela
    imports/lib.wrela
```

## Public API Shape

```rust
pub mod mir;

pub fn mir::build_mir(check: &crate::check::CheckResult) -> mir::MirBuildResult;
pub fn mir::verify_module(module: &mir::MirModule) -> mir::VerifyResult;
pub fn mir::text::render_module(module: &mir::MirModule) -> String;
pub fn mir::parse_text::parse_module(text: &str) -> Result<mir::MirModule, mir::TextParseError>;
```

## Parallel Work Map

- Task 1 must run first.
- Task 2 must run second because it locks the shared `MirModule` data model and `src/mir/mod.rs` exports.
- Task 3 depends on Task 2 because `build_mir` imports the arena model.
- Task 4 depends on Tasks 1-3.
- Task 5 depends on Task 4.
- Task 6 depends on Tasks 2 and 5.
- Task 7 depends on Tasks 4-6.
- Task 8 is final integration and must run last.
- No two MIR 01 subagents may edit `src/mir/mod.rs`, `src/mir/ir.rs`, `src/mir/build.rs`, or `tests/mir.rs` at the same time; those files are integration-owned.

## Subagent Verification Discipline

Subagents may run focused `cargo test mir::...`, `cargo test --test mir ...`, or `cargo check --all-targets` for their task with an execution timeout. Subagents must not run the full quality gate, strict clean gates, perf loops, or unbounded commands. The orchestrator runs `./scripts/quality-gate.sh` after integrating each completed task branch.

---

### Task 1: MIR Module Skeleton, IDs, Effects, And Types

**Files:**
- Create: `src/mir/mod.rs`
- Create: `src/mir/id.rs`
- Create: `src/mir/effect.rs`
- Create: `src/mir/facts.rs`
- Create: `src/mir/ty.rs`
- Modify: `src/lib.rs`
- Test: unit tests inside the new files

**Description:** Add the public MIR module and the stable primitive types used by subsequent tasks: dense IDs, effect bitsets, access modes, scalar types, MIR types, semantic fact carriers, and source locations.

- [ ] **Step 1: Write failing unit tests for IDs and effect bitsets**

Add to `src/mir/id.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{BlockId, OperationId, RegionId, ValueId};

    #[test]
    fn ids_expose_stable_raw_values() {
        assert_eq!(RegionId::new(7).raw(), 7);
        assert_eq!(BlockId::new(8).raw(), 8);
        assert_eq!(OperationId::new(9).raw(), 9);
        assert_eq!(ValueId::new(10).raw(), 10);
    }
}
```

Add to `src/mir/effect.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{Effect, EffectSet};

    #[test]
    fn effect_sets_union_and_subset() {
        let trap = EffectSet::single(Effect::Trap);
        let io = EffectSet::single(Effect::Io);
        let both = trap.union(io);

        assert!(trap.is_subset_of(both));
        assert!(io.is_subset_of(both));
        assert!(both.contains(Effect::Trap));
        assert!(both.contains(Effect::Io));
        assert_eq!(EffectSet::empty().bits(), 0);
    }
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test mir::id::tests::ids_expose_stable_raw_values
cargo test mir::effect::tests::effect_sets_union_and_subset
```

Expected: both commands fail because `src/mir` does not exist yet.

- [ ] **Step 3: Implement ID wrappers**

Create `src/mir/id.rs`:

```rust
macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(RegionId);
id_type!(BlockId);
id_type!(OperationId);
id_type!(ValueId);
id_type!(PlaceId);
id_type!(TypeId);
```

- [ ] **Step 4: Implement effect bitsets**

Create `src/mir/effect.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Trap,
    Io,
    Volatile,
    Time,
    Entropy,
    Arena,
    Mutate,
    Dma,
    Block,
    AddressArithmetic,
    Assembly,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet(u16);

impl EffectSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn single(effect: Effect) -> Self {
        Self(1 << effect as u16)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, effect: Effect) -> bool {
        (self.0 & (1 << effect as u16)) != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_subset_of(self, other: Self) -> bool {
        (self.0 & !other.0) == 0
    }
}
```

- [ ] **Step 5: Implement semantic fact carriers**

Create `src/mir/facts.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityPath {
    class_name: String,
    path: Vec<String>,
}

impl CapabilityPath {
    pub fn new(class_name: String, path: Vec<String>) -> Self {
        Self { class_name, path }
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrapProvenance {
    reason: String,
}

impl TrapProvenance {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OperationFacts {
    ownership: Vec<OwnershipMode>,
    capabilities: Vec<CapabilityPath>,
    trap: Option<TrapProvenance>,
    state_edge: bool,
    capacity_token: bool,
}

impl OperationFacts {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn has_state_edge(&self) -> bool {
        self.state_edge
    }

    pub fn has_capacity_token(&self) -> bool {
        self.capacity_token
    }
}
```

- [ ] **Step 6: Implement MIR types**

Create `src/mir/ty.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    Bool,
    I64,
    U32,
    U64,
    String,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirType {
    Scalar(ScalarType),
    Data(String),
    LayoutData(String),
    Class(String),
    UniqueClass(String),
    Interface(String),
    Error(String),
    Image(String),
    HostImage(String),
    Capability { class_name: String, path: String },
    Table { item: String, rows: u64 },
    Column { item: String, table: String, rows: u64 },
    Mask { table: String, rows: u64 },
    RowToken { table: String },
    StateToken(StateTokenKind),
    CapacityToken { owner: String },
    Never,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateTokenKind {
    Table(String),
    Mmio(String),
    Atomic(String),
    Sync(String),
}
```

- [ ] **Step 7: Wire module exports**

Create `src/mir/mod.rs`:

```rust
pub mod effect;
pub mod facts;
pub mod id;
pub mod ty;

pub use effect::{Effect, EffectSet};
pub use facts::{CapabilityPath, OperationFacts, OwnershipMode, TrapProvenance};
pub use id::{BlockId, OperationId, PlaceId, RegionId, TypeId, ValueId};
pub use ty::{AccessMode, MirType, ScalarType, StateTokenKind};
```

Modify `src/lib.rs`:

```rust
pub mod check;
pub mod command;
pub mod diagnostic;
pub mod discover;
pub mod lexer;
pub mod mir;
pub mod source;
pub mod syntax;
```

- [ ] **Step 8: Run focused tests**

Run:

```bash
cargo test mir::id::tests::ids_expose_stable_raw_values
cargo test mir::effect::tests::effect_sets_union_and_subset
```

Expected: both tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/lib.rs src/mir/mod.rs src/mir/id.rs src/mir/effect.rs src/mir/facts.rs src/mir/ty.rs
git commit -m "feat: add MIR primitive types -Codex Automated"
```

**Acceptance criteria:**
- `wrela::mir` is public.
- ID wrappers expose stable `raw()` values.
- `EffectSet` supports empty, single, union, contains, subset, and bits.
- Semantic fact carriers exist for ownership, capability paths, trap provenance, state edges, and capacity tokens.
- No external dependencies are added.

---

### Task 2: MIR Arena, Regions, Blocks, Operations, And Reports

**Files:**
- Create: `src/mir/ir.rs`
- Create: `src/mir/report.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/ir.rs` and `src/mir/report.rs`

**Description:** Define the immutable W-MIR module shape and deterministic counters. This task does not build from source yet.

- [ ] **Step 1: Write failing tests for arena insertion and counters**

Add to `src/mir/ir.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{EffectSet, MirType, ScalarType};

    #[test]
    fn module_assigns_dense_ids() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.Foo.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let value = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        let op = module.push_operation(
            block,
            OperationData::new(OperationKind::Return, Vec::new(), vec![value], EffectSet::empty()),
        );

        assert_eq!(region.raw(), 0);
        assert_eq!(block.raw(), 0);
        assert_eq!(value.raw(), 0);
        assert_eq!(op.raw(), 0);
        assert_eq!(module.region(region).blocks(), &[block]);
        assert_eq!(module.block(block).operations(), &[op]);
    }
}
```

Add to `src/mir/report.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::MirReport;

    #[test]
    fn report_counts_are_stable() {
        let report = MirReport::new(2, 3, 5, 8, 13, 21, 34);
        assert_eq!(report.regions(), 2);
        assert_eq!(report.blocks(), 3);
        assert_eq!(report.operations(), 5);
        assert_eq!(report.values(), 8);
        assert_eq!(report.places(), 13);
        assert_eq!(report.state_tokens(), 21);
        assert_eq!(report.capacity_tokens(), 34);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::ir::tests::module_assigns_dense_ids
cargo test mir::report::tests::report_counts_are_stable
```

Expected: both fail because `ir` and `report` are not implemented.

- [ ] **Step 3: Implement IR arena types**

Create `src/mir/ir.rs` with these public types and accessors:

```rust
use crate::source::Span;

use super::{BlockId, EffectSet, MirType, OperationFacts, OperationId, PlaceId, RegionId, ValueId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionKind {
    Lambda { symbol: String },
    Gamma { label: String },
    Theta(ThetaKind),
    Delta { label: String },
    Omega { symbol: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThetaKind {
    Repeat,
    For,
    Drain,
    Reduce,
    Scan,
    Loop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionData {
    kind: RegionKind,
    blocks: Vec<BlockId>,
    source: Option<Span>,
}

impl RegionData {
    pub fn lambda(symbol: String) -> Self {
        Self {
            kind: RegionKind::Lambda { symbol },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn omega(symbol: String) -> Self {
        Self {
            kind: RegionKind::Omega { symbol },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    pub fn kind(&self) -> &RegionKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockData {
    name: String,
    arguments: Vec<ValueId>,
    operations: Vec<OperationId>,
}

impl BlockData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            arguments: Vec::new(),
            operations: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arguments(&self) -> &[ValueId] {
        &self.arguments
    }

    pub fn operations(&self) -> &[OperationId] {
        &self.operations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueData {
    ty: MirType,
    source: Option<Span>,
}

impl ValueData {
    pub fn new(ty: MirType) -> Self {
        Self { ty, source: None }
    }

    pub fn ty(&self) -> &MirType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceData {
    name: String,
    ty: MirType,
}

impl PlaceData {
    pub fn new(name: String, ty: MirType) -> Self {
        Self { name, ty }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> &MirType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Literal(String),
    ReadValue(String),
    Binary(String),
    Let(String),
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationData {
    kind: OperationKind,
    operands: Vec<ValueId>,
    results: Vec<ValueId>,
    effects: EffectSet,
    facts: OperationFacts,
}

impl OperationData {
    pub fn new(
        kind: OperationKind,
        operands: Vec<ValueId>,
        results: Vec<ValueId>,
        effects: EffectSet,
    ) -> Self {
        Self {
            kind,
            operands,
            results,
            effects,
            facts: OperationFacts::empty(),
        }
    }

    pub fn with_facts(mut self, facts: OperationFacts) -> Self {
        self.facts = facts;
        self
    }

    pub fn kind(&self) -> &OperationKind {
        &self.kind
    }

    pub fn operands(&self) -> &[ValueId] {
        &self.operands
    }

    pub fn results(&self) -> &[ValueId] {
        &self.results
    }

    pub fn effects(&self) -> EffectSet {
        self.effects
    }

    pub fn facts(&self) -> &OperationFacts {
        &self.facts
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirModule {
    regions: Vec<RegionData>,
    blocks: Vec<BlockData>,
    operations: Vec<OperationData>,
    values: Vec<ValueData>,
    places: Vec<PlaceData>,
}

impl MirModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_region(&mut self, data: RegionData) -> RegionId {
        let id = RegionId::new(self.regions.len() as u32);
        self.regions.push(data);
        id
    }

    pub fn push_block(&mut self, region: RegionId, data: BlockData) -> BlockId {
        let id = BlockId::new(self.blocks.len() as u32);
        self.blocks.push(data);
        self.regions[region.raw() as usize].blocks.push(id);
        id
    }

    pub fn push_block_argument(&mut self, block: BlockId, value: ValueId) {
        self.blocks[block.raw() as usize].arguments.push(value);
    }

    pub fn push_operation(&mut self, block: BlockId, data: OperationData) -> OperationId {
        let id = OperationId::new(self.operations.len() as u32);
        self.operations.push(data);
        self.blocks[block.raw() as usize].operations.push(id);
        id
    }

    pub fn push_value(&mut self, data: ValueData) -> ValueId {
        let id = ValueId::new(self.values.len() as u32);
        self.values.push(data);
        id
    }

    pub fn push_place(&mut self, data: PlaceData) -> PlaceId {
        let id = PlaceId::new(self.places.len() as u32);
        self.places.push(data);
        id
    }

    pub fn region(&self, id: RegionId) -> &RegionData {
        &self.regions[id.raw() as usize]
    }

    pub fn block(&self, id: BlockId) -> &BlockData {
        &self.blocks[id.raw() as usize]
    }

    pub fn operation(&self, id: OperationId) -> &OperationData {
        &self.operations[id.raw() as usize]
    }

    pub fn value(&self, id: ValueId) -> &ValueData {
        &self.values[id.raw() as usize]
    }

    pub fn regions(&self) -> &[RegionData] {
        &self.regions
    }

    pub fn blocks(&self) -> &[BlockData] {
        &self.blocks
    }

    pub fn operations(&self) -> &[OperationData] {
        &self.operations
    }

    pub fn values(&self) -> &[ValueData] {
        &self.values
    }

    pub fn places(&self) -> &[PlaceData] {
        &self.places
    }
}
```

- [ ] **Step 4: Implement MIR report counters**

Create `src/mir/report.rs`:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirReport {
    regions: usize,
    blocks: usize,
    operations: usize,
    values: usize,
    places: usize,
    state_tokens: usize,
    capacity_tokens: usize,
}

impl MirReport {
    pub const fn new(
        regions: usize,
        blocks: usize,
        operations: usize,
        values: usize,
        places: usize,
        state_tokens: usize,
        capacity_tokens: usize,
    ) -> Self {
        Self {
            regions,
            blocks,
            operations,
            values,
            places,
            state_tokens,
            capacity_tokens,
        }
    }

    pub const fn regions(&self) -> usize { self.regions }
    pub const fn blocks(&self) -> usize { self.blocks }
    pub const fn operations(&self) -> usize { self.operations }
    pub const fn values(&self) -> usize { self.values }
    pub const fn places(&self) -> usize { self.places }
    pub const fn state_tokens(&self) -> usize { self.state_tokens }
    pub const fn capacity_tokens(&self) -> usize { self.capacity_tokens }
}
```

- [ ] **Step 5: Export modules**

Modify `src/mir/mod.rs` by adding the `ir` and `report` exports to the Task 1 file. Keep the existing `facts` module and `facts` public exports; do not replace the whole file with a shorter export list.

```rust
pub mod effect;
pub mod facts;
pub mod id;
pub mod ir;
pub mod report;
pub mod ty;

pub use effect::{Effect, EffectSet};
pub use facts::{CapabilityPath, OperationFacts, OwnershipMode, TrapProvenance};
pub use id::{BlockId, OperationId, PlaceId, RegionId, TypeId, ValueId};
pub use ir::{
    BlockData, MirModule, OperationData, OperationKind, PlaceData, RegionData, RegionKind,
    ThetaKind, ValueData,
};
pub use report::MirReport;
pub use ty::{AccessMode, MirType, ScalarType, StateTokenKind};
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test mir::ir::tests::module_assigns_dense_ids
cargo test mir::report::tests::report_counts_are_stable
```

Expected: both tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/ir.rs src/mir/report.rs
git commit -m "feat: add MIR arena and counters -Codex Automated"
```

**Acceptance criteria:**
- `MirModule` stores regions, blocks, operations, values, and places in dense deterministic order.
- Region and block child lists preserve insertion order.
- `MirReport` exposes deterministic counters.
- No external dependencies are added.

---

### Task 3: MIR Build Entry Point And Check Precondition

**Files:**
- Create: `src/mir/build.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/check/mod.rs`
- Modify: `src/discover.rs`
- Test: `tests/mir.rs`
- Fixture: `fixtures/mir/basic.wrela`

**Description:** Add `build_mir(&CheckResult)` and define the check-success boundary. This task creates module-level lambda/omega regions and refuses failed `check` results.

- [ ] **Step 1: Create fixture**

Create `fixtures/mir/basic.wrela`:

```wrela
module app.main

pub data Bytes {
    value: U32
}

pub class Worker {
    fn run(read self) -> None {
        return None
    }
}

pub unique class MacOSHost {}

pub host image Main {
    phase run(host: unique MacOSHost) {
        return None
    }
}
```

- [ ] **Step 2: Write failing integration tests**

Create `tests/mir.rs`:

```rust
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn mir_build_requires_successful_check() {
    let dir = std::env::temp_dir().join("wrela-mir-build-failed-check");
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.join("root.wrela");
    std::fs::write(&root, "module root\npub data Broken { field: }\n").unwrap();

    let check = wrela::check::check_root(&root);
    assert!(!check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(!mir.ok());
    assert!(mir.module().is_none());
    assert_eq!(mir.diagnostics().len(), check.diagnostics().len());
}

#[test]
fn mir_build_creates_lambda_and_omega_regions() {
    let check = wrela::check::check_root(fixture("basic.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let module = mir.module().expect("valid MIR module");

    assert_eq!(mir.report().regions(), 3);
    let region_symbols = module
        .regions()
        .iter()
        .filter_map(|region| match region.kind() {
            wrela::mir::RegionKind::Lambda { symbol }
            | wrela::mir::RegionKind::Omega { symbol } => Some(symbol.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(region_symbols.contains(&"app.main.Worker.run"));
    assert!(region_symbols.contains(&"app.main.Main"));
    assert!(region_symbols.contains(&"app.main.Main.run"));
}
```

- [ ] **Step 3: Run test and verify failure**

Run:

```bash
cargo test --test mir mir_build_requires_successful_check
cargo test --test mir mir_build_creates_lambda_and_omega_regions
```

Expected: both fail because `build_mir` and the MIR region constructors are not implemented yet.

- [ ] **Step 4: Retain lexed artifacts in `CheckResult`**

Modify `src/discover.rs`:

```rust
impl DiscoverResult {
    pub fn into_lexed_files(self) -> Vec<LexedFile> {
        self.lexed_files
    }
}
```

Modify `src/check/mod.rs`:

```rust
pub struct CheckResult {
    source_map: SourceMap,
    lexed_files: Vec<LexedFile>,
    parsed: Vec<ParsedSyntax>,
    // existing fields...
}

impl CheckResult {
    pub fn lexed_files(&self) -> &[LexedFile] {
        &self.lexed_files
    }
}
```

At the end of `check_root`, after `build_semantic_report` and immediately before constructing `CheckResult`, move the lexed files into the result. The local variable is named `discovered`; this edit must happen after the last call to `discovered.lexed_files()`.

Replace the current tail:

```rust
let semantic_report = build_semantic_report(
    &summaries,
    ownership_check.diagnostics().len(),
    effect_check.diagnostics().len(),
    layout_check.diagnostics().len(),
);

CheckResult {
    source_map,
    parsed,
    summaries,
    resolved_graph,
    signature_check,
    body_check,
    ownership_check,
    effect_check,
    layout_check,
    semantic_report,
    diagnostics,
}
```

with:

```rust
let semantic_report = build_semantic_report(
    &summaries,
    ownership_check.diagnostics().len(),
    effect_check.diagnostics().len(),
    layout_check.diagnostics().len(),
);

let lexed_files = discovered.into_lexed_files();

CheckResult {
    source_map,
    lexed_files,
    parsed,
    summaries,
    resolved_graph,
    signature_check,
    body_check,
    ownership_check,
    effect_check,
    layout_check,
    semantic_report,
    diagnostics,
}
```

This moves token/trivia artifacts without cloning source text. Do not add a public `ModuleInput` accessor; MIR build owns its own deterministic maps from `CheckResult`.

- [ ] **Step 5: Implement build result and precondition**

Create `src/mir/build.rs`:

```rust
use crate::check::CheckResult;
use crate::diagnostic::Diagnostic;

use super::{BlockData, MirModule, MirReport, RegionData};

#[derive(Clone, Debug)]
pub struct MirBuildResult {
    module: Option<MirModule>,
    diagnostics: Vec<Diagnostic>,
    report: MirReport,
}

impl MirBuildResult {
    pub fn failed(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            module: None,
            diagnostics,
            report: MirReport::default(),
        }
    }

    pub fn new(module: MirModule, report: MirReport) -> Self {
        Self {
            module: Some(module),
            diagnostics: Vec::new(),
            report,
        }
    }

    pub fn module(&self) -> Option<&MirModule> {
        self.module.as_ref()
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn report(&self) -> &MirReport {
        &self.report
    }

    pub fn ok(&self) -> bool {
        self.module.is_some() && self.diagnostics.is_empty()
    }
}

pub fn build_mir(check: &CheckResult) -> MirBuildResult {
    if !check.ok() {
        return MirBuildResult::failed(check.diagnostics().to_vec());
    }

    let mut module = MirModule::new();

    for summary in check.summaries() {
        let module_path = summary.module_path().as_dotted();
        for item in summary.items() {
            match item.kind() {
                crate::check::summary::ItemKind::Image
                | crate::check::summary::ItemKind::HostImage => {
                    let symbol = format!("{module_path}.{}", item.name().text());
                    module.push_region(RegionData::omega(symbol));
                }
                _ => {}
            }

            for member in item.members() {
                if member.body().is_some() {
                    let member_name = member
                        .name()
                        .map(|name| name.text().to_string())
                        .unwrap_or_else(|| "constructor".to_string());
                    let symbol = format!(
                        "{module_path}.{}.{}",
                        item.name().text(),
                        member_name
                    );
                    let region = module.push_region(RegionData::lambda(symbol));
                    module.push_block(region, BlockData::new("entry".to_string()));
                }
            }
        }
    }

    let report = MirReport::new(
        module.regions().len(),
        module.blocks().len(),
        module.operations().len(),
        module.values().len(),
        module.places().len(),
        0,
        0,
    );
    MirBuildResult::new(module, report)
}
```

- [ ] **Step 6: Export builder**

Modify `src/mir/mod.rs`:

```rust
pub mod build;
pub mod effect;
pub mod id;
pub mod ir;
pub mod report;
pub mod ty;

pub use build::{MirBuildResult, build_mir};
```

Keep the existing exports from prior tasks below the new build export.

- [ ] **Step 7: Run focused passing test**

Run:

```bash
cargo test --test mir mir_build_requires_successful_check
cargo test --test mir mir_build_creates_lambda_and_omega_regions
```

Expected: both pass.

- [ ] **Step 8: Commit**

```bash
git add src/discover.rs src/check/mod.rs src/mir/mod.rs src/mir/build.rs tests/mir.rs fixtures/mir/basic.wrela
git commit -m "feat: add MIR build entry point -Codex Automated"
```

**Acceptance criteria:**
- `build_mir` returns no module for failed check results and preserves check diagnostics.
- Valid checked source produces lambda regions for members with bodies and omega regions for image roots.
- The task does not add W-MIR body lowering yet.

---

### Task 4: MIR Body Lowering For Current Checked Subset

**Files:**
- Modify: `src/mir/build.rs`
- Modify: `src/mir/ir.rs`
- Modify: `src/check/cst.rs`
- Test: `tests/mir.rs`
- Fixture: `fixtures/mir/data_flow.wrela`

**Description:** Lower the expression and statement forms currently accepted by `check`: empty returns, `return` with literals/names/binary expressions, `let` with initializer, and expression statements for checked expressions. Source unsupported by `check` never reaches this task because `build_mir` requires a successful check.

- [ ] **Step 1: Add data-flow fixture**

Create `fixtures/mir/data_flow.wrela`:

```wrela
module app.flow

pub class Calculator {
    fn add(read self, read left: U32, read right: U32) -> U32 {
        let sum: U32 = left + right
        return sum
    }
}
```

- [ ] **Step 2: Add failing body-lowering test**

Append to `tests/mir.rs`:

```rust
#[test]
fn mir_build_lowers_checked_let_binary_and_return() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());

    assert!(text.contains("op read_value left"));
    assert!(text.contains("op read_value right"));
    assert!(text.contains("op binary +"));
    assert!(text.contains("op let sum"));
    assert!(text.contains("op return"));
}
```

- [ ] **Step 3: Run test and verify failure**

Run:

```bash
cargo test --test mir mir_build_lowers_checked_let_binary_and_return
```

Expected: fail because body operations are not lowered.

- [ ] **Step 4: Extend operation kinds for stable dump text**

Modify `src/mir/ir.rs` `OperationKind`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Literal(String),
    ReadValue(String),
    Binary(String),
    Let(String),
    Return,
}
```

If this exact shape already exists from Task 2, keep it unchanged.

- [ ] **Step 5: Implement CST body lowering helpers**

In `src/mir/build.rs`, add imports:

```rust
use std::collections::BTreeMap;

use crate::check::cst::CstView;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::{LexedFile, Punct, TokenKind};
use crate::source::SourceFile;
use crate::syntax::{ParsedSyntax, SyntaxKind, SyntaxNodeId};

use super::{EffectSet, MirType, OperationData, OperationKind, ScalarType, ValueData};
```

Add a local lowering context:

```rust
struct BodyLowerer<'a> {
    view: CstView<'a>,
    module: &'a mut MirModule,
    block: super::BlockId,
    locals: BTreeMap<String, super::ValueId>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> BodyLowerer<'a> {
    fn new(
        parsed: &'a ParsedSyntax,
        lexed: &'a LexedFile,
        source: &'a SourceFile,
        module: &'a mut MirModule,
        block: super::BlockId,
        locals: BTreeMap<String, super::ValueId>,
    ) -> Self {
        Self {
            view: CstView::new(parsed.tree(), lexed, source),
            module,
            block,
            locals,
            diagnostics: Vec::new(),
        }
    }

    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn lower_block(&mut self, body: SyntaxNodeId) {
        for stmt in self.view.child_nodes(body) {
            match self.view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.lower_let(stmt),
                SyntaxKind::ReturnStmt => self.lower_return(stmt),
                SyntaxKind::ExprStmt => {
                    if let Some(expr) = first_expr_child(&self.view, stmt) {
                        self.lower_expr(expr);
                    }
                }
                _ => {}
            }
        }
    }

    fn lower_let(&mut self, stmt: SyntaxNodeId) {
        let Some(name) = self.view.first_identifier_text(stmt) else {
            return;
        };
        let Some(expr) = first_expr_child(&self.view, stmt) else {
            return;
        };
        let value = self.lower_expr(expr);
        self.locals.insert(name.text().to_string(), value);
        self.module.push_operation(
            self.block,
            OperationData::new(
                OperationKind::Let(name.text().to_string()),
                vec![value],
                Vec::new(),
                EffectSet::empty(),
            ),
        );
    }

    fn lower_return(&mut self, stmt: SyntaxNodeId) {
        let operands = first_expr_child(&self.view, stmt)
            .map(|expr| vec![self.lower_expr(expr)])
            .unwrap_or_default();
        self.module.push_operation(
            self.block,
            OperationData::new(OperationKind::Return, operands, Vec::new(), EffectSet::empty()),
        );
    }

    fn lower_expr(&mut self, expr: SyntaxNodeId) -> super::ValueId {
        match self.view.node_kind(expr) {
            SyntaxKind::LiteralExpr => self.lower_literal(expr),
            SyntaxKind::NameExpr => self.lower_name(expr),
            SyntaxKind::ParenExpr => first_expr_child(&self.view, expr)
                .map(|child| self.lower_expr(child))
                .unwrap_or_else(|| self.none_value()),
            SyntaxKind::BinaryExpr => self.lower_binary(expr),
            _ => self.unsupported_expr(expr),
        }
    }

    fn lower_literal(&mut self, expr: SyntaxNodeId) -> super::ValueId {
        let text = self
            .view
            .child_tokens(expr)
            .first()
            .map(|token| self.view.token_text(*token).text().to_string())
            .unwrap_or_else(|| "None".to_string());
        let ty = if text == "None" {
            MirType::Scalar(ScalarType::None)
        } else if text == "true" || text == "false" {
            MirType::Scalar(ScalarType::Bool)
        } else if text.starts_with('"') {
            MirType::Scalar(ScalarType::String)
        } else {
            MirType::Scalar(ScalarType::I64)
        };
        let value = self.module.push_value(ValueData::new(ty));
        self.module.push_operation(
            self.block,
            OperationData::new(
                OperationKind::Literal(text),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
            ),
        );
        value
    }

    fn lower_name(&mut self, expr: SyntaxNodeId) -> super::ValueId {
        let Some(name) = self.view.first_identifier_text(expr) else {
            return self.none_value();
        };
        match name.text() {
            "None" => return self.lower_builtin_name("None", ScalarType::None),
            "true" | "false" => return self.lower_builtin_name(name.text(), ScalarType::Bool),
            _ => {}
        }
        if let Some(value) = self.locals.get(name.text()).copied() {
            return value;
        }
        let value = self.module.push_value(ValueData::new(MirType::Unknown));
        self.module.push_operation(
            self.block,
            OperationData::new(
                OperationKind::ReadValue(name.text().to_string()),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
            ),
        );
        self.locals.insert(name.text().to_string(), value);
        value
    }

    fn lower_builtin_name(&mut self, text: &str, scalar: ScalarType) -> super::ValueId {
        let value = self
            .module
            .push_value(ValueData::new(MirType::Scalar(scalar)));
        self.module.push_operation(
            self.block,
            OperationData::new(
                OperationKind::Literal(text.to_string()),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
            ),
        );
        value
    }

    fn lower_binary(&mut self, expr: SyntaxNodeId) -> super::ValueId {
        let children = self
            .view
            .child_nodes(expr)
            .into_iter()
            .filter(|child| is_expr_kind(self.view.node_kind(*child)))
            .collect::<Vec<_>>();
        if children.len() != 2 {
            return self.none_value();
        }
        let left = self.lower_expr(children[0]);
        let right = self.lower_expr(children[1]);
        let operator = binary_operator(&self.view, expr).unwrap_or_else(|| {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::CheckUnsupported,
                    "MIR 01 cannot lower binary expression without an operator",
                )
                .primary(self.view.node_span(expr), "missing binary operator")
                .finish(),
            );
            "+".to_string()
        });
        let value = self.module.push_value(ValueData::new(MirType::Scalar(ScalarType::I64)));
        self.module.push_operation(
            self.block,
            OperationData::new(
                OperationKind::Binary(operator),
                vec![left, right],
                vec![value],
                EffectSet::empty(),
            ),
        );
        value
    }

    fn none_value(&mut self) -> super::ValueId {
        self.module
            .push_value(ValueData::new(MirType::Scalar(ScalarType::None)))
    }

    fn unsupported_expr(&mut self, expr: SyntaxNodeId) -> super::ValueId {
        let kind = self.view.node_kind(expr);
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::CheckUnsupported,
                format!("MIR 01 cannot lower {kind:?}"),
            )
            .primary(self.view.node_span(expr), "unsupported in MIR 01")
            .finish(),
        );
        self.module.push_value(ValueData::new(MirType::Unknown))
    }
}
```

Add local helpers:

```rust
fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .find(|child| is_expr_kind(view.node_kind(*child)))
}

fn binary_operator(view: &CstView<'_>, node: SyntaxNodeId) -> Option<String> {
    for token in view.child_tokens(node) {
        let syntax_token = view.tree().token(token);
        let raw = view.lexed().tokens()[syntax_token.token().raw() as usize];
        if let TokenKind::Punct(Punct::Plus) = raw.kind() {
            return Some("+".to_string());
        }
    }
    None
}

fn is_expr_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LiteralExpr
            | SyntaxKind::NameExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::BinaryExpr
    )
}
```

Add the small accessors used by `binary_operator` to `src/check/cst.rs`:

```rust
impl<'a> CstView<'a> {
    pub fn tree(&self) -> &'a SyntaxTree {
        self.tree
    }

    pub fn lexed(&self) -> &'a LexedFile {
        self.lexed
    }
}
```

- [ ] **Step 6: Connect body lowering to module inputs**

In `build_mir`, build maps from `file_id` to parsed/source/lexed data using `check.parsed()` and `check.source_map()`. Reuse the current `CheckResult` accessors. When a member has a body and the corresponding parsed/source/lexed file exists, create the region and entry block, then run `BodyLowerer::new(...).lower_block(body)`.

The connection must keep deterministic summary order:

```rust
let parsed_by_file = check
    .parsed()
    .iter()
    .map(|parsed| (parsed.file_id(), parsed))
    .collect::<BTreeMap<_, _>>();
```

Build the lexed map from the accessor added in Task 3:

```rust
let lexed_by_file = check
    .lexed_files()
    .iter()
    .map(|lexed| (lexed.file_id(), lexed))
    .collect::<BTreeMap<_, _>>();
```

Add the source map:

```rust
let source_by_file = check
    .source_map()
    .files()
    .iter()
    .map(|source| (source.id(), source))
    .collect::<BTreeMap<_, _>>();
```

Iterate summaries in explicit deterministic order before lowering bodies:

```rust
let mut summaries = check.summaries().iter().collect::<Vec<_>>();
summaries.sort_by(|left, right| {
    left.file_id()
        .raw()
        .cmp(&right.file_id().raw())
        .then(left.module_path().as_dotted().cmp(&right.module_path().as_dotted()))
});

let mut mir_diagnostics = Vec::new();

for summary in summaries {
    let Some(parsed) = parsed_by_file.get(&summary.file_id()).copied() else {
        continue;
    };
    let Some(lexed) = lexed_by_file.get(&summary.file_id()).copied() else {
        continue;
    };
    let Some(source) = source_by_file.get(&summary.file_id()).copied() else {
        continue;
    };
    let module_path = summary.module_path().as_dotted();

    for item in summary.items() {
        for member in item.members() {
            let Some(body) = member.body() else {
                continue;
            };
            let member_name = member
                .name()
                .map(|name| name.text().to_string())
                .unwrap_or_else(|| "constructor".to_string());
            let symbol = format!("{module_path}.{}.{}", item.name().text(), member_name);
            let region = module.push_region(RegionData::lambda(symbol));
            let block = module.push_block(region, BlockData::new("entry".to_string()));
            let mut locals = BTreeMap::new();

            if let Some(signature) = check.signature_check().member_signature(member) {
                for param in signature.params() {
                    let value = module.push_value(ValueData::new(mir_type_for_checked_type(
                        check.signature_check().type_table(),
                        param.ty(),
                    )));
                    module.push_block_argument(block, value);
                    locals.insert(param.name().text().to_string(), value);
                }
            }

            let mut lowerer = BodyLowerer::new(parsed, lexed, source, &mut module, block, locals);
            lowerer.lower_block(body);
            mir_diagnostics.extend(lowerer.into_diagnostics());
        }
    }
}

if !mir_diagnostics.is_empty() {
    return MirBuildResult::failed(mir_diagnostics);
}
```

Add the type mapper used above:

```rust
fn mir_type_for_checked_type(
    table: &crate::check::types::TypeTable,
    id: crate::check::types::TypeId,
) -> MirType {
    match table.kind(id) {
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::Bool)) => {
            MirType::Scalar(ScalarType::Bool)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::I64)) => {
            MirType::Scalar(ScalarType::I64)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::U32)) => {
            MirType::Scalar(ScalarType::U32)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::U64)) => {
            MirType::Scalar(ScalarType::U64)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::String)) => {
            MirType::Scalar(ScalarType::String)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::None)) => {
            MirType::Scalar(ScalarType::None)
        }
        _ => MirType::Unknown,
    }
}
```

This replaces the body-loop skeleton from Task 3. Keep the image/omega creation logic in the same deterministic summary loop. Do not expose `ModuleInput` publicly and do not clone source text.

- [ ] **Step 7: Run focused test**

Run:

```bash
cargo test --test mir mir_build_lowers_checked_let_binary_and_return
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add src/check/cst.rs src/mir/build.rs src/mir/ir.rs tests/mir.rs fixtures/mir/data_flow.wrela
git commit -m "feat: lower checked bodies to MIR -Codex Automated"
```

**Acceptance criteria:**
- Let bindings, returns, literals, names, parenthesized expressions, and binary expressions lower to W-MIR operations.
- Binary operators preserve the parsed operator text for the supported current operator set.
- Checked-but-unsupported expression kinds produce MIR diagnostics and no MIR module.
- MIR build still refuses failed check results.
- No unsupported source is accepted by MIR if `check` rejected it.

---

### Task 5: MIR Verifier

**Files:**
- Create: `src/mir/verify.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/verify.rs`
- Test: `tests/mir.rs`

**Description:** Add a verifier that checks the MIR invariants present in MIR 01: ID validity, region/block ownership, operation operand/result ID validity, return terminator placement, and effect summary consistency for non-nested operations.

- [ ] **Step 1: Write failing verifier tests**

Create `src/mir/verify.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::verify_module;
    use crate::mir::{
        BlockData, EffectSet, MirModule, MirType, OperationData, OperationKind, RegionData,
        ScalarType, ValueData, ValueId,
    };

    #[test]
    fn verifier_accepts_minimal_returning_lambda() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let none = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(OperationKind::Return, vec![none], Vec::new(), EffectSet::empty()),
        );

        assert!(verify_module(&module).ok());
    }

    #[test]
    fn verifier_rejects_missing_operand_definition() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![ValueId::new(99)],
                Vec::new(),
                EffectSet::empty(),
            ),
        );

        let result = verify_module(&module);
        assert!(!result.ok());
        assert!(result.messages()[0].contains("unknown value v99"));
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::verify::tests
```

Expected: fail because verifier is not implemented.

- [ ] **Step 3: Implement verifier result and checks**

Implement `src/mir/verify.rs`:

```rust
use std::collections::{BTreeMap, BTreeSet};

use super::{BlockId, MirModule, OperationId, OperationKind, RegionKind, ValueId};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifyResult {
    messages: Vec<String>,
}

impl VerifyResult {
    pub fn ok(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    fn push(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }
}

pub fn verify_module(module: &MirModule) -> VerifyResult {
    let mut result = VerifyResult::default();
    let block_count = module.blocks().len() as u32;
    let operation_count = module.operations().len() as u32;
    let value_count = module.values().len() as u32;
    let mut owned_blocks = BTreeSet::new();
    let mut value_producers = BTreeMap::<ValueId, OperationId>::new();

    for (region_index, region) in module.regions().iter().enumerate() {
        if region.blocks().is_empty() {
            result.push(format!("region r{region_index} has no blocks"));
        }
        for block in region.blocks() {
            if block.raw() >= block_count {
                result.push(format!("region r{region_index} owns unknown block b{}", block.raw()));
                continue;
            }
            if !owned_blocks.insert(*block) {
                result.push(format!("block b{} is owned by multiple regions", block.raw()));
            }
        }
    }

    for (block_index, block) in module.blocks().iter().enumerate() {
        let block_id = BlockId::new(block_index as u32);
        if !owned_blocks.contains(&block_id) {
            result.push(format!("block b{block_index} is not owned by any region"));
        }

        let mut seen_return = false;
        let mut local_defs = block.arguments().iter().copied().collect::<BTreeSet<_>>();

        for op_id in block.operations() {
            if op_id.raw() >= operation_count {
                result.push(format!("block b{block_index} contains unknown operation o{}", op_id.raw()));
                continue;
            }
            let op = module.operation(*op_id);
            if seen_return {
                result.push(format!("operation after return in block b{block_index}"));
            }
            for operand in op.operands() {
                verify_value(*operand, value_count, &mut result);
                if operand.raw() < value_count && !local_defs.contains(operand) {
                    result.push(format!(
                        "operand v{} is used before definition in block b{block_index}",
                        operand.raw()
                    ));
                }
            }
            for output in op.results() {
                verify_value(*output, value_count, &mut result);
                if value_producers.insert(*output, *op_id).is_some() {
                    result.push(format!("value v{} has multiple producers", output.raw()));
                }
                local_defs.insert(*output);
            }
            if op.facts().has_state_edge() && op.effects().bits() == 0 {
                result.push(format!(
                    "operation o{} has state-edge facts but empty effect set",
                    op_id.raw()
                ));
            }
            if matches!(op.kind(), OperationKind::Return) {
                seen_return = true;
            }
        }
    }

    for (region_index, region) in module.regions().iter().enumerate() {
        if matches!(region.kind(), RegionKind::Lambda { .. }) {
            for block in region.blocks() {
                if block.raw() < block_count {
                    let data = module.block(*block);
                    let ends_in_return = data
                        .operations()
                        .last()
                        .map(|op_id| {
                            op_id.raw() < operation_count
                                && matches!(module.operation(*op_id).kind(), OperationKind::Return)
                        })
                        .unwrap_or(false);
                    if !ends_in_return {
                        result.push(format!(
                            "lambda region r{region_index} block b{} does not end in return",
                            block.raw()
                        ));
                    }
                }
            }
        }
    }

    result
}

fn verify_value(value: ValueId, value_count: u32, result: &mut VerifyResult) {
    if value.raw() >= value_count {
        result.push(format!("unknown value v{}", value.raw()));
    }
}
```

- [ ] **Step 4: Export verifier**

Modify `src/mir/mod.rs`:

```rust
pub mod verify;

pub use verify::{VerifyResult, verify_module};
```

Keep the existing module declarations and exports.

- [ ] **Step 5: Add integration verifier test**

Append to `tests/mir.rs`:

```rust
#[test]
fn mir_build_output_verifies() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let verification = wrela::mir::verify_module(mir.module().unwrap());
    assert!(verification.ok(), "{:?}", verification.messages());
}
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test mir::verify::tests
cargo test --test mir mir_build_output_verifies
```

Expected: both pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/verify.rs tests/mir.rs
git commit -m "feat: verify MIR invariants -Codex Automated"
```

**Acceptance criteria:**
- Verifier rejects invalid value references.
- Verifier rejects operations after return.
- Verifier rejects empty regions.
- Verifier rejects operation IDs that are outside the operation arena.
- Verifier rejects blocks that are unowned or owned by multiple regions.
- Verifier rejects values with multiple producers.
- Verifier rejects use-before-definition within the current MIR 01 single-block lambda shape.
- Verifier rejects lambda blocks that do not end in `return`.
- Verifier rejects state-edge fact metadata with an empty effect set.
- Valid `mir.build` output verifies.

---

### Task 6: Deterministic Text Dump And Round-Trip Parser

**Files:**
- Create: `src/mir/text.rs`
- Create: `src/mir/parse_text.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/text.rs` and `src/mir/parse_text.rs`
- Test: `tests/mir.rs`

**Description:** Add a deterministic text format and a parser for that format. The MIR 01 parser only needs to parse the format emitted by `render_module`; it must reject malformed text with structured errors instead of panicking.

- [ ] **Step 1: Write failing text tests**

Create `src/mir/text.rs` with:

```rust
#[cfg(test)]
mod tests {
    use crate::mir::{
        BlockData, EffectSet, MirModule, MirType, OperationData, OperationKind, RegionData,
        ScalarType, ValueData,
    };

    #[test]
    fn text_dump_is_stable_for_minimal_module() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let none = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(OperationKind::Return, vec![none], Vec::new(), EffectSet::empty()),
        );

        let text = super::render_module(&module);
        assert!(text.contains("wmir v1"));
        assert!(text.contains("region r0 lambda root.F.run"));
        assert!(text.contains("block b0 entry"));
        assert!(text.contains("op o0 return v0"));
    }
}
```

Create `src/mir/parse_text.rs` with:

```rust
#[cfg(test)]
mod tests {
    use crate::mir::{
        BlockData, EffectSet, MirModule, MirType, OperationData, OperationKind, RegionData,
        ScalarType, ValueData,
    };

    #[test]
    fn parser_round_trips_rendered_module() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let none = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(OperationKind::Return, vec![none], Vec::new(), EffectSet::empty()),
        );

        let text = crate::mir::text::render_module(&module);
        let parsed = super::parse_module(&text).expect("round-trip parse succeeds");
        assert_eq!(crate::mir::text::render_module(&parsed), text);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::text::tests::text_dump_is_stable_for_minimal_module
cargo test mir::parse_text::tests::parser_round_trips_rendered_module
```

Expected: fail because text modules are not implemented.

- [ ] **Step 3: Implement deterministic renderer**

Create `src/mir/text.rs`:

```rust
use std::fmt::Write;

use super::{MirModule, OperationKind, RegionKind};

pub fn render_module(module: &MirModule) -> String {
    let mut out = String::new();
    out.push_str("wmir v1\n");
    for (index, region) in module.regions().iter().enumerate() {
        let kind = match region.kind() {
            RegionKind::Lambda { symbol } => format!("lambda {symbol}"),
            RegionKind::Omega { symbol } => format!("omega {symbol}"),
            RegionKind::Gamma { label } => format!("gamma {label}"),
            RegionKind::Theta(kind) => format!("theta {}", theta_kind_text(kind)),
            RegionKind::Delta { label } => format!("delta {label}"),
        };
        let _ = writeln!(out, "region r{index} {kind}");
        for block in region.blocks() {
            let block_data = module.block(*block);
            let _ = writeln!(out, "  block b{} {}", block.raw(), block_data.name());
            for op in block_data.operations() {
                let op_data = module.operation(*op);
                let operands = op_data
                    .operands()
                    .iter()
                    .map(|value| format!("v{}", value.raw()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let results = op_data
                    .results()
                    .iter()
                    .map(|value| format!("v{}", value.raw()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let payload = operation_payload(op_data.kind());
                if results.is_empty() {
                    let _ = writeln!(out, "    op o{} {payload} {operands}", op.raw());
                } else {
                    let _ = writeln!(out, "    op o{} {payload} {operands} -> {results}", op.raw());
                }
            }
        }
    }
    out
}

fn operation_payload(kind: &OperationKind) -> String {
    match kind {
        OperationKind::Literal(text) => format!("literal {text}"),
        OperationKind::ReadValue(name) => format!("read_value {name}"),
        OperationKind::Binary(op) => format!("binary {op}"),
        OperationKind::Let(name) => format!("let {name}"),
        OperationKind::Return => "return".to_string(),
    }
}

fn theta_kind_text(kind: &super::ThetaKind) -> &'static str {
    match kind {
        super::ThetaKind::Repeat => "repeat",
        super::ThetaKind::For => "for",
        super::ThetaKind::Drain => "drain",
        super::ThetaKind::Reduce => "reduce",
        super::ThetaKind::Scan => "scan",
        super::ThetaKind::Loop => "loop",
    }
}
```

- [ ] **Step 4: Implement round-trip parser**

The MIR 01 text grammar is intentionally small and stable:

```text
module       := "wmir v1" newline region*
region       := "region r" number ("lambda" | "omega") symbol newline block*
block        := two-space "block b" number name newline op*
op           := four-space "op o" number payload operands (" -> " results)? newline
operands     := ("v" number)*
results      := ("v" number)*
```

Renderer and parser must preserve result lists exactly. Values that appear only after `->` are results, not operands.

Create `src/mir/parse_text.rs`:

```rust
use super::{
    BlockData, EffectSet, MirModule, MirType, OperationData, OperationKind, RegionData,
    ScalarType, ValueData, ValueId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextParseError {
    message: String,
}

impl TextParseError {
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn parse_module(text: &str) -> Result<MirModule, TextParseError> {
    let mut lines = text.lines();
    match lines.next() {
        Some("wmir v1") => {}
        _ => return Err(TextParseError::new("missing wmir v1 header")),
    }

    let mut module = MirModule::new();
    let mut current_block = None;
    let mut max_value_id = None::<u32>;

    for line in lines {
        if let Some(rest) = line.strip_prefix("region ") {
            let symbol = rest
                .split_once(" lambda ")
                .map(|(_, symbol)| symbol.to_string())
                .or_else(|| rest.split_once(" omega ").map(|(_, symbol)| symbol.to_string()))
                .ok_or_else(|| TextParseError::new(format!("unsupported region line: {line}")))?;
            if rest.contains(" omega ") {
                module.push_region(RegionData::omega(symbol));
            } else {
                module.push_region(RegionData::lambda(symbol));
            }
            current_block = None;
        } else if let Some(rest) = line.strip_prefix("  block ") {
            let region = super::RegionId::new(module.regions().len() as u32 - 1);
            let name = rest
                .split_once(' ')
                .map(|(_, name)| name.to_string())
                .ok_or_else(|| TextParseError::new(format!("bad block line: {line}")))?;
            current_block = Some(module.push_block(region, BlockData::new(name)));
        } else if let Some(rest) = line.strip_prefix("    op ") {
            let block = current_block.ok_or_else(|| TextParseError::new("op before block"))?;
            let kind = parse_operation_kind(rest)?;
            let (operands_text, results_text) = split_operands_and_results(rest);
            let operands = parse_values(operands_text)?;
            let results = parse_values(results_text)?;
            for value in operands.iter().chain(results.iter()) {
                max_value_id = Some(max_value_id.map_or(value.raw(), |max| max.max(value.raw())));
            }
            module.push_operation(
                block,
                OperationData::new(kind, operands, results, EffectSet::empty()),
            );
        } else if !line.trim().is_empty() {
            return Err(TextParseError::new(format!("unexpected line: {line}")));
        }
    }

    for _ in 0..=max_value_id.unwrap_or(0) {
        module.push_value(ValueData::new(MirType::Unknown));
    }
    Ok(module)
}

fn parse_operation_kind(rest: &str) -> Result<OperationKind, TextParseError> {
    if rest.contains(" return") {
        Ok(OperationKind::Return)
    } else if let Some((_, tail)) = rest.split_once(" literal ") {
        Ok(OperationKind::Literal(first_word(tail).to_string()))
    } else if let Some((_, tail)) = rest.split_once(" read_value ") {
        Ok(OperationKind::ReadValue(first_word(tail).to_string()))
    } else if let Some((_, tail)) = rest.split_once(" binary ") {
        Ok(OperationKind::Binary(first_word(tail).to_string()))
    } else if let Some((_, tail)) = rest.split_once(" let ") {
        Ok(OperationKind::Let(first_word(tail).to_string()))
    } else {
        Err(TextParseError::new(format!("unsupported op line: {rest}")))
    }
}

fn split_operands_and_results(rest: &str) -> (&str, &str) {
    rest.split_once(" -> ").unwrap_or((rest, ""))
}

fn parse_values(text: &str) -> Result<Vec<ValueId>, TextParseError> {
    text.split_whitespace()
        .filter_map(|word| word.strip_prefix('v'))
        .map(|raw| {
            raw.parse::<u32>()
                .map(ValueId::new)
                .map_err(|_| TextParseError::new(format!("bad value id v{raw}")))
        })
        .collect()
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}
```

This parser is intentionally scoped to MIR 01 text emitted by `render_module`. It rejects unsupported text with `TextParseError`.

- [ ] **Step 5: Export text modules**

Modify `src/mir/mod.rs`:

```rust
pub mod parse_text;
pub mod text;

pub use parse_text::{TextParseError, parse_module};
```

- [ ] **Step 6: Add integration round-trip test**

Append to `tests/mir.rs`:

```rust
#[test]
fn mir_text_round_trips_for_fixture() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());
    let parsed = wrela::mir::parse_text::parse_module(&text).unwrap();

    assert_eq!(wrela::mir::text::render_module(&parsed), text);
}
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test mir::text::tests::text_dump_is_stable_for_minimal_module
cargo test mir::parse_text::tests::parser_round_trips_rendered_module
cargo test --test mir mir_text_round_trips_for_fixture
cargo test --test mir mir_build_creates_lambda_and_omega_regions
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/mir/mod.rs src/mir/text.rs src/mir/parse_text.rs tests/mir.rs
git commit -m "feat: add MIR text round trip -Codex Automated"
```

**Acceptance criteria:**
- `render_module` is deterministic for the same `MirModule`.
- `parse_text::parse_module(render_module(m))` renders back to the same bytes.
- Malformed text returns `TextParseError` instead of panicking.

---

### Task 7: MIR Telemetry And `wrela dump mir`

**Files:**
- Modify: `src/mir/report.rs`
- Modify: `src/mir/build.rs`
- Modify: `src/command.rs`
- Test: `tests/mir.rs`
- Test: `src/command.rs` unit tests

**Description:** Add W-MIR counters to build results and expose `wrela dump mir <root.wrela>` through the existing CLI.

- [ ] **Step 1: Add failing telemetry and CLI tests**

Append to `tests/mir.rs`:

```rust
#[test]
fn mir_report_counts_fixture_shape() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    let report = mir.report();

    assert!(report.regions() >= 1);
    assert!(report.blocks() >= 1);
    assert!(report.operations() >= 4);
    assert!(report.values() >= 3);
}

#[test]
fn dump_mir_command_prints_deterministic_text() {
    let root = fixture("data_flow.wrela");
    let args = vec![
        "wrela".to_string(),
        "dump".to_string(),
        "mir".to_string(),
        root.to_string_lossy().to_string(),
    ];
    let mut out1 = Vec::new();
    let mut err1 = Vec::new();
    let mut out2 = Vec::new();
    let mut err2 = Vec::new();

    let code1 = wrela::command::run_with_io(args.clone(), &mut out1, &mut err1);
    let code2 = wrela::command::run_with_io(args, &mut out2, &mut err2);

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert!(err1.is_empty());
    assert!(err2.is_empty());
    assert_eq!(out1, out2);
    assert!(String::from_utf8(out1).unwrap().contains("wmir v1"));
}
```

Add to `src/command.rs` tests:

```rust
#[test]
fn help_lists_mir_dump_command() {
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = run_with_io(vec!["wrela".to_string(), "help".to_string()], &mut out, &mut err);

    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("wrela dump mir <root.wrela>")
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test mir mir_report_counts_fixture_shape
cargo test --test mir dump_mir_command_prints_deterministic_text
cargo test command::tests::help_lists_mir_dump_command
```

Expected: fail because CLI and counters are not wired.

- [ ] **Step 3: Compute report from module**

In `src/mir/report.rs`, add:

```rust
use super::{MirModule, MirType};

pub fn report_for_module(module: &MirModule) -> MirReport {
    let state_tokens = module
        .values()
        .iter()
        .filter(|value| matches!(value.ty(), MirType::StateToken(_)))
        .count();
    let capacity_tokens = module
        .values()
        .iter()
        .filter(|value| matches!(value.ty(), MirType::CapacityToken { .. }))
        .count();

    MirReport::new(
        module.regions().len(),
        module.blocks().len(),
        module.operations().len(),
        module.values().len(),
        module.places().len(),
        state_tokens,
        capacity_tokens,
    )
}
```

In `src/mir/build.rs`, replace manual `MirReport::new(...)` calls for successful modules with `report_for_module(&module)`.

- [ ] **Step 4: Wire CLI help**

Modify both help paths in `src/command.rs` to include:

```rust
let _ = writeln!(out, "wrela dump mir <root.wrela>");
```

- [ ] **Step 5: Wire `dump mir` command**

Modify the `Some("dump")` branch in `src/command.rs`:

```rust
Some("dump") => match collected.get(2).map(String::as_str) {
    Some("tokens") => { /* existing branch unchanged */ }
    Some("mir") => match collected.get(3) {
        Some(path) => {
            if collected.len() > 4 {
                let _ = writeln!(err, "malformed command");
                2
            } else {
                dump_mir(path, out, err)
            }
        }
        None => {
            let _ = writeln!(err, "missing root file path");
            2
        }
    },
    _ => {
        let _ = writeln!(err, "malformed command");
        2
    }
},
```

Add `dump_mir`:

```rust
fn dump_mir<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let check = crate::check::check_root(path);
    if !check.ok() {
        print_diagnostics(out, check.diagnostics());
        return 1;
    }

    let mir = crate::mir::build_mir(&check);
    if !mir.ok() {
        print_diagnostics(out, mir.diagnostics());
        return 1;
    }

    let module = mir.module().expect("ok MIR result has module");
    let _ = write!(out, "{}", crate::mir::text::render_module(module));
    0
}
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test --test mir mir_report_counts_fixture_shape
cargo test --test mir dump_mir_command_prints_deterministic_text
cargo test command::tests::help_lists_mir_dump_command
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/report.rs src/mir/build.rs src/command.rs tests/mir.rs
git commit -m "feat: expose MIR dump command -Codex Automated"
```

**Acceptance criteria:**
- `MirReport` counters reflect module contents.
- `wrela dump mir <root.wrela>` exits `0` and prints deterministic W-MIR text for valid source.
- `wrela dump mir <root.wrela>` exits `1` and prints diagnostics for invalid source.
- Help output lists `wrela dump mir <root.wrela>`.

---

### Task 8: MIR 01 Integration Fixtures And Documentation

**Files:**
- Create: `fixtures/mir/imports/root.wrela`
- Create: `fixtures/mir/imports/lib.wrela`
- Create: `docs/design/compiler-pipeline.md`
- Create: `docs/design/supported-wrela-subset.md`
- Test: `tests/mir.rs`

**Description:** Add multi-file MIR coverage and update current-state docs so they mention W-MIR dump as implemented after this plan.

- [ ] **Step 1: Add import fixture**

Create `fixtures/mir/imports/lib.wrela`:

```wrela
module lib

pub data Bytes {
    value: U32
}

pub class Helper {
    fn id(read self, read value: U32) -> U32 {
        return value
    }
}
```

Create `fixtures/mir/imports/root.wrela`:

```wrela
module app.root

use { Bytes, Helper } from lib

pub class App {
    fn run(read self, read value: U32) -> U32 {
        let copy: U32 = value
        return copy
    }
}
```

- [ ] **Step 2: Add failing multi-file test**

Append to `tests/mir.rs`:

```rust
#[test]
fn mir_builds_reachable_import_graph_in_stable_order() {
    let check = wrela::check::check_root(fixture("imports/root.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());

    let app_index = text.find("lambda app.root.App.run").unwrap();
    let helper_index = text.find("lambda lib.Helper.id").unwrap();
    assert!(app_index < helper_index);
}
```

- [ ] **Step 3: Run deterministic ordering test**

Run:

```bash
cargo test --test mir mir_builds_reachable_import_graph_in_stable_order
```

Expected: pass. `build_mir` must iterate summaries in deterministic order sorted by `FileId::raw()` and then module path before emitting regions.

- [ ] **Step 4: Create compiler pipeline docs**

Create `docs/design/compiler-pipeline.md`:

```markdown
# Compiler Pipeline

| Command | Pipeline |
|---|---|
| `wrela dump mir <root.wrela>` | discover -> lex -> parse -> check -> W-MIR build -> verify -> text dump |

## Phase Responsibilities

| Phase | Input | Output | Responsibility |
|---|---|---|---|
| `mir` | `CheckResult` | `MirBuildResult` | Regioned Effect SSA build, verify, deterministic text |
```

- [ ] **Step 5: Create supported subset docs**

Create `docs/design/supported-wrela-subset.md`:

```markdown
# Supported Wrela Subset

## Commands

- `wrela dump tokens <root.wrela>`
- `wrela lex <root.wrela>`
- `wrela parse <root.wrela>`
- `wrela check <root.wrela>`
- `wrela dump mir <root.wrela>`

## MIR Body Forms

`wrela dump mir <root.wrela>` builds W-MIR only after `check` succeeds. The first MIR builder covers the checker-supported body subset: `let`, `return`, literals, names, parenthesized expressions, and binary expressions.

Checked expressions outside this MIR 01 subset must produce a MIR diagnostic and no MIR module. They must not be silently lowered to `None`.
```

- [ ] **Step 6: Run focused checks**

Run:

```bash
cargo test --test mir
cargo run -- dump mir fixtures/mir/data_flow.wrela
```

Expected:
- `cargo test --test mir` passes.
- `cargo run -- dump mir fixtures/mir/data_flow.wrela` exits `0` and prints text starting with `wmir v1`.

- [ ] **Step 7: Commit**

```bash
git add fixtures/mir/imports/root.wrela fixtures/mir/imports/lib.wrela tests/mir.rs docs/design/compiler-pipeline.md docs/design/supported-wrela-subset.md
git commit -m "docs: document MIR 01 surface -Codex Automated"
```

**Acceptance criteria:**
- Multi-file reachable imports produce deterministic W-MIR region order.
- Current-state docs mention `wrela dump mir`.
- No parser/check behavior changes are introduced.

---

### Task 9: MIR 01 Final Quality Gate And Phase A Handoff

**Files:**
- No production file edits unless verification reveals a bug.

**Description:** Prove the MIR 01 product boundary before handoff.

- [ ] **Step 1: Run full local quality gate**

Run:

```bash
./scripts/quality-gate.sh
```

Expected: pass.

- [ ] **Step 2: Run MIR smoke commands**

Run:

```bash
cargo run -- dump mir fixtures/mir/basic.wrela
cargo run -- dump mir fixtures/mir/data_flow.wrela
cargo run -- dump mir fixtures/mir/imports/root.wrela
```

Expected: all exit `0`, each output starts with `wmir v1`, and each output is deterministic if run twice.

- [ ] **Step 3: Run strict gate when ready for final merge**

Run when intended implementation changes are committed:

```bash
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

Expected: pass only when the working tree is clean.

- [ ] **Step 4: Complete Phase A self review**

Use the repository review workflow in `docs/implementation/reviews/README.md`. Fix every finding at every priority unless the verdict documents an explicit disagreement.

**Acceptance criteria:**
- `./scripts/quality-gate.sh` passes.
- MIR smoke commands pass.
- Phase A verdict is APPROVED in the handoff message before user handoff.

## Self-Review Checklist

- [ ] MIR build refuses failed check results.
- [ ] MIR build succeeds for all fixtures under `fixtures/mir/`.
- [ ] Verifier rejects malformed modules in unit tests.
- [ ] W-MIR text dump round-trips.
- [ ] CLI command is `wrela dump mir <root.wrela>`.
- [ ] MIR counters are deterministic.
- [ ] No external crates were added.
