# Check 03: Typechecking And Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add declaration, signature, expression, and body type checking with permissive recovery, unknown facts, cascade suppression, and useful suggested fixes.

**Architecture:** `check/types.rs` owns the type universe and signature validation. `check/body.rs` owns statement/expression checking and unknown-name recovery. `check/resolve.rs` gains type-name lookup helpers. Diagnostics stay structured and are rendered by the existing JSON/human renderers.

**Tech Stack:** Rust 2024, standard library only, existing semantic summaries and resolved graph.

---

## File Ownership

```text
src/check/
  types.rs     type IDs, builtins, signature validation
  body.rs      body checker, scopes, unknown facts, cascade suppression
  resolve.rs   graph lookup helpers used by typechecking
  mod.rs       pipeline wiring for type and body checks
  json.rs      suggestion and related-location JSON rendering
src/
  diagnostic.rs  add safe related-location extension helper if needed
tests/
  check.rs     type, recovery, suggestion behavior
```

## Real Parallel Work

- Task 1 owns `types.rs` and runs first.
- Task 2 depends on Task 1 and owns signature validation.
- Task 3 depends on Task 2 and owns body checking.
- Task 4 depends on Task 3 and owns cascade suppression plus suggestions.
- Task 5 runs last.

This plan is intentionally sequential because each layer consumes the previous layer's facts.

Subagents may run the focused tests listed in their task with a timeout set in the execution tool. Recommended focused-command timeout: 120 seconds. The orchestrator runs `./scripts/quality-gate.sh` after merging each task.

---

### Task 1: Type Model And Type Name Lookup

**Files:**
- Create: `src/check/types.rs`
- Modify: `src/check/mod.rs`
- Modify: `src/check/resolve.rs`

**Description:** Define type IDs, builtin types, unknown type facts, and graph helpers for resolving type names.

- [ ] **Step 1: Write failing type model tests**

Add to `src/check/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_stable_type_ids() {
        let table = TypeTable::with_builtins();

        assert_eq!(table.builtin(BuiltinType::Bool), TypeId::new(0));
        assert_eq!(table.builtin(BuiltinType::I64), TypeId::new(1));
        assert_eq!(table.builtin(BuiltinType::U32), TypeId::new(2));
        assert_eq!(table.builtin(BuiltinType::U64), TypeId::new(3));
        assert_eq!(table.builtin(BuiltinType::String), TypeId::new(4));
        assert_eq!(table.builtin(BuiltinType::None), TypeId::new(5));
        assert!(table.is_integer(table.builtin(BuiltinType::U32)));
    }

    #[test]
    fn unknown_types_are_distinct_and_marked_unknown() {
        let mut table = TypeTable::with_builtins();
        let first = table.push_unknown(UnknownReason::UnresolvedTypeName);
        let second = table.push_unknown(UnknownReason::UnresolvedExpressionName);

        assert_ne!(first, second);
        assert!(table.is_unknown(first));
        assert!(table.is_unknown(second));
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test check::types::tests
```

Expected result: compilation fails because `types.rs` does not exist.

- [ ] **Step 3: Implement type model**

Create `src/check/types.rs`:

```rust
use std::collections::BTreeMap;

use crate::diagnostic::{Applicability, Diagnostic, DiagnosticCode, Severity, SourceEdit};
use crate::source::Span;

use super::resolve::{ItemId, ResolvedGraph, ResolvedItem, ResolvedModule, SymbolKind};
use super::summary::{MemberSummary, CheckModuleSummary, Name, TypeRefSummary};
use super::suggest::nearest_name;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypeId(u32);

impl TypeId {
    pub const fn new(raw: u32) -> Self { Self(raw) }
    pub const fn raw(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinType {
    Bool,
    I64,
    U32,
    U64,
    String,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    UnresolvedTypeName,
    UnresolvedExpressionName,
    UnsupportedExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Builtin(BuiltinType),
    Item(ItemId, SymbolKind),
    Unknown(UnknownReason),
}

#[derive(Clone, Debug)]
pub struct TypeTable {
    kinds: Vec<TypeKind>,
    builtin_ids: BTreeMap<BuiltinType, TypeId>,
}
```

Add methods:

```rust
impl TypeTable {
    pub fn with_builtins() -> Self {
        let mut table = Self {
            kinds: Vec::new(),
            builtin_ids: BTreeMap::new(),
        };
        for builtin in [
            BuiltinType::Bool,
            BuiltinType::I64,
            BuiltinType::U32,
            BuiltinType::U64,
            BuiltinType::String,
            BuiltinType::None,
        ] {
            let id = table.push(TypeKind::Builtin(builtin));
            table.builtin_ids.insert(builtin, id);
        }
        table
    }

    pub fn push(&mut self, kind: TypeKind) -> TypeId {
        let id = TypeId::new(self.kinds.len() as u32);
        self.kinds.push(kind);
        id
    }

    pub fn push_unknown(&mut self, reason: UnknownReason) -> TypeId {
        self.push(TypeKind::Unknown(reason))
    }

    pub fn builtin(&self, builtin: BuiltinType) -> TypeId {
        self.builtin_ids[&builtin]
    }

    pub fn kind(&self, id: TypeId) -> Option<TypeKind> {
        self.kinds.get(id.raw() as usize).copied()
    }

    pub fn is_unknown(&self, id: TypeId) -> bool {
        matches!(self.kind(id), Some(TypeKind::Unknown(_)))
    }

    pub fn is_integer(&self, id: TypeId) -> bool {
        matches!(
            self.kind(id),
            Some(TypeKind::Builtin(BuiltinType::I64 | BuiltinType::U32 | BuiltinType::U64))
        )
    }
}
```

