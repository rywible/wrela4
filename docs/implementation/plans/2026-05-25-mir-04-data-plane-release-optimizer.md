# MIR 04 Certified Data-Plane Mask Optimizer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Wrela's first certified data-plane release optimizer slice at the IR level: checked table/mask types, MIR provenance facts, authority query V0, certified mask boolean identity rewrites, branch/select cost seeds, rewrite reports, and an explicit generated-code unsupported boundary.

**Architecture:** MIR 04 extends the MIR 01-03 foundation. The checked language gets a narrow `Table[T,N]` / `Mask[table,N]` subset, MIR carries table identity and row-token facts through verification and text round-trip, data-plane legality facts come from an authority query module, and release passes reuse MIR 03 certificates and bounded rewrite events. Data-plane generated-code execution is not claimed in this plan: `wrela perf code` returns a clear unsupported result for data-plane fixtures until MIR 05 adds LIR load/branch/mask-loop codegen and table-loop fusion.

**Tech Stack:** Rust 2024, standard library only, existing parser/check/MIR/release/perf modules, handwritten authority queries, handwritten AArch64 cost seeds, handwritten certified data-plane rewrites.

---

## Locked Decisions

- MIR 04 implements a real data-plane subset with checked fixtures; it is not a design-only spike.
- `Table[T,N]` is a checked type whose second generic argument is a positive integer row count.
- `Mask[packets,N]` names a previously declared parameter in the same signature. In MIR 04, `Mask[Packet,N]` is rejected because it lacks table-parameter provenance.
- Signature checking resolves parameters left-to-right so a mask can refer to an earlier table parameter and cannot refer to a later one.
- `Table` and `Mask` row counts must match at check time and again in the MIR verifier.
- Mask rewrites fire only when `authority::same_mask_domain(left, right)` returns a positive fact.
- MIR 04 implements the certified mask identity rules `m & true -> m`, `m | false -> m`, `m & m -> m`, `m | m -> m`, and `!!m -> m`.
- Table loop fusion, data-plane AArch64 loop codegen, parameterized runtime A/B evidence, persistent ledger storage, and cost feedback are MIR 05 work.
- Parameterized data-plane generated-code benchmarks in MIR 04 must exit `2` with a deterministic unsupported message.
- MIR 04 performance evidence is IR-level: certificates, rewrite counts, verifier evidence, authority facts, and text-dump before/after reports. Runtime claims are not made for data-plane passes in MIR 04.
- No external crate dependencies are introduced.

## Planned File Structure

```text
src/check/summary.rs       # generic arg summaries for type, integer, and parameter-name args
src/check/types.rs         # checked Table/Mask type forms and parameter-scoped resolution
src/check/body.rs          # reduce/rows/field/index subset checking
src/check/layout.rs        # exhaustive TypeKind handling for Table/Mask
src/check/json.rs          # exhaustive TypeKind rendering if report output touches types
src/mir/dataplane.rs       # table/mask facts, row tokens, test module builders
src/mir/authority.rs       # authority query V0 for table/mask/effect facts
src/mir/ir.rs              # data-plane OperationKind variants
src/mir/build.rs           # lower checked data-plane subset into MIR facts
src/mir/verify.rs          # provenance and row-token verifier rules
src/mir/text.rs            # render data-plane operations
src/mir/parse_text.rs      # parse data-plane operations
src/mir/rewrite.rs         # certified mask algebra and branch/select passes
src/mir/pass_control.rs    # MIR 04 pass names
src/mir/cost.rs            # target cost profiles
src/mir/perf.rs            # data-plane perf unsupported result
tests/check.rs
tests/mir_dataplane.rs
tests/mir_perf.rs
fixtures/mir/dataplane/filter_sum.wrela
fixtures/perf/filter_sum.wrela
```

## Parallel Work Map

- Task 1 must run first because it defines checked type and body semantics.
- Task 2 depends on Task 1 and owns MIR operation/fact surfaces.
- Task 3 depends on Task 2 and owns authority query V0.
- Task 4 depends on Task 2 and creates reusable data-plane test builders.
- Task 5 depends on Tasks 3-4 and owns certified mask identity rewrites.
- Task 6 depends on Tasks 3-5 and owns target cost profiles plus branch/select decisions.
- Task 7 depends on Tasks 1-6 and owns the generated-code unsupported boundary.
- Task 8 runs last and owns quality gate plus Phase A handoff.
- Integration-owned shared files: `src/check/types.rs`, `src/check/body.rs`, `src/mir/ir.rs`, `src/mir/rewrite.rs`, `src/mir/perf.rs`, `tests/mir_dataplane.rs`, and `tests/mir_perf.rs`. Only one subagent edits each at a time.

---

### Task 1: Checked Data-Plane Subset

**Files:**
- Modify: `src/check/summary.rs`
- Modify: `src/check/types.rs`
- Modify: `src/check/body.rs`
- Modify: `src/check/layout.rs`
- Modify: `src/check/json.rs`
- Test: `tests/check.rs`
- Fixture: `fixtures/mir/dataplane/filter_sum.wrela`

**Description:** Teach `check` enough table/mask/reduce/index semantics to accept the first real data-plane benchmark. This task does not optimize.

- [ ] **Step 1: Add failing checked fixture test**

Append to `tests/check.rs`:

```rust
#[test]
fn check_accepts_filter_sum_dataplane_subset() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join("dataplane")
        .join("filter_sum.wrela");

    let result = wrela::check::check_root(root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}
```

Create `fixtures/mir/dataplane/filter_sum.wrela`:

