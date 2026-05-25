# Check 04: Ownership, Effects, Layout, And Final Product Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Wrela's first ownership, effect, and layout checks, then lock the diagnostic product bar with examples, docs, quality gate, and Phase A review.

**Architecture:** `check/ownership.rs` reuses summaries, signatures, and body CSTs to validate access and moves. `check/effects.rs` infers first-pass effect facts and context policy violations. `check/layout.rs` validates `layout C data` field legality. `check/report.rs` exposes reviewable semantic facts for future `wrela report` without adding a CLI command in this plan.

**Tech Stack:** Rust 2024, standard library only, existing checker artifacts from Plans 01-03.

---

## File Ownership

```text
src/check/
  ownership.rs   access and move checks for the supported body subset
  effects.rs     effect facts and context policy diagnostics
  layout.rs      layout data legality checks
  report.rs      structured semantic facts for future report/query commands
  mod.rs         pipeline wiring
  json.rs        final complete diagnostic JSON arrays
tests/
  check.rs       ownership/effect/layout/product-bar tests
fixtures/check/
  diagnostics/  reviewable examples used by docs and CLI smoke tests
docs/
  AGENTS.md              check CLI and agent rules
  design-principles.md
```

## Real Parallel Work

- Task 1 owns `ownership.rs` and runs first because it uses body/type facts.
- Task 2 (`effects.rs`) and Task 3 (`layout.rs`) may run in parallel after Task 1 for their owned checker files and fixtures.
- Task 4 (`report.rs`, docs, fixtures) depends on Tasks 1-3.
- Task 5 is final verification and Phase A handoff.

Parallel subagents should not both edit `src/check/mod.rs` or `tests/check.rs`. Let the integration owner add module registrations and consolidate integration tests when merging branches.

Subagents may run the focused tests listed in their task with a timeout set in the execution tool. Recommended focused-command timeout: 120 seconds. The orchestrator runs `./scripts/quality-gate.sh` after each merge and strict mode before handoff.

---

## First-Pass Semantic Decisions

Ownership:

- `read` values may be read but not moved.
- `mut` values may be read and mutated but not moved.
- `own` values may be moved once.
- Returning a non-`None` value moves it to the caller.
- `let name = value` moves `value` when `value` is an owned local or owned parameter.
- Reusing a moved value emits `W-OWN-MOVE`.
- Moving a `read` or `mut` value emits `W-OWN-ACCESS`.
- Unsupported expression forms do not silently pass ownership. They produce unknown ownership facts and the existing unsupported diagnostics.

Effects:

- Supported pure expressions and local lets have no effects.
- Unsupported calls, loops, `try`, `reduce`, `scan`, and unchecked expression forms produce `EffectKind::Unknown`.
- Image `phase` bodies reject unknown effects with `W-EFFECT-UNSUPPORTED`.
- Tests reject unknown effects with `W-EFFECT-UNSUPPORTED`.

Layout:

- `layout C data` fields may use only `Bool`, `I64`, `U32`, or `U64` in V1.
- `String`, user-defined item types, unknown types, generic types, and access-qualified fields are invalid in `layout C data`.
- Ordinary `data` is not subject to layout C restrictions.

---

### Task 1: Ownership Checks

**Files:**
- Create: `src/check/ownership.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Validate first-pass ownership and access rules over the body subset checked in Plan 03.

- [ ] **Step 1: Write failing ownership tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_reports_use_after_move() {
    let dir = temp_dir("check-use-after-move");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(own bytes: Bytes) -> Bytes {\n  let saved = bytes\n  return bytes\n} }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-OWN-MOVE]"));
    assert!(rendered.contains("value was moved here"));
}

#[test]
fn check_reports_moving_read_parameter() {
    let dir = temp_dir("check-read-move");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(read bytes: Bytes) -> Bytes { return bytes } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-OWN-ACCESS]"));
    assert!(rendered.contains("`read` value cannot be moved"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because ownership checks are not wired.

- [ ] **Step 3: Add ownership result and state**

Create `src/check/ownership.rs`:

```rust
use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::source::Span;
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::summary::{AccessMode, MemberSummary, Name};
use super::types::{BuiltinType, CheckedSignature, SignatureCheck};