- [ ] **Step 4: Add graph type lookup helpers**

In `src/check/resolve.rs`, add:

```rust
impl SymbolKind {
    pub const fn is_type(self) -> bool {
        matches!(
            self,
            SymbolKind::Data
                | SymbolKind::LayoutData
                | SymbolKind::Interface
                | SymbolKind::Class
                | SymbolKind::UniqueClass
                | SymbolKind::Error
                | SymbolKind::BuiltinType
        )
    }
}

impl ResolvedModule {
    pub fn resolve_any(&self, name: &str) -> Option<ItemId> {
        self.resolve_local(name).or_else(|| self.resolve_imported(name))
    }

    pub fn visible_names(&self) -> Vec<&str> {
        let mut names = self
            .local_symbols
            .keys()
            .map(String::as_str)
            .chain(self.imported_symbols.keys().map(String::as_str))
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }
}

impl ResolvedGraph {
    pub fn module_for_summary(&self, summary: &CheckModuleSummary) -> Option<&ResolvedModule> {
        self.module_by_path(&summary.module_path().as_dotted())
    }

    pub fn resolve_visible_type(
        &self,
        module: &ResolvedModule,
        name: &str,
    ) -> Option<(&ResolvedModule, &ResolvedItem)> {
        let item_id = module.resolve_any(name)?;
        let item = self.item(item_id)?;
        if item.kind().is_type() {
            let owner = self.modules().get(item_id.module().raw() as usize)?;
            Some((owner, item))
        } else {
            None
        }
    }
}
```

Make `ResolvedItem` visible in the import list if the compiler asks for it.

- [ ] **Step 5: Export module and run tests**

Add to `src/check/mod.rs`:

```rust
pub mod types;
```

Run:

```bash
cargo test check::types::tests
```

Expected result: tests pass.

**Acceptance Criteria:**
- Builtin type IDs are stable.
- Unknown types carry reasons.
- Type lookup distinguishes wrong-kind names.
- No MIR or layout assumptions enter the type model.

---

### Task 2: Signature Validation

**Files:**
- Modify: `src/check/types.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Validate type names in fields, parameters, return types, and implements clauses before body checking.

- [ ] **Step 1: Write failing signature tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_reports_unknown_type_with_suggestion() {
    let dir = temp_dir("check-type-suggestion");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console } from io\npub data Port { value: Consol }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-UNKNOWN]"));
    assert!(rendered.contains("did you mean `Console`?"));
    assert!(rendered.contains("suggestion (likely): replace with `Console`"));
}

#[test]
fn check_reports_wrong_kind_type_reference() {
    let dir = temp_dir("check-wrong-kind");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub image Boot target Mac { phase start(value: Boot) { return } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-RESOLVE-KIND"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because type validation is not wired.

- [ ] **Step 3: Add signature result types**

Add to `src/check/types.rs`:

```rust
#[derive(Clone, Debug)]
pub struct CheckedParam {
    name: Name,
    access: super::summary::AccessMode,
    ty: TypeId,
    span: Span,
}

#[derive(Clone, Debug)]
pub struct CheckedSignature {
    params: Vec<CheckedParam>,
    return_type: TypeId,
}

#[derive(Clone, Debug)]
pub struct SignatureCheck {
    type_table: TypeTable,
    field_types: BTreeMap<u32, TypeId>,
    member_signatures: BTreeMap<u32, CheckedSignature>,
    diagnostics: Vec<Diagnostic>,
}
```

Add getters:

```rust
impl CheckedParam {
    pub fn name(&self) -> &Name { &self.name }
    pub fn access(&self) -> super::summary::AccessMode { self.access }
    pub fn ty(&self) -> TypeId { self.ty }
    pub fn span(&self) -> Span { self.span }
}

impl CheckedSignature {
    pub fn params(&self) -> &[CheckedParam] { &self.params }
    pub fn return_type(&self) -> TypeId { self.return_type }
}