```wrela
module perf.filter_sum

pub data Packet {
    len: U32
    flags: U32
}

pub class FilterSumBench {
    fn run(
        read self,
        read packets: Table[Packet, 256],
        read valid: Mask[packets, 256],
        read small: Table[Packet, 128],
        read small_valid: Mask[small, 128],
    ) -> U64 {
        let total = reduce packets.rows(valid) as row, acc: U64 = 0 {
            return acc + packets.len[row]
        }
        let flagged = reduce packets.rows(valid) as row, acc: U64 = 0 {
            return acc + packets.flags[row]
        }
        let small_total = reduce small.rows(small_valid) as row, acc: U64 = 0 {
            return acc + small.len[row]
        }
        return total + flagged + small_total
    }
}
```

- [ ] **Step 2: Preserve generic integer and parameter-name arguments**

Replace `TypeRefSummary`'s recursive `args: Vec<TypeRefSummary>` field in `src/check/summary.rs` with:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeArgSummary {
    Type(TypeRefSummary),
    Int { value: u64, span: Span },
    ValueName(Name),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSummary {
    node: SyntaxNodeId,
    access: Option<AccessMode>,
    unique: bool,
    path: Vec<Name>,
    args: Vec<TypeArgSummary>,
    span: Span,
}
```

Add direct-path extraction before generic-argument extraction. This fixes the current descendant-identifier behavior where `Table[Packet, 256]` can accidentally summarize as path `Table.Packet`.

```rust
fn summarize_type_ref(view: &CstView<'_>, node: SyntaxNodeId) -> Option<TypeRefSummary> {
    let generic_args = view
        .child_nodes(node)
        .find(|child| view.node_kind(*child) == SyntaxKind::GenericArgList)
        .map(|arg_list| summarize_generic_args(view, arg_list))
        .unwrap_or_default();

    let path = summarize_direct_type_path(view, node);
    if path.is_empty() {
        return None;
    }

    Some(TypeRefSummary {
        node,
        access: summarize_access_mode(view, node),
        unique: summarize_unique_marker(view, node),
        path,
        args: generic_args,
        span: view.node_span(node),
    })
}

fn summarize_direct_type_path(view: &CstView<'_>, type_ref: SyntaxNodeId) -> Vec<Name> {
    let mut names = Vec::new();
    for child in view.child_nodes(type_ref) {
        if view.node_kind(child) == SyntaxKind::GenericArgList {
            break;
        }
        if view.node_kind(child) == SyntaxKind::Name {
            if let Some(name) = summarize_name(view, child) {
                names.push(name);
            }
        }
    }
    names
}
```

Implement generic-argument collection. The parser represents a bare identifier generic argument such as `packets` in `Mask[packets, 256]` as a single-segment `TypeRef`, not as a raw token. Preserve that shape as `TypeArgSummary::Type`; `Mask` resolution in Step 3 explicitly accepts a single-segment type arg as a parameter-name reference. Do not normalize all single-segment type args to `ValueName`, because `Table[Packet, 256]` must still see `Packet` as a type argument.

```rust
fn summarize_generic_args(view: &CstView<'_>, arg_list: SyntaxNodeId) -> Vec<TypeArgSummary> {
    let mut args = Vec::new();
    for child in view.child_nodes(arg_list) {
        if view.node_kind(child) == SyntaxKind::TypeRef {
            if let Some(ty) = summarize_type_ref(view, child) {
                args.push(TypeArgSummary::Type(ty));
            }
        }
    }
    for token in view.child_tokens(arg_list) {
        let text = view.token_text(token);
        if let Ok(value) = text.text().parse::<u64>() {
            args.push(TypeArgSummary::Int { value, span: text.span() });
        } else if is_identifier_text(text.text()) {
            args.push(TypeArgSummary::ValueName(name_from_token(text)));
        }
    }
    args
}
```

Add accessors used by type resolution:

```rust
impl TypeRefSummary {
    pub fn args(&self) -> &[TypeArgSummary] { &self.args }
    pub fn path(&self) -> &[Name] { &self.path }

    pub fn path_text(&self) -> String {
        self.path
            .iter()
            .map(Name::text)
            .collect::<Vec<_>>()
            .join(".")
    }
}
```

- [ ] **Step 3: Implement checked `Table` and `Mask` type forms**

In `src/check/types.rs`, extend `TypeKind`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Builtin(BuiltinType),
    Item(ItemId, SymbolKind),
    Table { item: ItemId, rows: u64 },
    Mask { table_param: u32, rows: u64 },
    Unknown(UnknownReason),
}
```

Add constructors:

```rust
impl TypeTable {
    pub fn table(&mut self, item: ItemId, rows: u64) -> TypeId {
        self.push(TypeKind::Table { item, rows })
    }

    pub fn mask(&mut self, table_param: u32, rows: u64) -> TypeId {
        self.push(TypeKind::Mask { table_param, rows })
    }
}
```

Resolution rules:

```text
Table requires exactly [Type, Int] where Int > 0.
Mask accepts exactly [ValueName, Int] or [Type(single-segment, no args), Int].
For the [Type(single-segment), Int] form, resolve the single segment as a value-parameter name rather than a type name.
The value parameter must name an earlier parameter.
Mask row count must equal the named table parameter row count.
Mask cannot refer to a later parameter.
Mask cannot refer to a data type name.
```

- [ ] **Step 4: Check reduce/rows/index subset**

In `src/check/body.rs`, accept:

```text
reduce packets.rows(valid) as row, acc: U64 = 0 { return acc + packets.len[row] }
```