#[derive(Clone, Debug)]
pub struct OwnershipCheck {
    diagnostics: Vec<Diagnostic>,
}

impl OwnershipCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct OwnershipContext<'a> {
    pub modules: Vec<super::ModuleInput<'a>>,
    pub signatures: &'a SignatureCheck,
}

#[derive(Clone, Debug)]
struct OwnershipValue {
    name: Name,
    access: AccessMode,
    moved_at: Option<Span>,
}
```

- [ ] **Step 4: Implement ownership pass**

Add:

```rust
pub fn check_ownership(context: OwnershipContext<'_>) -> OwnershipCheck {
    let mut checker = OwnershipChecker {
        signatures: context.signatures,
        diagnostics: Vec::new(),
    };

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                if let Some(body) = member.body() {
                    checker.check_member(&view, member, body);
                }
            }
        }
    }

    OwnershipCheck {
        diagnostics: checker.diagnostics,
    }
}

struct OwnershipChecker<'a> {
    signatures: &'a SignatureCheck,
    diagnostics: Vec<Diagnostic>,
}

impl OwnershipChecker<'_> {
    fn check_member(&mut self, view: &CstView<'_>, member: &MemberSummary, body: SyntaxNodeId) {
        let Some(signature) = self.signatures.member_signature(member) else {
            return;
        };
        let mut values = BTreeMap::<String, OwnershipValue>::new();
        for param in signature.params() {
            values.insert(
                param.name().text().to_string(),
                OwnershipValue {
                    name: param.name().clone(),
                    access: param.access(),
                    moved_at: None,
                },
            );
        }
        self.check_block(view, body, signature, &mut values);
    }

    fn check_block(
        &mut self,
        view: &CstView<'_>,
        block: SyntaxNodeId,
        signature: &CheckedSignature,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.check_let(view, stmt, values),
                SyntaxKind::ReturnStmt => self.check_return(view, stmt, signature, values),
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 5: Implement move rules**

Add:

```rust
impl OwnershipChecker<'_> {
    fn check_let(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        let Some(name_token) = view.first_identifier_text(stmt) else {
            return;
        };
        if let Some(expr) = first_expr_child(view, stmt) {
            self.consume_expr_as_move(view, expr, values);
        }
        values.insert(
            name_token.text().to_string(),
            OwnershipValue {
                name: Name::new(name_token.text(), name_token.span()),
                access: AccessMode::Own,
                moved_at: None,
            },
        );
    }

    fn check_return(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        signature: &CheckedSignature,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        if signature.return_type() == self.signatures.type_table().builtin(BuiltinType::None) {
            return;
        }
        if let Some(expr) = first_expr_child(view, stmt) {
            self.consume_expr_as_move(view, expr, values);
        }
    }

    fn consume_expr_as_move(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        if view.node_kind(expr) != SyntaxKind::NameExpr {
            return;
        }
        let Some(token) = view.first_identifier_text(expr) else {
            return;
        };
        let Some(value) = values.get_mut(token.text()) else {
            return;
        };
        if let Some(moved_at) = value.moved_at {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::OwnershipMove,
                    format!("value `{}` used after move", token.text()),
                )
                .primary(token.span(), "value used after it was moved")
                .secondary(moved_at, "value was moved here")
                .finish(),
            );
            return;
        }
        match value.access {
            AccessMode::Own => value.moved_at = Some(token.span()),
            AccessMode::Read | AccessMode::Mut => self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::OwnershipAccess,
                    format!("`{:?}` value `{}` cannot be moved", value.access, token.text()),
                )
                .primary(token.span(), "`read` value cannot be moved")
                .secondary(value.name.span(), "access mode is declared here")
                .finish(),
            ),
        }
    }
}

fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node).into_iter().find(|child| {
        matches!(
            view.node_kind(*child),
            SyntaxKind::NameExpr
                | SyntaxKind::LiteralExpr
                | SyntaxKind::ParenExpr
                | SyntaxKind::PrefixExpr
                | SyntaxKind::BinaryExpr
                | SyntaxKind::CallExpr
                | SyntaxKind::FieldExpr
                | SyntaxKind::IndexExpr
                | SyntaxKind::TryExpr
                | SyntaxKind::ReduceExpr
                | SyntaxKind::ScanExpr
        )
    })
}
```

- [ ] **Step 6: Wire ownership into pipeline**

Update `src/check/mod.rs`:

```rust
pub mod ownership;

use crate::check::ownership::{OwnershipCheck, OwnershipContext, check_ownership};

pub struct CheckResult {
    // existing fields
    ownership_check: OwnershipCheck,
}

impl CheckResult {
    pub fn ownership_check(&self) -> &OwnershipCheck {
        &self.ownership_check
    }
}
```

Reuse the `module_inputs` helper from Plan 03 and append ownership diagnostics after body diagnostics:

```rust
let ownership_check = check_ownership(OwnershipContext {
    modules: module_inputs(&summaries, &parsed, discovered.lexed_files(), &source_map),
    signatures: &signature_check,
});
diagnostics.extend(ownership_check.diagnostics().iter().cloned());
```

- [ ] **Step 7: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- Owned values move once.
- `read` and `mut` values cannot be moved.
- Move diagnostics include primary and secondary spans.
- Unsupported body expressions do not produce false ownership success.

---

### Task 2: Effect Facts And Context Policies

**Files:**
- Create: `src/check/effects.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Infer first-pass effect facts and reject unknown effects in phases and tests.

- [ ] **Step 1: Write failing effect tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_reports_unknown_effect_in_phase() {
    let dir = temp_dir("check-phase-effect");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub host image Mac {}\npub image Boot target Mac { phase start() { loop { return } } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-EFFECT-UNSUPPORTED]"));
    assert!(rendered.contains("phase bodies cannot carry unknown effects"));
}
```

- [ ] **Step 2: Run test and verify failure**

```bash
cargo test --test check check_reports_unknown_effect_in_phase
```

Expected result: test fails because effect checks are not wired.

- [ ] **Step 3: Add effect model**

Create `src/check/effects.rs`:

```rust
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::summary::MemberKind;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet {
    unknown: bool,
}

impl EffectSet {
    pub const fn pure() -> Self {
        Self { unknown: false }
    }

    pub const fn unknown() -> Self {
        Self { unknown: true }
    }

    pub const fn has_unknown(self) -> bool {
        self.unknown
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            unknown: self.unknown || other.unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EffectCheck {
    diagnostics: Vec<Diagnostic>,
}

impl EffectCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct EffectContext<'a> {
    pub modules: Vec<super::ModuleInput<'a>>,
}
```

- [ ] **Step 4: Implement effect pass**

Add:

```rust
pub fn check_effects(context: EffectContext<'_>) -> EffectCheck {
    let mut diagnostics = Vec::new();

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                let Some(body) = member.body() else {
                    continue;
                };
                let effects = infer_block_effects(&view, body);
                if effects.has_unknown()
                    && matches!(member.kind(), MemberKind::Phase | MemberKind::Test)
                {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::EffectUnsupported,
                            "body has unknown effects",
                        )
                        .primary(member.span(), context_message(member.kind()))
                        .finish(),
                    );
                }
            }
        }
    }

    EffectCheck { diagnostics }
}

fn context_message(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Phase => "phase bodies cannot carry unknown effects",
        MemberKind::Test => "test bodies cannot carry unknown effects",
        _ => "body cannot carry unknown effects",
    }
}

fn infer_block_effects(view: &CstView<'_>, block: SyntaxNodeId) -> EffectSet {
    let mut effects = EffectSet::pure();
    for child in view.child_nodes(block) {
        effects = effects.union(infer_node_effects(view, child));
    }
    effects
}

fn infer_node_effects(view: &CstView<'_>, node: SyntaxNodeId) -> EffectSet {
    match view.node_kind(node) {
        SyntaxKind::CallExpr
        | SyntaxKind::LoopStmt
        | SyntaxKind::TryExpr
        | SyntaxKind::ReduceExpr
        | SyntaxKind::ScanExpr => EffectSet::unknown(),
        _ => {
            let mut effects = EffectSet::pure();
            for child in view.child_nodes(node) {
                effects = effects.union(infer_node_effects(view, child));
            }
            effects
        }
    }
}
```

- [ ] **Step 5: Wire effects into pipeline**

Update `src/check/mod.rs`:

```rust
pub mod effects;

use crate::check::effects::{EffectCheck, EffectContext, check_effects};

pub struct CheckResult {
    // existing fields
    effect_check: EffectCheck,
}

impl CheckResult {
    pub fn effect_check(&self) -> &EffectCheck {
        &self.effect_check
    }
}
```

Reuse the `module_inputs` helper from Plan 03 and append effect diagnostics after ownership diagnostics:

```rust
let effect_check = check_effects(EffectContext {
    modules: module_inputs(&summaries, &parsed, discovered.lexed_files(), &source_map),
});
diagnostics.extend(effect_check.diagnostics().iter().cloned());
```

- [ ] **Step 6: Run focused test**

```bash
cargo test --test check check_reports_unknown_effect_in_phase
```

Expected result: test passes.

**Acceptance Criteria:**
- Effect inference is deterministic and read-only.
- Unknown effects are explicit facts, not silent success.
- Phase and test contexts reject unknown effects.

---

### Task 3: Layout C Data Checks

**Files:**
- Create: `src/check/layout.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Validate first-pass `layout C data` field types.

- [ ] **Step 1: Write failing layout tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_accepts_primitive_layout_c_data() {
    let dir = temp_dir("check-layout-ok");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub layout C data Packet { value: U32 flag: Bool }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}

#[test]
fn check_rejects_string_in_layout_c_data() {
    let dir = temp_dir("check-layout-string");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub layout C data Packet { name: String }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-LAYOUT-INVALID]"));
    assert!(rendered.contains("layout C fields must be fixed primitive scalars"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: the rejection test fails until layout checks are wired.

- [ ] **Step 3: Add layout checker**

Create `src/check/layout.rs`:

```rust
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};