impl SignatureCheck {
    pub fn type_table(&self) -> &TypeTable { &self.type_table }
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }
    pub fn field_type(&self, field: &MemberSummary) -> Option<TypeId> {
        self.field_types.get(&field.node_id().raw()).copied()
    }
    pub fn member_signature(&self, member: &MemberSummary) -> Option<&CheckedSignature> {
        self.member_signatures.get(&member.node_id().raw())
    }
}
```

- [ ] **Step 4: Implement type-ref validation**

Add:

```rust
pub fn check_signatures(summaries: &[CheckModuleSummary], graph: &ResolvedGraph) -> SignatureCheck {
    let mut checker = SignatureChecker {
        graph,
        type_table: TypeTable::with_builtins(),
        field_types: BTreeMap::new(),
        member_signatures: BTreeMap::new(),
        diagnostics: Vec::new(),
    };

    for summary in summaries {
        let Some(module) = graph.module_for_summary(summary) else {
            continue;
        };
        for item in summary.items() {
            for implements in item.implements() {
                checker.resolve_type_ref(module, implements);
            }
            for member in item.members() {
                if let Some(field_type) = member.field_type() {
                    let ty = checker.resolve_type_ref(module, field_type);
                    checker.field_types.insert(member.node_id().raw(), ty);
                }
                if let Some(signature) = member.signature() {
                    let checked = checker.check_signature(module, signature);
                    checker.member_signatures.insert(member.node_id().raw(), checked);
                }
            }
        }
    }

    SignatureCheck {
        type_table: checker.type_table,
        field_types: checker.field_types,
        member_signatures: checker.member_signatures,
        diagnostics: checker.diagnostics,
    }
}

struct SignatureChecker<'a> {
    graph: &'a ResolvedGraph,
    type_table: TypeTable,
    field_types: BTreeMap<u32, TypeId>,
    member_signatures: BTreeMap<u32, CheckedSignature>,
    diagnostics: Vec<Diagnostic>,
}
```

Implement:

```rust
impl SignatureChecker<'_> {
    fn check_signature(
        &mut self,
        module: &ResolvedModule,
        signature: &super::summary::SignatureSummary,
    ) -> CheckedSignature {
        let mut params = Vec::new();
        for param in signature.params() {
            let ty = self.resolve_type_ref(module, param.ty());
            params.push(CheckedParam {
                name: param.name().clone(),
                access: param.access(),
                ty,
                span: param.span(),
            });
        }
        let return_type = signature
            .return_type()
            .map(|ty| self.resolve_type_ref(module, ty))
            .unwrap_or_else(|| self.type_table.builtin(BuiltinType::None));
        CheckedSignature { params, return_type }
    }

    fn resolve_type_ref(&mut self, module: &ResolvedModule, ty: &TypeRefSummary) -> TypeId {
        let name = ty.path_text();
        if let Some(builtin) = builtin_by_name(&name) {
            return self.type_table.builtin(builtin);
        }
        match self.graph.resolve_visible_type(module, &name) {
            Some((_owner, item)) => self.type_table.push(TypeKind::Item(item.id(), item.kind())),
            None => {
                let unknown = self.type_table.push_unknown(UnknownReason::UnresolvedTypeName);
                self.diagnostics.push(self.unknown_type_diagnostic(module, ty, &name));
                unknown
            }
        }
    }

    fn unknown_type_diagnostic(
        &self,
        module: &ResolvedModule,
        ty: &TypeRefSummary,
        name: &str,
    ) -> Diagnostic {
        let mut builder = Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::TypeUnknownType,
            format!("unknown type `{name}`"),
        )
        .primary(ty.span(), "type name is not in scope");

        if let Some(suggestion) = nearest_name(name, module.visible_names()) {
            builder = builder
                .help(format!("did you mean `{suggestion}`?"))
                .suggestion(
                    format!("replace with `{suggestion}`"),
                    Applicability::Likely,
                    vec![SourceEdit::replace(ty.span(), suggestion)],
                );
        }

        builder.finish()
    }
}

fn builtin_by_name(name: &str) -> Option<BuiltinType> {
    match name {
        "Bool" => Some(BuiltinType::Bool),
        "I64" => Some(BuiltinType::I64),
        "U32" => Some(BuiltinType::U32),
        "U64" => Some(BuiltinType::U64),
        "String" => Some(BuiltinType::String),
        "None" => Some(BuiltinType::None),
        _ => None,
    }
}
```

For wrong-kind references, add `ResolvedGraph::resolve_visible_any` in `resolve.rs`:

```rust
impl ResolvedGraph {
    pub fn resolve_visible_any(
        &self,
        module: &ResolvedModule,
        name: &str,
    ) -> Option<(&ResolvedModule, &ResolvedItem)> {
        let item_id = module.resolve_any(name)?;
        let item = self.item(item_id)?;
        let owner = self.modules().get(item_id.module().raw() as usize)?;
        Some((owner, item))
    }
}
```

Then update `resolve_type_ref` before the `resolve_visible_type` call:

```rust
if let Some((_owner, item)) = self.graph.resolve_visible_any(module, &name) {
    if !item.kind().is_type() {
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveWrongKind,
                format!("`{name}` is not a type"),
            )
            .primary(ty.span(), "this name cannot be used as a type")
            .secondary(item.span(), "name resolves here")
            .finish(),
        );
        return self.type_table.push_unknown(UnknownReason::UnresolvedTypeName);
    }
}
```

- [ ] **Step 5: Wire into pipeline**

Update `src/check/mod.rs`:

```rust
use crate::check::types::{SignatureCheck, check_signatures};