Rules:

```text
packets must have TypeKind::Table.
valid must have TypeKind::Mask for packets and the same rows.
row is a row token scoped only to the reduce body.
packets.len[row] resolves to the field type of Packet.len.
Accumulator init and return expression must have the declared accumulator type.
```

Add a helper for unsupported data-plane expressions so this task does not rely on an implicit checker API:

```rust
impl<'a> BodyChecker<'a> {
    fn record_unsupported(&mut self, span: Span, message: impl Into<String>) -> TypeId {
        self.diagnostics.push(Diagnostic::error(span, message.into())
            .with_code(DiagnosticCode::UnsupportedExpression));
        self.type_table.push_unknown(UnknownReason::UnsupportedExpression)
    }
}
```

Use this helper for data-plane syntax outside the MIR 04 subset, including nested `reduce`, writes through `packets.field[row]`, non-identifier row variables, and non-integer accumulator annotations.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test --test check check_accepts_filter_sum_dataplane_subset
```

Expected: pass.

**Acceptance criteria:**
- `filter_sum.wrela` checks successfully.
- Invalid `Mask[Packet, 256]` is rejected because it lacks parameter provenance.
- Invalid row-count mismatch is rejected.
- Later-parameter mask references are rejected.

---

### Task 2: Data-Plane MIR Facts And Round Trip

**Files:**
- Create: `src/mir/dataplane.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/mir/ir.rs`
- Modify: `src/mir/build.rs`
- Modify: `src/mir/verify.rs`
- Modify: `src/mir/text.rs`
- Modify: `src/mir/parse_text.rs`
- Test: `tests/mir_dataplane.rs`

**Description:** Lower checked table/mask/reduce facts into W-MIR operation kinds and verify they round-trip through W-MIR text.

- [ ] **Step 1: Add failing round-trip test**

Create `tests/mir_dataplane.rs`:

```rust
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join("dataplane")
        .join("filter_sum.wrela")
}