use super::summary::{ItemKind, CheckModuleSummary};
use super::types::{BuiltinType, SignatureCheck, TypeId, TypeKind};

#[derive(Clone, Debug)]
pub struct LayoutCheck {
    diagnostics: Vec<Diagnostic>,
}

impl LayoutCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub fn check_layouts(summaries: &[CheckModuleSummary], signatures: &SignatureCheck) -> LayoutCheck {
    let mut diagnostics = Vec::new();
    for summary in summaries {
        for item in summary.items() {
            if item.kind() != ItemKind::LayoutData {
                continue;
            }
            for field in item.fields() {
                let Some(ty) = signatures.field_type(field) else {
                    continue;
                };
                let qualified = field
                    .field_type()
                    .is_some_and(|field_type| field_type.access().is_some() || field_type.unique());
                if qualified || !is_layout_c_scalar(signatures, ty) {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::LayoutInvalid,
                            "invalid layout C field type",
                        )
                        .primary(field.span(), "layout C fields must be fixed primitive scalars")
                        .finish(),
                    );
                }
            }
        }
    }
    LayoutCheck { diagnostics }
}

fn is_layout_c_scalar(signatures: &SignatureCheck, ty: TypeId) -> bool {
    matches!(
        signatures.type_table().kind(ty),
        Some(TypeKind::Builtin(
            BuiltinType::Bool | BuiltinType::I64 | BuiltinType::U32 | BuiltinType::U64
        ))
    )
}
```

- [ ] **Step 4: Wire layouts into pipeline**

Update `src/check/mod.rs`:

```rust
pub mod layout;