pub struct CheckResult {
    source_map: SourceMap,
    parsed: Vec<ParsedSyntax>,
    summaries: Vec<CheckModuleSummary>,
    resolved_graph: ResolvedGraph,
    signature_check: SignatureCheck,
    diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn signature_check(&self) -> &SignatureCheck {
        &self.signature_check
    }
}
```

After resolver:

```rust
let signature_check = check_signatures(&summaries, &resolved_graph);
diagnostics.extend(signature_check.diagnostics().iter().cloned());
```

- [ ] **Step 6: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- Field, parameter, return, and implements type refs are checked.
- Unknown type diagnostics include suggested fixes when a visible name is within edit distance 2.
- Wrong-kind type refs use `W-RESOLVE-KIND`.
- Unknown type IDs allow later phases to continue without derivative panics.

---

### Task 3: Body Type Checker

**Files:**
- Create: `src/check/body.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Check the supported statement/expression subset and emit explicit unsupported diagnostics for parsed constructs outside that subset.

Supported in this plan:

- blocks
- `let name = expr`
- `let name: Type = expr`
- `return`
- `return expr`
- expression statements
- integer and string literals
- `true`, `false`, and `None` as built-in name literals
- local/parameter names
- binary arithmetic and equality comparisons

Unsupported in this plan, with `W-CHECK-UNSUPPORTED`:

- calls
- field access
- indexing
- match/repeat/for/drain/loop/assert
- try/reduce/scan

- [ ] **Step 1: Write failing body tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_reports_return_type_mismatch() {
    let dir = temp_dir("check-return-type");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 { return \"no\" } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-RETURN]"));
    assert!(rendered.contains("expected `U32`"));
    assert!(rendered.contains("found `String`"));
}

#[test]
fn check_reports_unsupported_call_without_passing_silently() {
    let dir = temp_dir("check-unsupported-call");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 { return make() } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-CHECK-UNSUPPORTED]"));
    assert!(rendered.contains("call expressions are not checked yet"));
}