#[test]
fn dataplane_mir_round_trips_table_mask_facts() {
    let check = wrela::check::check_root(fixture());
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok(), "{:?}", mir.diagnostics());
    let module = mir.module().unwrap();

    let text = wrela::mir::text::render_module(module);
    assert!(text.contains("op table_rows packets rows=256"));
    assert!(text.contains("op mask_read valid table=packets rows=256"));
    assert!(text.contains("op row_token table=packets rows=256"));

    let parsed = wrela::mir::parse_text::parse_module(&text).unwrap();
    assert_eq!(wrela::mir::text::render_module(&parsed), text);
    assert!(wrela::mir::verify_module(&parsed).ok());
}
```

- [ ] **Step 2: Add data-plane fact types**

Create `src/mir/dataplane.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MaskValueKind {
    Input,
    AllTrue,
    AllFalse,
    Derived,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MaskProvenance {
    name: String,
    table: String,
    rows: u64,
    kind: MaskValueKind,
}

impl MaskProvenance {
    pub fn new(name: String, table: String, rows: u64) -> Self {
        Self { name, table, rows, kind: MaskValueKind::Input }
    }

    pub fn constant(table: String, rows: u64, kind: MaskValueKind) -> Self {
        Self { name: "<constant>".to_string(), table, rows, kind }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn table(&self) -> &str { &self.table }
    pub fn rows(&self) -> u64 { self.rows }
    pub fn kind(&self) -> MaskValueKind { self.kind }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowTokenFact {
    table: String,
    rows: u64,
    escapes: bool,
}
```

- [ ] **Step 3: Add operation kinds**

In `src/mir/ir.rs`, extend `OperationKind`:

```rust
TableRows { table: String, rows: u64 },
MaskRead { name: String, table: String, rows: u64 },
MaskAllTrue { table: String, rows: u64 },
MaskAllFalse { table: String, rows: u64 },
MaskNot,
MaskAnd,
MaskOr,
RowToken { table: String, rows: u64 },
ReduceRows { table: String, mask: String, field: String, rows: u64 },
```

- [ ] **Step 4: Lower checked data-plane bodies into MIR**

In `src/mir/build.rs`, lower the three checked reduces in `fixtures/mir/dataplane/filter_sum.wrela` into explicit data-plane operations. The builder emits operations in source order and uses the reduce result values in the final scalar return expression:

```rust
fn lower_reduce_rows(
    module: &mut MirModule,
    block: BlockId,
    table: &str,
    mask: &str,
    field: &str,
    rows: u64,
    result_ty: MirType,
) -> ValueId {
    let rows_value = module.push_value(ValueData::new(MirType::Table {
        item: table.to_string(),
        rows,
    }));
    module.push_operation(block, OperationData::new(
        OperationKind::TableRows { table: table.to_string(), rows },
        Vec::new(),
        vec![rows_value],
        EffectSet::empty(),
    ));

    let mask_value = module.push_value(ValueData::new(MirType::Mask {
        table: table.to_string(),
        rows,
    }));
    module.push_operation(block, OperationData::new(
        OperationKind::MaskRead {
            name: mask.to_string(),
            table: table.to_string(),
            rows,
        },
        Vec::new(),
        vec![mask_value],
        EffectSet::empty(),
    ));

    let row = module.push_value(ValueData::new(MirType::RowToken {
        table: table.to_string(),
    }));
    module.push_operation(block, OperationData::new(
        OperationKind::RowToken { table: table.to_string(), rows },
        vec![rows_value],
        vec![row],
        EffectSet::empty(),
    ));

    let acc = module.push_value(ValueData::new(result_ty));
    module.push_operation(block, OperationData::new(
        OperationKind::ReduceRows {
            table: table.to_string(),
            mask: mask.to_string(),
            field: field.to_string(),
            rows,
        },
        vec![rows_value, mask_value, row],
        vec![acc],
        EffectSet::empty(),
    ));
    acc
}
```

For the fixture, call it as:

```rust
let total = lower_reduce_rows(module, block, "packets", "valid", "len", 256, MirType::Scalar(ScalarType::U64));
let flagged = lower_reduce_rows(module, block, "packets", "valid", "flags", 256, MirType::Scalar(ScalarType::U64));
let small_total = lower_reduce_rows(module, block, "small", "small_valid", "len", 128, MirType::Scalar(ScalarType::U64));
let sum_1 = lower_binary_add(module, block, total, flagged, MirType::Scalar(ScalarType::U64));
let sum_2 = lower_binary_add(module, block, sum_1, small_total, MirType::Scalar(ScalarType::U64));
lower_return(module, block, sum_2);
```

`lower_binary_add` and `lower_return` use the existing scalar MIR operation forms from MIR 01. This task does not invent a data-plane-specific return operation.

- [ ] **Step 5: Verify provenance rules**

In `src/mir/verify.rs`, reject:

```text
MaskAnd or MaskOr where input mask domains differ.
ReduceRows whose table/mask rows differ.
RowToken values used outside their owning reduce/row loop.
RowToken values stored in Place or returned from the region.
```

- [ ] **Step 6: Render and parse operations**

Add stable text forms:

```text
op table_rows packets rows=256
op mask_read valid table=packets rows=256
op mask_all_true table=packets rows=256
op mask_all_false table=packets rows=256
op mask_not
op mask_and
op mask_or
op row_token table=packets rows=256
op reduce_rows table=packets mask=valid field=len rows=256
```

- [ ] **Step 7: Run focused test**

Run:

```bash
cargo test --test mir_dataplane dataplane_mir_round_trips_table_mask_facts
```

Expected: pass.

**Acceptance criteria:**
- Data-plane operation kinds exist in W-MIR.
- Text dump is deterministic and round-trip parseable.
- Verifier rejects mismatched mask/table provenance and escaping row tokens.

---

### Task 3: Authority Query V0

**Files:**
- Modify: `src/mir/cert.rs`
- Modify: `src/mir/effect.rs`
- Create: `src/mir/authority.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/authority.rs`

**Description:** Introduce a real legality-query layer for data-plane facts. MIR 04 starts with table/mask/effect queries; MIR 05 extends this module to capability paths, ordering domains, state edges, and trap order.

- [ ] **Step 1: Write failing authority tests**

Create `src/mir/authority.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::{AuthorityFact, AuthorityQuery};
    use crate::mir::dataplane::MaskProvenance;

    #[test]
    fn authority_query_proves_same_mask_domain() {
        let query = AuthorityQuery::new();
        let left = MaskProvenance::new("valid".to_string(), "packets".to_string(), 256);
        let right = MaskProvenance::new("large".to_string(), "packets".to_string(), 256);

        assert_eq!(
            query.same_mask_domain(&left, &right),
            Some(AuthorityFact::SameMaskDomain {
                table: "packets".to_string(),
                rows: 256,
            })
        );
    }

    #[test]
    fn authority_query_refuses_mismatched_mask_domain() {
        let query = AuthorityQuery::new();
        let left = MaskProvenance::new("valid".to_string(), "packets".to_string(), 256);
        let right = MaskProvenance::new("small_valid".to_string(), "small".to_string(), 128);

        assert_eq!(query.same_mask_domain(&left, &right), None);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::authority::tests::
```

Expected: fail because authority query V0 is not implemented.

- [ ] **Step 3: Extend certificate facts for authority evidence**

In `src/mir/cert.rs`, extend `RewriteFact`:

```rust
pub enum RewriteFact {
    PureOperation { op: OperationId },
    LiteralZero { value: String },
    EffectAbsent { effect: &'static str },
    Authority { name: &'static str, detail: String },
    VerifierPassed,
}
```

- [ ] **Step 4: Implement authority facts**

Replace `src/mir/authority.rs` with:

```rust
use super::{dataplane::MaskProvenance, Effect, EffectSet, RewriteFact};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityFact {
    SameTable { table: String },
    SameMaskDomain { table: String, rows: u64 },
    SameRowCount { rows: u64 },
    EffectAbsent { effect: Effect },
}

impl AuthorityFact {
    pub fn as_rewrite_fact(&self) -> RewriteFact {
        match self {
            AuthorityFact::SameTable { table } => RewriteFact::Authority {
                name: "same-table",
                detail: table.clone(),
            },
            AuthorityFact::SameMaskDomain { table, rows } => RewriteFact::Authority {
                name: "same-mask-domain",
                detail: format!("{table}:{rows}"),
            },
            AuthorityFact::SameRowCount { rows } => RewriteFact::Authority {
                name: "same-row-count",
                detail: rows.to_string(),
            },
            AuthorityFact::EffectAbsent { effect } => RewriteFact::EffectAbsent {
                effect: effect.name(),
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthorityQuery;

impl AuthorityQuery {
    pub fn new() -> Self {
        Self
    }

    pub fn same_mask_domain(
        &self,
        left: &MaskProvenance,
        right: &MaskProvenance,
    ) -> Option<AuthorityFact> {
        if left.table() == right.table() && left.rows() == right.rows() {
            Some(AuthorityFact::SameMaskDomain {
                table: left.table().to_string(),
                rows: left.rows(),
            })
        } else {
            None
        }
    }

    pub fn effect_absent(&self, effects: EffectSet, effect: Effect) -> Option<AuthorityFact> {
        if effects.contains(effect) {
            None
        } else {
            Some(AuthorityFact::EffectAbsent { effect })
        }
    }
}
```

Add `Effect::name()` in `src/mir/effect.rs`:

```rust
impl Effect {
    pub const fn name(self) -> &'static str {
        match self {
            Effect::Trap => "Trap",
            Effect::Io => "Io",
            Effect::Volatile => "Volatile",
            Effect::Time => "Time",
            Effect::Entropy => "Entropy",
            Effect::Arena => "Arena",
            Effect::Mutate => "Mutate",
            Effect::Dma => "Dma",
            Effect::Block => "Block",
            Effect::AddressArithmetic => "AddressArithmetic",
            Effect::Assembly => "Assembly",
        }
    }
}

impl EffectSet {
    pub const fn is_empty(self) -> bool {
        self.bits() == 0
    }
}
```

- [ ] **Step 5: Export authority API**

Modify `src/mir/mod.rs`:

```rust
pub mod authority;

pub use authority::{AuthorityFact, AuthorityQuery};
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test mir::authority::tests::
```

Expected: pass.

**Acceptance criteria:**
- Mask-domain legality is queried through `AuthorityQuery`.
- Mismatched table or row count returns `None`.
- Query facts can be copied into rewrite certificates.

---

### Task 4: Data-Plane Test Module Builders

**Files:**
- Modify: `src/mir/dataplane.rs`
- Modify: `src/mir/ir.rs`
- Test: unit tests in `src/mir/dataplane.rs`

**Description:** Add small test-only builders for mask identity rewrite tests. Builders must use normal MIR constructors so verifier coverage remains meaningful.

- [ ] **Step 1: Add failing builder test**

Append to `src/mir/dataplane.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn dataplane_testing_builders_create_verifiable_modules() {
        let module = super::testing::mask_and_true_module("packets", 256);
        assert!(crate::mir::verify_module(&module).ok());
    }
}
```

- [ ] **Step 2: Add test-only MIR construction helpers**

Add these helpers under `#[cfg(test)]` in `src/mir/ir.rs`:

```rust
impl MirModule {
    pub fn empty_for_test() -> Self {
        let mut module = Self::new();
        let region = module.push_region(RegionData::lambda("test".to_string()));
        module.push_block(region, BlockData::new("entry".to_string()));
        module
    }

    pub fn push_test_operation(&mut self, data: OperationData) -> OperationId {
        self.push_operation(BlockId::new(0), data)
    }

    pub fn replace_operation_kind_for_test(&mut self, index: usize, kind: OperationKind) {
        self.operations[index].replace_kind(kind);
    }
}

impl OperationData {
    pub fn replace_kind(&mut self, kind: OperationKind) {
        self.kind = kind;
    }
}
```

- [ ] **Step 3: Add test-only builders**

Append to `src/mir/dataplane.rs`:

```rust
#[cfg(test)]
pub mod testing {
    use crate::mir::{EffectSet, MirModule, MirType, OperationData, OperationKind, ValueData};

    pub fn mask_and_true_module(table: &str, rows: u64) -> MirModule {
        let mut module = MirModule::empty_for_test();
        let mask = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let all_true = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let result = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskRead {
                name: "valid".to_string(),
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![mask],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskAllTrue {
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![all_true],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskAnd,
            vec![mask, all_true],
            vec![result],
            EffectSet::empty(),
        ));
        module
    }

    pub fn mask_and_mismatched_true_module(table: &str, left_rows: u64, right_rows: u64) -> MirModule {
        let mut module = mask_and_true_module(table, left_rows);
        module.replace_operation_kind_for_test(1, OperationKind::MaskAllTrue {
            table: table.to_string(),
            rows: right_rows,
        });
        module
    }
}
```

- [ ] **Step 4: Run focused test**

Run:

```bash
cargo test mir::dataplane::tests::dataplane_testing_builders_create_verifiable_modules
```

Expected: pass.

**Acceptance criteria:**
- Helper modules verify.
- Helpers are compiled only for tests.
- Helpers use normal `OperationData` construction.

---

### Task 5: Certified Mask Boolean Identity

**Files:**
- Modify: `src/mir/pass_control.rs`
- Modify: `src/mir/rewrite.rs`
- Modify: `src/mir/dataplane.rs`
- Test: unit tests in `src/mir/rewrite.rs`

**Description:** Add certified mask identity rewrites guarded by authority query facts. This proves the Trident machinery on a tractable data-plane rewrite before MIR 05's table-loop fusion.

- [ ] **Step 1: Add failing rewrite tests**

Add to the `#[cfg(test)]` module in `src/mir/rewrite.rs`:

```rust
#[test]
fn mask_algebra_rewrites_and_true_for_same_table() {
    let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
    let passes = crate::mir::PassSet::only(crate::mir::Pass::MaskAlgebra);
    let optimized = crate::mir::optimize_release(&module, &passes);

    assert_eq!(optimized.report().rewrites_applied(), 1);
    assert_eq!(optimized.report().rewrite_events().len(), 1);
    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "mask-and-true");
    assert!(cert.facts().iter().any(|fact| format!("{fact:?}").contains("same-mask-domain")));
    assert!(!crate::mir::text::render_module(optimized.module()).contains("op mask_and"));
}

#[test]
fn mask_algebra_refuses_different_row_counts() {
    let module = crate::mir::dataplane::testing::mask_and_mismatched_true_module(
        "packets",
        256,
        128,
    );
    let passes = crate::mir::PassSet::only(crate::mir::Pass::MaskAlgebra);
    let optimized = crate::mir::optimize_release(&module, &passes);

    assert_eq!(optimized.report().rewrites_applied(), 0);
    assert_eq!(optimized.report().rewrite_events().len(), 0);
}
```

- [ ] **Step 2: Add MIR 04 pass names and rule names**

Extend `Pass`:

```rust
pub enum Pass {
    ScalarPeephole,
    DeadCode,
    CopyProp,
    MaskAlgebra,
    BranchSelect,
}
```

Pass names:

```rust
Pass::MaskAlgebra => "mask-algebra",
Pass::BranchSelect => "branch-select",
```

Extend `RewriteRule`:

```rust
pub enum RewriteRule {
    ScalarAddZero,
    MaskAndTrue,
    MaskOrFalse,
    MaskAndSelf,
    MaskOrSelf,
    MaskDoubleNot,
    BranchSelect,
}
```

Rule names:

```rust
RewriteRule::MaskAndTrue => "mask-and-true",
RewriteRule::MaskOrFalse => "mask-or-false",
RewriteRule::MaskAndSelf => "mask-and-self",
RewriteRule::MaskOrSelf => "mask-or-self",
RewriteRule::MaskDoubleNot => "mask-double-not",
RewriteRule::BranchSelect => "branch-select",
```

- [ ] **Step 3: Implement provenance extraction**

In `src/mir/rewrite.rs`, add:

```rust
use crate::mir::dataplane::{MaskProvenance, MaskValueKind};

fn mask_provenance_by_value(module: &MirModule) -> BTreeMap<ValueId, MaskProvenance> {
    let mut map = BTreeMap::new();
    for op in module.operations() {
        match op.kind() {
            OperationKind::MaskRead { name, table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(*result, MaskProvenance::new(name.clone(), table.clone(), *rows));
                }
            }
            OperationKind::MaskAllTrue { table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(
                        *result,
                        MaskProvenance::constant(table.clone(), *rows, MaskValueKind::AllTrue),
                    );
                }
            }
            OperationKind::MaskAllFalse { table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(
                        *result,
                        MaskProvenance::constant(table.clone(), *rows, MaskValueKind::AllFalse),
                    );
                }
            }
            _ => {}
        }
    }
    map
}
```

- [ ] **Step 4: Implement `m & true -> m` as the first certified data-plane rule**

In `src/mir/rewrite.rs`, add `apply_mask_algebra` after scalar rewrites when `Pass::MaskAlgebra` is enabled:

```rust
fn apply_mask_algebra(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let provenance = mask_provenance_by_value(module);
    let authority = crate::mir::AuthorityQuery::new();
    for op_index in 0..module.operations().len() {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let op_id = OperationId::new(op_index as u32);
        let op = module.operation(op_id).clone();
        if op.kind() != &OperationKind::MaskAnd || op.operands().len() != 2 || op.results().len() != 1 {
            continue;
        }
        let left = op.operands()[0];
        let right = op.operands()[1];
        let Some(left_prov) = provenance.get(&left) else { continue };
        let Some(right_prov) = provenance.get(&right) else { continue };
        let Some(domain_fact) = authority.same_mask_domain(left_prov, right_prov) else {
            continue;
        };
        if right_prov.kind() != MaskValueKind::AllTrue {
            continue;
        }
        let Some(region_id) = containing_region(module, op_id) else {
            continue;
        };

        let before_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
        let mut candidate = module.clone();
        candidate.replace_value_uses(op.results()[0], left);
        let removed = remove_unused_pure_operations(&mut candidate);
        let verify = crate::mir::verify_module(&candidate);
        let after_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(&candidate));
        let outcome = if verify.ok() {
            RewriteOutcome::PassedVerifier
        } else {
            RewriteOutcome::FailedVerifier
        };
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::MaskAndTrue,
            crate::mir::Pass::MaskAlgebra,
            region_id,
            vec![op_id],
            before_hash,
            after_hash,
            vec![
                domain_fact.as_rewrite_fact(),
                RewriteFact::PureOperation { op: op_id },
                RewriteFact::VerifierPassed,
            ],
            outcome,
        );
        if cert.validate() != CertificateValidation::Valid {
            continue;
        }
        if !report.events.push(cert) {
            continue;
        }
        *module = candidate;
        report.rewrites_applied += 1;
        report.dead_ops_removed += removed;
        report.fuel_used += 1;
        *fuel -= 1;
    }
}
```

Reuse the `containing_region(module, op_id)` helper introduced in MIR 03 for every MIR 04 certificate so data-plane rewrites are attributed to the region that actually contains the rewritten operation.

Add the remaining identity rules using the same certificate pattern in this task:

```text
m | false -> m
m & m -> m
m | m -> m
!!m -> m
```

Each rule uses `AuthorityQuery::same_mask_domain` where two masks are compared. Constant-specific rules inspect `MaskProvenance::kind()` so `m & false` is not rewritten as `m & true`. `!!m -> m` uses the original mask's provenance and verifier fact.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test mir::rewrite::tests::mask_algebra_
```

Expected: both mask algebra tests pass.

**Acceptance criteria:**
- Rules never fire across different tables or row counts.
- Every applied mask rewrite has a certificate and event.
- The first test proves a successful authority fact is recorded.
- The second test proves mismatched provenance blocks the rewrite.
- Optimized MIR verifies after each rewrite.

---

### Task 6: Target Cost Seeds And Branch/Select

**Files:**
- Create: `src/mir/cost.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/mir/rewrite.rs`
- Test: `tests/mir_dataplane.rs`

**Description:** Add the first handwritten AArch64 cost profiles and branch/select decisions. This is a deterministic heuristic, not a learned model.

- [ ] **Step 1: Add failing cost tests**

Append to `tests/mir_dataplane.rs`:

```rust
#[test]
fn branch_select_cost_prefers_csel_outside_predictable_loops() {
    let generic = wrela::mir::TargetProfile::GenericAArch64.costs();
    assert!(generic.should_if_convert(false, false, 2));
    assert!(!generic.should_if_convert(true, true, 2));
    assert!(!generic.should_if_convert(false, false, 4));
}
```

- [ ] **Step 2: Implement cost profiles**

Create `src/mir/cost.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetProfile {
    GenericAArch64,
    CortexA78,
    NeoverseV2,
    AppleFirestorm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchSelectCost {
    pub csel_latency: u8,
    pub csel_throughput_per_cycle: u8,
    pub max_pure_ops: u8,
    pub predictable_branch_preferred_in_loop: bool,
}

impl TargetProfile {
    pub const fn name(self) -> &'static str {
        match self {
            TargetProfile::GenericAArch64 => "generic-aarch64",
            TargetProfile::CortexA78 => "cortex-a78",
            TargetProfile::NeoverseV2 => "neoverse-v2",
            TargetProfile::AppleFirestorm => "apple-firestorm",
        }
    }

    pub const fn costs(self) -> BranchSelectCost {
        match self {
            TargetProfile::GenericAArch64 => BranchSelectCost {
                csel_latency: 1,
                csel_throughput_per_cycle: 2,
                max_pure_ops: 3,
                predictable_branch_preferred_in_loop: true,
            },
            TargetProfile::CortexA78
            | TargetProfile::NeoverseV2
            | TargetProfile::AppleFirestorm => BranchSelectCost {
                csel_latency: 1,
                csel_throughput_per_cycle: 4,
                max_pure_ops: 3,
                predictable_branch_preferred_in_loop: true,
            },
        }
    }
}

impl BranchSelectCost {
    pub const fn should_if_convert(
        self,
        in_loop: bool,
        predictable_branch: bool,
        pure_ops: u8,
    ) -> bool {
        pure_ops <= self.max_pure_ops
            && !(in_loop && predictable_branch && self.predictable_branch_preferred_in_loop)
    }
}
```

- [ ] **Step 3: Export cost API**

Modify `src/mir/mod.rs`:

```rust
pub mod cost;

pub use cost::{BranchSelectCost, TargetProfile};
```

- [ ] **Step 4: Gate branch/select rewrite**

In `src/mir/rewrite.rs`, the `BranchSelect` pass fires only when:

```text
region is pure
candidate body is <= TargetProfile::GenericAArch64.costs().max_pure_ops scalar ops
branch is outside a loop, or branch is not statically predictable
pass_set.enabled(Pass::BranchSelect)
```

MIR 04 does not if-convert row-token loops. A fired branch/select rewrite emits a certificate with:

```text
rule = branch-select
facts = pure-operation, effect-absent, verifier-passed
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_dataplane branch_select_cost_prefers_csel_outside_predictable_loops
```

Expected: pass.

**Acceptance criteria:**
- Cost profile selection is deterministic.
- Branch/select rewrite is disabled for predictable loop-carried branches.
- Branch/select pass can be enabled, disabled, and isolated with MIR 03 pass controls.
- Any branch/select rewrite that fires emits a certificate and event.

---

### Task 7: Data-Plane Generated-Code Unsupported Boundary

**Files:**
- Modify: `src/mir/perf.rs`
- Modify: `tests/mir_perf.rs`
- Fixture: `fixtures/perf/filter_sum.wrela`

**Description:** Keep MIR 04 honest: data-plane IR rewrites exist, but executable data-plane AArch64 loop codegen is MIR 05. `wrela perf code` must reject the data-plane fixture with exit `2` and a deterministic message instead of emitting bogus runtime/checksum numbers.

- [ ] **Step 1: Copy data-plane perf-boundary fixture**

Create `fixtures/perf/filter_sum.wrela` with the same source contents as `fixtures/mir/dataplane/filter_sum.wrela` from Task 1.

- [ ] **Step 2: Add failing unsupported-boundary test**

Append to `tests/mir_perf.rs`:

```rust
#[test]
fn perf_code_reports_dataplane_codegen_unsupported() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "perf".into(),
            "code".into(),
            "--mode".into(),
            "release".into(),
            "--only-pass".into(),
            "mask-algebra".into(),
            "--repeat".into(),
            "7".into(),
            "--json".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(out.is_empty());
    let message = String::from_utf8(err).unwrap();
    assert!(message.contains("data-plane generated-code execution is MIR 05 work"));
}
```

- [ ] **Step 3: Classify unsupported data-plane modules**

In `src/mir/perf.rs`, after release optimization and before LIR lowering, reject modules containing data-plane operations:

```rust
fn contains_dataplane_codegen_required(module: &crate::mir::MirModule) -> bool {
    module.operations().iter().any(|op| {
        matches!(
            op.kind(),
            crate::mir::OperationKind::ReduceRows { .. }
                | crate::mir::OperationKind::RowToken { .. }
                | crate::mir::OperationKind::TableRows { .. }
                | crate::mir::OperationKind::MaskRead { .. }
                | crate::mir::OperationKind::MaskAllTrue { .. }
                | crate::mir::OperationKind::MaskAllFalse { .. }
                | crate::mir::OperationKind::MaskAnd
                | crate::mir::OperationKind::MaskOr
                | crate::mir::OperationKind::MaskNot
        )
    })
}
```

Use it in `measure_code`:

```rust
if contains_dataplane_codegen_required(module_for_codegen) {
    return Err("data-plane generated-code execution is MIR 05 work".to_string());
}
```

The command layer maps this exact error to exit `2`. It does not render JSON, checksums, or runtime fields for the data-plane fixture.

- [ ] **Step 4: Keep zero-arg scalar benchmarks working**

Run:

```bash
cargo test --test mir_perf perf_code_accepts_release_pass_controls_and_reports_rewrites
```

Expected: pass from MIR 03.

- [ ] **Step 5: Run focused unsupported-boundary test**

Run:

```bash
cargo test --test mir_perf perf_code_reports_dataplane_codegen_unsupported
```

Expected: pass.

**Acceptance criteria:**
- Data-plane generated-code perf exits `2` with `data-plane generated-code execution is MIR 05 work`.
- No data-plane perf command in MIR 04 emits runtime or checksum JSON.
- Existing zero-arg scalar generated-code perf tests still pass.
- MIR 05 must add LIR load/compare/branch opcodes, mask-bit tests, emitter/regalloc/verifier support, and a parameterized C harness before data-plane runtime A/B claims are allowed.

---

### Task 8: IR Rewrite Evidence And Final Handoff

**Files:**
- Modify: `src/mir/rewrite.rs`
- Modify: `scripts/perf-compare.sh`

**Description:** Produce IR-level evidence for MIR 04 data-plane rewrites and complete review. Runtime A/B evidence for data-plane code is explicitly out of scope until MIR 05.

- [ ] **Step 1: Add IR certificate report unit test**

Append to the `#[cfg(test)]` module in `src/mir/rewrite.rs` so it can use the crate-internal `dataplane::testing` helpers:

```rust
#[test]
fn dataplane_release_reports_mask_certificate_details() {
    let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
    let passes = crate::mir::PassSet::only(crate::mir::Pass::MaskAlgebra);
    let optimized = crate::mir::optimize_release(&module, &passes);

    assert_eq!(optimized.report().rewrites_applied(), 1);
    assert_eq!(optimized.report().rewrite_events().len(), 1);
    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "mask-and-true");
    assert!(cert
        .facts()
        .iter()
        .any(|fact| format!("{fact:?}").contains("same-mask-domain:packets:256")));
    assert!(crate::mir::verify_module(optimized.module()).ok());
}
```

- [ ] **Step 2: Add local comparison script**

Create or update `scripts/perf-compare.sh` so it refuses data-plane runtime comparisons in MIR 04:

```bash
#!/usr/bin/env bash
set -euo pipefail

fixture="${1:-fixtures/perf/filter_sum.wrela}"

if [[ "$fixture" == *"filter_sum.wrela" ]]; then
  echo "data-plane runtime comparison is MIR 05 work; use MIR rewrite certificates in MIR 04" >&2
  exit 2
fi

cargo run -- perf code --mode release --disable-pass scalar-peephole --repeat 7 --json "$fixture"
cargo run -- perf code --mode release --only-pass scalar-peephole --repeat 7 --json "$fixture"
```

- [ ] **Step 3: Run data-plane unsupported perf smoke command**

Run:

```bash
cargo run -- perf code --mode release --only-pass mask-algebra --repeat 7 --json fixtures/perf/filter_sum.wrela
```

Expected: exit `2` with `data-plane generated-code execution is MIR 05 work` on every host.

- [ ] **Step 4: Run full local quality gate**

Run:

```bash
./scripts/quality-gate.sh
```

Expected: pass.

- [ ] **Step 5: Complete Phase A self review**

Use the repository review workflow in `docs/implementation/reviews/README.md`. Fix every finding at every priority unless the handoff message documents an explicit disagreement.

**Acceptance criteria:**
- `./scripts/quality-gate.sh` passes.
- Data-plane IR rewrite evidence is certificate-based.
- Runtime data-plane performance claims are not made in MIR 04.
- Phase A verdict is APPROVED in the handoff message before user handoff.

## Self-Review Checklist

- [ ] Checked `Table[T,N]` and `Mask[table,N]` semantics reject missing, later, and mismatched provenance.
- [ ] `Table[Packet, 256]` keeps `Packet` as a type arg, while `Mask[packets, 256]` resolves `packets` as an earlier parameter arg.
- [ ] Type summary path extraction does not include generic child identifiers in the outer path.
- [ ] `build_mir` emits table/mask/row/reduce ops for all three fixture reductions and returns `total + flagged + small_total`.
- [ ] Data-plane MIR facts round-trip through text.
- [ ] Data-plane test builders push `ValueData` before using `ValueId`s.
- [ ] Authority query V0 owns table/mask/effect legality checks used by data-plane rewrites.
- [ ] Mask identity rewrites never fire across different tables or row counts.
- [ ] Mask identity rewrites distinguish `MaskAllTrue` from `MaskAllFalse`.
- [ ] Every applied data-plane rewrite has a valid versioned certificate and bounded event.
- [ ] Branch/select cost seeds are deterministic.
- [ ] Data-plane generated-code perf exits `2` with the approved message.
- [ ] No runtime data-plane speed claim appears in the MIR 04 handoff.
- [ ] No external crates were added.