use crate::check::layout::{LayoutCheck, check_layouts};

pub struct CheckResult {
    // existing fields
    layout_check: LayoutCheck,
}

impl CheckResult {
    pub fn layout_check(&self) -> &LayoutCheck {
        &self.layout_check
    }
}
```

Append layout diagnostics after effect diagnostics:

```rust
let layout_check = check_layouts(&summaries, &signature_check);
diagnostics.extend(layout_check.diagnostics().iter().cloned());
```

- [ ] **Step 5: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- `layout C data` with Bool/I64/U32/U64 fields passes.
- `layout C data` with String, item types, unknown types, or generic types fails.
- Ordinary `data` declarations are not rejected by layout C rules.

---

### Task 4: Reportable Semantic Facts And Diagnostic Product Bar

**Files:**
- Create: `src/check/report.rs`
- Modify: `src/check/mod.rs`
- Modify: `src/check/json.rs`
- Modify: `src/diagnostic.rs`
- Create: `fixtures/check/diagnostics/unknown-type.wrela`
- Create: `fixtures/check/diagnostics/use-after-move.wrela`
- Create: `fixtures/check/diagnostics/layout-invalid.wrela`
- Modify: `AGENTS.md`

**Description:** Add structured semantic facts for future agentic commands and lock concrete diagnostic examples into fixtures and docs.

- [ ] **Step 1: Add report facts**

Create `src/check/report.rs`:

```rust
#[derive(Clone, Debug, Default)]
pub struct SemanticReport {
    checked_modules: usize,
    checked_items: usize,
    checked_bodies: usize,
    ownership_diagnostics: usize,
    effect_diagnostics: usize,
    layout_diagnostics: usize,
}