#[test]
fn check_reports_let_annotation_mismatch() {
    let dir = temp_dir("check-let-annotation");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 {\n  let value: U32 = \"no\"\n  return value\n} }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-MISMATCH]"));
    assert!(rendered.contains("let annotation expects `U32`"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because body checking is not wired.

- [ ] **Step 3: Add body result types**

Create `src/check/body.rs`:

```rust
use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::source::Span;
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::resolve::ResolvedGraph;
use super::summary::MemberSummary;
use super::types::{BuiltinType, SignatureCheck, TypeId, TypeTable, UnknownReason};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScopeId(u32);

#[derive(Clone, Debug)]
pub struct BodyCheckResult {
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct ValueSymbol {
    ty: TypeId,
}
```

Add the production body-check context and entry point:

```rust
impl BodyCheckResult {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct BodyCheckContext<'a> {
    pub modules: Vec<super::ModuleInput<'a>>,
    pub graph: &'a ResolvedGraph,
    pub signatures: &'a SignatureCheck,
}

pub fn check_bodies(context: BodyCheckContext<'_>) -> BodyCheckResult {
    let mut checker = BodyChecker {
        signatures: context.signatures,
        type_table: context.signatures.type_table().clone(),
        diagnostics: Vec::new(),
        next_scope: 0,
    };

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                if let Some(body) = member.body() {
                    checker.check_member_body(&view, member, body);
                }
            }
        }
    }

    BodyCheckResult {
        diagnostics: checker.diagnostics,
    }
}

struct BodyChecker<'a> {
    signatures: &'a SignatureCheck,
    type_table: TypeTable,
    diagnostics: Vec<Diagnostic>,
    next_scope: u32,
}
```

- [ ] **Step 4: Implement statement checking**

Use this checker shape:

```rust
impl BodyChecker<'_> {
    fn check_member_body(&mut self, view: &CstView<'_>, member: &MemberSummary, body: SyntaxNodeId) {
        let Some(signature) = self.signatures.member_signature(member) else {
            return;
        };
        let scope_id = self.fresh_scope();
        let mut scope = BTreeMap::<String, ValueSymbol>::new();
        for param in signature.params() {
            scope.insert(
                param.name().text().to_string(),
                ValueSymbol {
                    ty: param.ty(),
                },
            );
        }
        self.check_block(view, body, signature.return_type(), scope_id, &mut scope);
    }

    fn fresh_scope(&mut self) -> ScopeId {
        let id = ScopeId(self.next_scope);
        self.next_scope += 1;
        id
    }

    fn check_block(
        &mut self,
        view: &CstView<'_>,
        block: SyntaxNodeId,
        expected_return: TypeId,
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.check_let(view, stmt, scope_id, scope),
                SyntaxKind::ReturnStmt => self.check_return(view, stmt, expected_return, scope_id, scope),
                SyntaxKind::ExprStmt => {
                    if let Some(expr) = first_expr_child(view, stmt) {
                        self.check_expr(view, expr, scope_id, scope);
                    }
                }
                SyntaxKind::MatchStmt
                | SyntaxKind::RepeatStmt
                | SyntaxKind::ForStmt
                | SyntaxKind::DrainStmt
                | SyntaxKind::LoopStmt
                | SyntaxKind::AssertStmt => self.unsupported(view, stmt, "statement form is not checked yet"),
                _ => {}
            }
        }
    }
}
```

Implement `check_return`:

```rust
fn check_return(
    &mut self,
    view: &CstView<'_>,
    stmt: SyntaxNodeId,
    expected: TypeId,
    scope_id: ScopeId,
    scope: &mut BTreeMap<String, ValueSymbol>,
) {
    let actual = first_expr_child(view, stmt)
        .map(|expr| self.check_expr(view, expr, scope_id, scope))
        .unwrap_or_else(|| self.type_table.builtin(BuiltinType::None));
    if self.type_table.is_unknown(actual)
        || self.type_table.is_unknown(expected)
        || actual == expected
    {
        return;
    }
    self.diagnostics.push(
        Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::TypeReturn,
            "return type does not match signature",
        )
        .primary(view.node_span(stmt), "return expression has the wrong type")
        .note(format!(
            "expected `{}`, found `{}`",
            self.type_name(expected),
            self.type_name(actual)
        ))
        .finish(),
    );
}
```

Implement `check_let` so annotated lets compare annotation against initializer, and unannotated lets infer from initializer:

```rust
fn check_let(
    &mut self,
    view: &CstView<'_>,
    stmt: SyntaxNodeId,
    scope_id: ScopeId,
    scope: &mut BTreeMap<String, ValueSymbol>,
) {
    let Some(name_token) = view.first_identifier_text(stmt) else {
        return;
    };
    let Some(expr) = first_expr_child(view, stmt) else {
        return;
    };
    let ty = self.check_expr(view, expr, scope_id, scope);
    let annotation = view
        .first_child(stmt, SyntaxKind::TypeRef)
        .and_then(|type_ref| self.builtin_type_ref(view, type_ref));
    if let Some(expected) = annotation {
        if self.should_compare_types(expected, ty) && expected != ty {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeMismatch,
                    "let annotation does not match initializer",
                )
                .primary(view.node_span(stmt), format!("let annotation expects `{}`", self.type_name(expected)))
                .finish(),
            );
        }
    }
    scope.insert(
        name_token.text().to_string(),
        ValueSymbol {
            ty: annotation.unwrap_or(ty),
        },
    );
}
```

- [ ] **Step 5: Implement expression checking**

Use this expression core:

```rust
fn check_expr(
    &mut self,
    view: &CstView<'_>,
    expr: SyntaxNodeId,
    scope_id: ScopeId,
    scope: &mut BTreeMap<String, ValueSymbol>,
) -> TypeId {
    match view.node_kind(expr) {
        SyntaxKind::LiteralExpr => self.literal_type(view, expr),
        SyntaxKind::NameExpr => self.name_type(view, expr, scope_id, scope),
        SyntaxKind::ParenExpr => first_expr_child(view, expr)
            .map(|child| self.check_expr(view, child, scope_id, scope))
            .unwrap_or_else(|| self.unknown(UnknownReason::UnsupportedExpression)),
        SyntaxKind::BinaryExpr => self.binary_type(view, expr, scope_id, scope),
        SyntaxKind::CallExpr => {
            self.unsupported(view, expr, "call expressions are not checked yet");
            self.unknown(UnknownReason::UnsupportedExpression)
        }
        SyntaxKind::FieldExpr | SyntaxKind::IndexExpr | SyntaxKind::TryExpr | SyntaxKind::ReduceExpr | SyntaxKind::ScanExpr => {
            self.unsupported(view, expr, "expression form is not checked yet");
            self.unknown(UnknownReason::UnsupportedExpression)
        }
        _ => self.unknown(UnknownReason::UnsupportedExpression),
    }
}

fn literal_type(&self, view: &CstView<'_>, expr: SyntaxNodeId) -> TypeId {
    let text = view.first_identifier_text(expr).map(|token| token.text().to_string());
    match text.as_deref() {
        Some("true") | Some("false") => self.type_table.builtin(BuiltinType::Bool),
        Some("None") => self.type_table.builtin(BuiltinType::None),
        _ => {
            let span_text = view
                .child_tokens(expr)
                .first()
                .map(|token| view.token_text(*token).text().to_string())
                .unwrap_or_default();
            if span_text.starts_with('"') {
                self.type_table.builtin(BuiltinType::String)
            } else {
                self.type_table.builtin(BuiltinType::I64)
            }
        }
    }
}

fn name_type(
    &mut self,
    view: &CstView<'_>,
    expr: SyntaxNodeId,
    scope_id: ScopeId,
    scope: &mut BTreeMap<String, ValueSymbol>,
) -> TypeId {
    let Some(token) = view.first_identifier_text(expr) else {
        return self.unknown(UnknownReason::UnresolvedExpressionName);
    };
    match token.text() {
        "true" | "false" => return self.type_table.builtin(BuiltinType::Bool),
        "None" => return self.type_table.builtin(BuiltinType::None),
        _ => {}
    }
    if let Some(symbol) = scope.get(token.text()) {
        return symbol.ty;
    }
    self.record_unknown_name(scope_id, token.text(), token.span());
    self.unknown(UnknownReason::UnresolvedExpressionName)
}
```

For binary expressions:

```rust
fn binary_type(
    &mut self,
    view: &CstView<'_>,
    expr: SyntaxNodeId,
    scope_id: ScopeId,
    scope: &mut BTreeMap<String, ValueSymbol>,
) -> TypeId {
    let children = view.child_nodes(expr)
        .into_iter()
        .filter(|child| is_expr_kind(view.node_kind(*child)))
        .collect::<Vec<_>>();
    if children.len() != 2 {
        return self.unknown(UnknownReason::UnsupportedExpression);
    }
    let left = self.check_expr(view, children[0], scope_id, scope);
    let right = self.check_expr(view, children[1], scope_id, scope);
    if self.type_table.is_unknown(left) || self.type_table.is_unknown(right) {
        return self.unknown(UnknownReason::UnsupportedExpression);
    }
    if self.type_table.is_integer(left) && self.type_table.is_integer(right) {
        return left;
    }
    self.diagnostics.push(
        Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::TypeMismatch,
            "binary operands have incompatible types",
        )
        .primary(view.node_span(expr), "operands must be compatible")
        .finish(),
    );
    self.unknown(UnknownReason::UnsupportedExpression)
}
```

Add helper:

```rust
fn is_expr_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
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
}

fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .find(|child| is_expr_kind(view.node_kind(*child)))
}
```

Add the helper methods referenced above:

```rust
impl BodyChecker<'_> {
    fn unknown(&mut self, reason: UnknownReason) -> TypeId {
        self.type_table.push_unknown(reason)
    }

    fn unsupported(&mut self, view: &CstView<'_>, node: SyntaxNodeId, message: &'static str) {
        self.diagnostics.push(
            Diagnostic::builder(Severity::Error, DiagnosticCode::CheckUnsupported, message)
                .primary(view.node_span(node), message)
                .finish(),
        );
    }

    fn record_unknown_name(&mut self, _scope_id: ScopeId, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveUnknownName,
                format!("unknown name `{name}`"),
            )
            .primary(span, "name is not in scope")
            .finish(),
        );
    }

    fn type_name(&self, ty: TypeId) -> &'static str {
        match self.type_table.kind(ty) {
            Some(super::types::TypeKind::Builtin(BuiltinType::Bool)) => "Bool",
            Some(super::types::TypeKind::Builtin(BuiltinType::I64)) => "I64",
            Some(super::types::TypeKind::Builtin(BuiltinType::U32)) => "U32",
            Some(super::types::TypeKind::Builtin(BuiltinType::U64)) => "U64",
            Some(super::types::TypeKind::Builtin(BuiltinType::String)) => "String",
            Some(super::types::TypeKind::Builtin(BuiltinType::None)) => "None",
            Some(super::types::TypeKind::Item(_, _)) => "item",
            Some(super::types::TypeKind::Unknown(_)) | None => "unknown",
        }
    }

    fn builtin_type_ref(&self, view: &CstView<'_>, type_ref: SyntaxNodeId) -> Option<TypeId> {
        let name = view.first_identifier_text(type_ref)?.text();
        match name {
            "Bool" => Some(self.type_table.builtin(BuiltinType::Bool)),
            "I64" => Some(self.type_table.builtin(BuiltinType::I64)),
            "U32" => Some(self.type_table.builtin(BuiltinType::U32)),
            "U64" => Some(self.type_table.builtin(BuiltinType::U64)),
            "String" => Some(self.type_table.builtin(BuiltinType::String)),
            "None" => Some(self.type_table.builtin(BuiltinType::None)),
            _ => None,
        }
    }

    fn should_compare_types(&self, left: TypeId, right: TypeId) -> bool {
        !self.type_table.is_unknown(left) && !self.type_table.is_unknown(right)
    }
}
```

- [ ] **Step 6: Wire parsed modules into context**

Update `src/check/mod.rs` to build reusable parsed module inputs by matching `summary.file_id()` to parsed, lexed, and source:

```rust
pub struct ModuleInput<'a> {
    pub summary: &'a CheckModuleSummary,
    pub parsed: &'a ParsedSyntax,
    pub lexed: &'a LexedFile,
    pub source: &'a SourceFile,
}

fn module_inputs<'a>(
    summaries: &'a [CheckModuleSummary],
    parsed: &'a [ParsedSyntax],
    lexed_files: &'a [LexedFile],
    source_map: &'a SourceMap,
) -> Vec<ModuleInput<'a>> {
    let parsed_by_file = parsed
        .iter()
        .map(|parsed| (parsed.file_id(), parsed))
        .collect::<BTreeMap<_, _>>();
    let lexed_by_file = lexed_files
        .iter()
        .map(|lexed| (lexed.file_id(), lexed))
        .collect::<BTreeMap<_, _>>();
    let mut modules = Vec::new();

    for summary in summaries {
        let Some(parsed) = parsed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(lexed) = lexed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(source) = source_map.get(summary.file_id()) else {
            continue;
        };
        modules.push(ModuleInput {
            summary,
            parsed,
            lexed,
            source,
        });
    }

    modules
}
```

Add the imports needed by the helper:

```rust
use std::collections::BTreeMap;
use crate::lexer::LexedFile;
use crate::source::{SourceFile, SourceMap};
```

Store `body_check: BodyCheckResult` in `CheckResult` and append `body_check.diagnostics()`:

```rust
pub mod body;

use crate::check::body::{BodyCheckContext, BodyCheckResult, check_bodies};

impl CheckResult {
    pub fn body_check(&self) -> &BodyCheckResult {
        &self.body_check
    }
}
```

After `signature_check` is created:

```rust
let body_check = check_bodies(BodyCheckContext {
    modules: module_inputs(&summaries, &parsed, discovered.lexed_files(), &source_map),
    graph: &resolved_graph,
    signatures: &signature_check,
});
diagnostics.extend(body_check.diagnostics().iter().cloned());
```

- [ ] **Step 7: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- Body checking runs only when a member has a checked signature and body node.
- Unsupported parsed constructs emit `W-CHECK-UNSUPPORTED` and make `check` fail.
- Return mismatches suppress derivative errors when either side is unknown.
- No body checker code panics on parser recovery nodes.

---

### Task 4: Unknown Facts, Cascade Suppression, And Exact Fix Metadata

**Files:**
- Modify: `src/check/body.rs`
- Modify: `src/check/json.rs`
- Modify: `src/diagnostic.rs`
- Modify: `tests/check.rs`

**Description:** Group repeated unknown names by `(scope_id, name_text)`, attach later uses as related locations, and suppress type mismatch cascades that depend on unknown facts.

- [ ] **Step 1: Write failing cascade and JSON-fix tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_suppresses_type_cascade_after_unknown_name() {
    let dir = temp_dir("check-cascade");
    let root = write_file(
        &dir,
        "root.wrela",
        "module cascade\npub class C { fn m(read self) -> U32 { return missing + missing } }\n",
    );

    let result = wrela::check::check_root(&root);
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    let unknown_name_count = rendered.matches("W-RESOLVE-NAME").count();
    let mismatch_count = rendered.matches("W-TYPE-MISMATCH").count();

    assert_eq!(unknown_name_count, 1);
    assert_eq!(mismatch_count, 0);
    assert!(rendered.contains("related:"));
}

#[test]
fn check_default_json_includes_fix_metadata() {
    let dir = temp_dir("check-json-fix");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console } from io\npub data Port { value: Consol }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec!["wrela".to_string(), "check".to_string(), root.to_string_lossy().to_string()],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"suggestions\""));
    assert!(json.contains("\"applicability\":\"likely\""));
    assert!(json.contains("\"replacement\":\"Console\""));
    assert!(json.contains("\"sourceHash\""));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because unknown names are not grouped and JSON does not render fix arrays.