impl SemanticReport {
    pub const fn new(
        checked_modules: usize,
        checked_items: usize,
        checked_bodies: usize,
        ownership_diagnostics: usize,
        effect_diagnostics: usize,
        layout_diagnostics: usize,
    ) -> Self {
        Self {
            checked_modules,
            checked_items,
            checked_bodies,
            ownership_diagnostics,
            effect_diagnostics,
            layout_diagnostics,
        }
    }

    pub const fn checked_modules(&self) -> usize { self.checked_modules }
    pub const fn checked_items(&self) -> usize { self.checked_items }
    pub const fn checked_bodies(&self) -> usize { self.checked_bodies }
    pub const fn ownership_diagnostics(&self) -> usize { self.ownership_diagnostics }
    pub const fn effect_diagnostics(&self) -> usize { self.effect_diagnostics }
    pub const fn layout_diagnostics(&self) -> usize { self.layout_diagnostics }
}
```

Wire `SemanticReport` into `CheckResult` as `semantic_report()`. This is not a `wrela report` command; it is the data foundation for that later command.

- [ ] **Step 2: Create diagnostic fixtures**

Create `fixtures/check/diagnostics/unknown-type.wrela`:

```wrela
module root
pub unique class Console {}
pub data Port {
  value: Consol
}
```

Create `fixtures/check/diagnostics/use-after-move.wrela`:

```wrela
module root

pub data Bytes {
  value: U32
}

pub class C {
  fn m(own bytes: Bytes) -> Bytes {
    let saved = bytes
    return bytes
  }
}
```

Create `fixtures/check/diagnostics/layout-invalid.wrela`:

```wrela
module root

pub layout C data Packet {
  name: String
}
```

- [ ] **Step 3: Add product-bar tests**

Add to `tests/check.rs`:

```rust
#[test]
fn diagnostic_examples_have_rich_human_output() {
    for fixture in [
        "fixtures/check/diagnostics/unknown-type.wrela",
        "fixtures/check/diagnostics/use-after-move.wrela",
        "fixtures/check/diagnostics/layout-invalid.wrela",
    ] {
        let result = wrela::check::check_root(fixture);
        assert!(!result.ok(), "{fixture} should be invalid");
        let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
        assert!(rendered.contains("error["), "{fixture} missing code");
        assert!(rendered.contains("-->"), "{fixture} missing source reference");
        assert!(rendered.contains("^"), "{fixture} missing underline");
    }
}

#[test]
fn semantic_report_counts_checked_facts() {
    let dir = temp_dir("check-report");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(read self) -> U32 { return 0 } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
    assert_eq!(result.semantic_report().checked_modules(), 1);
    assert_eq!(result.semantic_report().checked_items(), 2);
    assert_eq!(result.semantic_report().checked_bodies(), 1);
}
```

- [ ] **Step 4: Complete JSON renderer**

Ensure every diagnostic field renders:

- `severity`
- `phase`
- `code`
- `message`
- `primary`
- `secondary`
- `related`
- `notes`
- `help`
- `suggestions`
- `group`

Plan 02 already renders secondary and related arrays. Plan 03 already renders suggestions and edits. In this task, replace the temporary empty `notes`, `help`, and `group` fields with:

```rust
out.push_str("\"notes\":");
render_string_array(out, diagnostic.notes());
comma(out);
out.push_str("\"help\":");
render_string_array(out, diagnostic.help());
comma(out);
out.push_str("\"suggestions\":");
render_suggestions(out, diagnostic, source_map);
comma(out);
out.push_str("\"group\":");
render_group(out, diagnostic);
```

Add:

```rust
fn render_group(out: &mut String, diagnostic: &Diagnostic) {
    match diagnostic.group() {
        Some(group) => {
            out.push('{');
            field_u32(out, "id", group.id().raw());
            comma(out);
            field_str(out, "role", group_role_text(group.role()));
            out.push('}');
        }
        None => out.push_str("null"),
    }
}