- [ ] **Step 3: Add unknown name tracker**

In `src/check/body.rs`:

```rust
#[derive(Default)]
struct UnknownNameTracker {
    entries: BTreeMap<(u32, String), UnknownNameEntry>,
    next_group: u32,
}

struct UnknownNameEntry {
    group: crate::diagnostic::DiagnosticGroupId,
    name: String,
    first_span: Span,
    related_spans: Vec<Span>,
}

impl UnknownNameTracker {
    fn record(&mut self, scope: ScopeId, name: &str, span: Span) {
        let key = (scope.0, name.to_string());
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.related_spans.push(span);
            return;
        }
        let group = crate::diagnostic::DiagnosticGroupId::new(self.next_group);
        self.next_group += 1;
        self.entries.insert(
            key,
            UnknownNameEntry {
                group,
                name: name.to_string(),
                first_span: span,
                related_spans: Vec::new(),
            },
        );
    }

    fn flush(self, diagnostics: &mut Vec<Diagnostic>) {
        for entry in self.entries.into_values() {
            let mut builder = Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveUnknownName,
                format!("unknown name `{}`", entry.name),
            )
            .primary(entry.first_span, "name is not in scope")
            .root_cause_group(entry.group);
            for span in entry.related_spans {
                builder = builder.related(span, "same unresolved name is used again here");
            }
            diagnostics.push(builder.finish());
        }
    }
}
```