fn group_role_text(role: DiagnosticGroupRole) -> &'static str {
    match role {
        DiagnosticGroupRole::RootCause => "rootCause",
        DiagnosticGroupRole::Cascade => "cascade",
    }
}
```

The exact suggestion JSON shape is:

```json
{
  "title": "replace with `Console`",
  "applicability": "likely",
  "edits": [
    {
      "span": {
        "fileId": 0,
        "start": 50,
        "end": 56,
        "startLine": 4,
        "startColumn": 10,
        "endLine": 4,
        "endColumn": 16,
        "sourceHash": "..."
      },
      "replacement": "Console"
    }
  ]
}
```

- [ ] **Step 5: Update docs**

Update [`AGENTS.md`](../../AGENTS.md) with the `wrela check` section and these rules:

```markdown
- `wrela check` is read-only; formatting is a separate future command.
- `wrela check` emits JSON by default. Human diagnostics require `--human`.
- Diagnostic codes are stable and live in `DiagnosticCode` (`src/diagnostic.rs`).
- Suggested fixes must include applicability and source-hash preconditions.
- MIR lowering must consume semantic artifacts, not canonical source text.
```

Ensure `DiagnosticCode` in `src/diagnostic.rs` covers every code emitted by ownership, effects, and layout checks.

- [ ] **Step 6: Run focused product tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- Diagnostic examples are fixtures, not only prose.
- Every diagnostic example has code, source reference, underline, and useful explanatory text.
- `SemanticReport` provides agent-useful facts without adding a new CLI command.
- Docs lock JSON default, read-only check, diagnostic code registry, and MIR independence.

---

### Task 5: Final Quality Gate And Phase A Handoff

**Files:**
- Review outputs under `docs/implementation/reviews/` as required by the review workflow.

**Description:** Prove the full check product boundary is production quality before user handoff.

- [ ] **Step 1: Run full check tests**

```bash
cargo test --test check
```

Expected result: all check integration tests pass.

- [ ] **Step 2: Run quality gate**

```bash
./scripts/quality-gate.sh
```

Expected result: quality gate passes. Interpret failures as local CI failures; fix them before continuing.

- [ ] **Step 3: Run strict quality gate**

```bash
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

Expected result: strict quality gate passes before final handoff or merge. If review artifacts are intentionally untracked at this point, run strict mode after the review workflow records the required files and all intended files are staged or committed.

- [ ] **Step 4: Complete Phase A review**

Run the Phase A workflow in `docs/implementation/reviews/README.md`: adversarial self review, fix findings, hand off only on honest **`Verdict: APPROVED`**. Document explicit disagreements with rationale when skipping an item.

Phase A means a self-review that looks for correctness bugs, missing tests, locked-decision violations, diagnostic quality failures, and maintainability risks before user feedback.

- [ ] **Step 5: Commit**

```bash
git add src/check src/diagnostic.rs tests/check.rs fixtures/check docs/design docs/implementation
git commit -m "feat: add ownership effects layout check product bar -Codex Automated"
```

**Final Acceptance Criteria:**
- `wrela check <root.wrela>` emits JSON by default.
- `wrela check --human <root.wrela>` emits rich diagnostics.
- Name resolution, type checking, ownership, effect, and layout diagnostics are all structured data.
- Repeated unknown-name cascades are grouped and derivative type mismatches are suppressed.
- Suggested fixes include applicability and source-hash metadata.
- Valid parser-supported smoke fixtures pass through ownership/effects/layout.
- Unsupported parser constructs produce explicit diagnostics.
- `./scripts/quality-gate.sh` passes.
- `QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh` passes before merge.
- Phase A honestly **APPROVED** (no unresolved findings).