Update `BodyChecker` and its initializer:

```rust
struct BodyChecker<'a> {
    signatures: &'a SignatureCheck,
    type_table: TypeTable,
    diagnostics: Vec<Diagnostic>,
    next_scope: u32,
    unknown_names: UnknownNameTracker,
}

let mut checker = BodyChecker {
    signatures: context.signatures,
    type_table: context.signatures.type_table().clone(),
    diagnostics: Vec::new(),
    next_scope: 0,
    unknown_names: UnknownNameTracker::default(),
};
```

Replace immediate unknown-name diagnostic emission with:

```rust
fn record_unknown_name(&mut self, scope_id: ScopeId, name: &str, span: Span) {
    self.unknown_names.record(scope_id, name, span);
}
```

Before returning `BodyCheckResult`, call:

```rust
checker.unknown_names.flush(&mut checker.diagnostics);
```

- [ ] **Step 4: Suppress type cascades**

Keep the `should_compare_types` helper from Task 3 and use it everywhere a type mismatch would be emitted:

```rust
fn should_compare_types(&self, left: TypeId, right: TypeId) -> bool {
    !self.type_table.is_unknown(left) && !self.type_table.is_unknown(right)
}
```

For return checking, binary checking, and let annotation checking, only emit mismatch diagnostics when `should_compare_types` returns true.

- [ ] **Step 5: Render JSON arrays completely**

Expand `src/check/json.rs` so diagnostics render all structured arrays:

```rust
fn render_string_array(out: &mut String, values: &[String]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        json_string(out, value);
    }
    out.push(']');
}
```

Add edit validation to `src/diagnostic.rs`:

```rust
pub fn source_edits_are_sorted_and_disjoint(edits: &[SourceEdit]) -> bool {
    let mut previous: Option<Span> = None;
    for edit in edits {
        if let Some(prev) = previous {
            if prev.file_id() > edit.span().file_id()
                || (prev.file_id() == edit.span().file_id() && prev.end() > edit.span().start())
            {
                return false;
            }
        }
        previous = Some(edit.span());
    }
    true
}
```

Every phase that creates a multi-edit suggestion must call this helper before attaching the suggestion. If it returns false, emit `W-CHECK-IO` at the diagnostic's primary span with message `suggested fix edits overlap` and do not attach that suggestion. Single-edit suggestions, including the unknown-type rename suggestion in this plan, trivially pass.

Render suggestions:

```rust
fn render_suggestions(out: &mut String, diagnostic: &Diagnostic, source_map: &SourceMap) {
    out.push('[');
    for (index, suggestion) in diagnostic.suggestions().iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        out.push('{');
        field_str(out, "title", suggestion.title());
        comma(out);
        field_str(out, "applicability", applicability_text(suggestion.applicability()));
        comma(out);
        out.push_str("\"edits\":");
        render_edits(out, suggestion.edits(), source_map);
        out.push('}');
    }
    out.push(']');
}

fn render_edits(out: &mut String, edits: &[SourceEdit], source_map: &SourceMap) {
    out.push('[');
    for (index, edit) in edits.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        out.push('{');
        out.push_str("\"span\":");
        render_span(out, edit.span(), source_map);
        comma(out);
        field_str(out, "replacement", edit.replacement());
        out.push('}');
    }
    out.push(']');
}

fn applicability_text(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::Certain => "certain",
        Applicability::Likely => "likely",
        Applicability::Maybe => "maybe",
    }
}
```

Ensure edit arrays are sorted and disjoint before rendering. If a diagnostic contains overlapping edits, render the suggestion with an empty `edits` array and add an internal `W-CHECK-IO` diagnostic in the result during the phase that creates the suggestion.

- [ ] **Step 6: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- Repeated unknown references in the same scope produce one `W-RESOLVE-NAME` diagnostic.
- Repeated uses are related locations, not duplicate root diagnostics.
- Type mismatch diagnostics are suppressed when either type is unknown.
- JSON includes suggested fix title, applicability, edits, replacement, span, and source hash.
- Human rendering prints `suggestion (likely): replace with \`Console\`` exactly.

---

### Task 5: Verification And Handoff

**Files:**
- No production file ownership.

- [ ] **Step 1: Run focused check test suite**

```bash
cargo test --test check
```

Expected result: all check integration tests pass.

- [ ] **Step 2: Run quality gate**

```bash
./scripts/quality-gate.sh
```

Expected result: all checks pass.

- [ ] **Step 3: Commit**

```bash
git add src/check src/diagnostic.rs tests/check.rs
git commit -m "feat: add check typechecking and recovery -Codex Automated"
```

**Acceptance Criteria:**
- Signature and body type checking are wired into `check_root`.
- Unknown facts allow permissive recovery without cascades.
- Suggested fixes are present in JSON and human output.
- Quality gate passes.
- Phase A is not required here when this child plan is executed as part of the parent check delivery; run Phase A here only if this child plan is handed to the user independently.
