# Check 02: Semantic Summaries And Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Derive owned semantic summaries from the CST and validate modules, imports, exports, duplicates, visibility, and name kinds.

**Architecture:** `check/cst.rs` provides small CST traversal helpers. `check/summary.rs` owns HIR-ish summary data copied out of the CST. `check/resolve.rs` owns deterministic symbol tables and import validation. `check/suggest.rs` owns small edit-distance utilities used by diagnostics. `check/mod.rs` wires summaries and resolution into `CheckResult`.

**Tech Stack:** Rust 2024, standard library only, existing CST/lexer/parser data structures.

---

## File Ownership

```text
src/check/
  cst.rs       CST traversal helpers for checker phases
  summary.rs   owned module/import/item/member/signature summaries
  resolve.rs   resolved module graph and symbol validation
  suggest.rs   edit-distance and nearest-name utilities
  mod.rs       CheckResult summary/resolution fields and pipeline wiring
  json.rs      JSON fields for summaries/resolution diagnostics
tests/
  check.rs     black-box check behavior
```

## Existing Name Collision Decision

The repository already has `syntax::lower::ModuleSummary` for the `wrela parse` debug summary and `syntax::imports::ImportSummary` for discovery's lightweight import scanner. Keep both existing types. Do not delete or rename them, because `command.rs` uses `summarize_module` for `wrela parse` and discovery uses `parse_import_summary`.

The checker-owned semantic types are named `CheckModuleSummary` and `CheckImportSummary`. Import them from `crate::check::summary`, never from `crate::syntax`.

## Real Parallel Work

- Task 1 owns `src/check/cst.rs` and runs first.
- After Task 1, Task 2 (`summary.rs`) and Task 4 (`suggest.rs`) may run in parallel for their owned files.
- Task 3 depends on Tasks 1 and 2.
- Task 5 depends on Tasks 2, 3, and 4.
- Task 6 depends on Task 5.

Parallel subagents should not both edit `src/check/mod.rs`. Let the integration owner add the `pub mod summary;`, `pub mod suggest;`, and `pub mod resolve;` lines when merging branches.

Subagents may run the focused tests listed in their task with a timeout set in the execution tool. Recommended focused-command timeout: 120 seconds. The orchestrator runs `./scripts/quality-gate.sh` after merging each task.

---

### Task 1: CST Traversal Helpers

**Files:**
- Create: `src/check/cst.rs`
- Modify: `src/check/mod.rs`

**Description:** Add small, allocation-friendly CST helpers so summary and checking code does not duplicate tree walking.

- [ ] **Step 1: Write failing helper tests**

Add this unit test to `src/check/cst.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use crate::syntax::{SyntaxKind, parse_file};
    use std::path::PathBuf;

    #[test]
    fn finds_child_nodes_and_identifier_text() {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            "module root\npub data Bytes { value: U32 }\n".to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        let view = CstView::new(parsed.tree(), &lexed, &source);

        let module = parsed.tree().root();
        let data = view
            .child_nodes(module)
            .into_iter()
            .find(|node| view.node_kind(*node) == SyntaxKind::PublicItem)
            .and_then(|public| view.first_descendant(public, SyntaxKind::DataDecl))
            .unwrap();

        assert_eq!(view.first_identifier_text(data).unwrap().text(), "Bytes");
        assert!(view.child_nodes(data).iter().any(|node| {
            view.node_kind(*node) == SyntaxKind::GenericParamList
                || view.node_kind(*node) == SyntaxKind::FieldDecl
        }));
    }
}
```

- [ ] **Step 2: Run test and verify failure**

```bash
cargo test check::cst::tests::finds_child_nodes_and_identifier_text
```

Expected result: compilation fails because `CstView` does not exist.

- [ ] **Step 3: Implement `CstView`**

Create `src/check/cst.rs`:

```rust
use crate::lexer::{LexedFile, TokenKind};
use crate::source::{SourceFile, Span};
use crate::syntax::{SyntaxElement, SyntaxKind, SyntaxNodeId, SyntaxTokenId, SyntaxTree};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenText<'a> {
    text: &'a str,
    span: Span,
}

impl<'a> TokenText<'a> {
    pub const fn new(text: &'a str, span: Span) -> Self {
        Self { text, span }
    }

    pub const fn text(self) -> &'a str {
        self.text
    }

    pub const fn span(self) -> Span {
        self.span
    }
}

pub struct CstView<'a> {
    tree: &'a SyntaxTree,
    lexed: &'a LexedFile,
    source: &'a SourceFile,
}

impl<'a> CstView<'a> {
    pub const fn new(tree: &'a SyntaxTree, lexed: &'a LexedFile, source: &'a SourceFile) -> Self {
        Self { tree, lexed, source }
    }

    pub fn node_kind(&self, node: SyntaxNodeId) -> SyntaxKind {
        self.tree.node(node).kind()
    }

    pub fn node_span(&self, node: SyntaxNodeId) -> Span {
        self.tree.node(node).span()
    }

    pub fn child_nodes(&self, node: SyntaxNodeId) -> Vec<SyntaxNodeId> {
        self.tree
            .elements(self.tree.node(node).children())
            .iter()
            .filter_map(|element| match *element {
                SyntaxElement::Node(child) => Some(child),
                _ => None,
            })
            .collect()
    }

    pub fn child_tokens(&self, node: SyntaxNodeId) -> Vec<SyntaxTokenId> {
        self.tree
            .elements(self.tree.node(node).children())
            .iter()
            .filter_map(|element| match *element {
                SyntaxElement::Token(token) => Some(token),
                _ => None,
            })
            .collect()
    }

    pub fn first_child(&self, node: SyntaxNodeId, kind: SyntaxKind) -> Option<SyntaxNodeId> {
        self.child_nodes(node)
            .into_iter()
            .find(|child| self.node_kind(*child) == kind)
    }

    pub fn first_descendant(&self, node: SyntaxNodeId, kind: SyntaxKind) -> Option<SyntaxNodeId> {
        for child in self.child_nodes(node) {
            if self.node_kind(child) == kind {
                return Some(child);
            }
            if let Some(found) = self.first_descendant(child, kind) {
                return Some(found);
            }
        }
        None
    }

    pub fn token_text(&self, token: SyntaxTokenId) -> TokenText<'a> {
        let syntax_token = self.tree.token(token);
        let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
        let span = raw.span();
        TokenText::new(&self.source.text()[span.start() as usize..span.end() as usize], span)
    }

    pub fn first_identifier_text(&self, node: SyntaxNodeId) -> Option<TokenText<'a>> {
        for token in self.child_tokens(node) {
            let syntax_token = self.tree.token(token);
            let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
            if raw.kind() == TokenKind::Identifier {
                return Some(self.token_text(token));
            }
        }
        for child in self.child_nodes(node) {
            if let Some(found) = self.first_identifier_text(child) {
                return Some(found);
            }
        }
        None
    }

    pub fn identifiers_in_node(&self, node: SyntaxNodeId) -> Vec<TokenText<'a>> {
        let mut identifiers = Vec::new();
        self.push_identifiers(node, &mut identifiers);
        identifiers
    }

    fn push_identifiers(&self, node: SyntaxNodeId, out: &mut Vec<TokenText<'a>>) {
        for token in self.child_tokens(node) {
            let syntax_token = self.tree.token(token);
            let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
            if raw.kind() == TokenKind::Identifier {
                out.push(self.token_text(token));
            }
        }
        for child in self.child_nodes(node) {
            self.push_identifiers(child, out);
        }
    }
}
```

Add to `src/check/mod.rs`:

```rust
pub mod cst;
```

- [ ] **Step 4: Run focused test**

```bash
cargo test check::cst::tests::finds_child_nodes_and_identifier_text
```

Expected result: test passes.

**Acceptance Criteria:**
- CST helpers do not mutate parser data.
- Helper return values are deterministic in source order.
- No helper panics on a well-formed `SyntaxNodeId` from the same tree.

---

### Task 2: Summary Data Model

**Files:**
- Create: `src/check/summary.rs`
- Modify: `src/check/mod.rs`

**Description:** Define the owned HIR-ish summary model. This task makes every struct field and getter explicit, including `ResolvedModule` dependencies for later tasks.

- [ ] **Step 1: Write failing summary model tests**

Add to `src/check/summary.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};
    use crate::syntax::SyntaxNodeId;

    #[test]
    fn module_summary_exposes_imports_items_and_structural_validity() {
        let name = Name::new("Console", Span::new(FileId::new(0), 4, 11));
        let item = ItemSummary::new(
            SyntaxNodeId::new(9),
            name.clone(),
            ItemKind::UniqueClass,
            true,
            Span::new(FileId::new(0), 0, 20),
        );
        let module = CheckModuleSummary::new(
            FileId::new(0),
            ModulePathSummary::from_names(vec![Name::new("root", Span::new(FileId::new(0), 7, 11))]),
            Vec::new(),
            vec![item],
            true,
        );

        assert!(module.structurally_valid());
        assert_eq!(module.module_path().as_dotted(), "root");
        assert_eq!(module.items()[0].name().text(), "Console");
        assert_eq!(module.items()[0].kind(), ItemKind::UniqueClass);
        assert!(module.items()[0].is_public());
    }
}
```

- [ ] **Step 2: Run test and verify failure**

```bash
cargo test check::summary::tests::module_summary_exposes_imports_items_and_structural_validity
```

Expected result: compilation fails because `summary.rs` does not exist.

- [ ] **Step 3: Implement exact summary structs**

Create `src/check/summary.rs`:

```rust
use crate::source::{FileId, Span};
use crate::syntax::SyntaxNodeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    text: String,
    span: Span,
}

impl Name {
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePathSummary {
    segments: Vec<Name>,
    span: Span,
}

impl ModulePathSummary {
    pub fn from_names(segments: Vec<Name>) -> Self {
        let span = span_covering_names(&segments);
        Self { segments, span }
    }

    pub fn segments(&self) -> &[Name] {
        &self.segments
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn as_dotted(&self) -> String {
        self.segments
            .iter()
            .map(Name::text)
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSummary {
    node_id: SyntaxNodeId,
    access: Option<AccessMode>,
    unique: bool,
    path: Vec<Name>,
    args: Vec<TypeRefSummary>,
    span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamSummary {
    node_id: SyntaxNodeId,
    name: Name,
    access: AccessMode,
    ty: TypeRefSummary,
    span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureSummary {
    node_id: SyntaxNodeId,
    params: Vec<ParamSummary>,
    return_type: Option<TypeRefSummary>,
    span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    Field,
    Method,
    Constructor,
    Test,
    Phase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberSummary {
    node_id: SyntaxNodeId,
    name: Option<Name>,
    kind: MemberKind,
    signature: Option<SignatureSummary>,
    field_type: Option<TypeRefSummary>,
    body: Option<SyntaxNodeId>,
    span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Data,
    LayoutData,
    Class,
    UniqueClass,
    Interface,
    Error,
    Image,
    HostImage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemSummary {
    node_id: SyntaxNodeId,
    name: Name,
    kind: ItemKind,
    public: bool,
    generic_params: Vec<Name>,
    implements: Vec<TypeRefSummary>,
    members: Vec<MemberSummary>,
    span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckImportSummary {
    node_id: SyntaxNodeId,
    module_path: ModulePathSummary,
    binders: Vec<Name>,
    span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckModuleSummary {
    file_id: FileId,
    module_path: ModulePathSummary,
    imports: Vec<CheckImportSummary>,
    items: Vec<ItemSummary>,
    structurally_valid: bool,
}
```

Add constructors and getters:

```rust
impl TypeRefSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        access: Option<AccessMode>,
        unique: bool,
        path: Vec<Name>,
        args: Vec<TypeRefSummary>,
        span: Span,
    ) -> Self {
        Self { node_id, access, unique, path, args, span }
    }

    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn access(&self) -> Option<AccessMode> { self.access }
    pub fn unique(&self) -> bool { self.unique }
    pub fn path(&self) -> &[Name] { &self.path }
    pub fn args(&self) -> &[TypeRefSummary] { &self.args }
    pub fn span(&self) -> Span { self.span }
    pub fn path_text(&self) -> String {
        self.path.iter().map(Name::text).collect::<Vec<_>>().join(".")
    }
}

impl ParamSummary {
    pub fn new(node_id: SyntaxNodeId, name: Name, access: AccessMode, ty: TypeRefSummary, span: Span) -> Self {
        Self { node_id, name, access, ty, span }
    }
    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn name(&self) -> &Name { &self.name }
    pub fn access(&self) -> AccessMode { self.access }
    pub fn ty(&self) -> &TypeRefSummary { &self.ty }
    pub fn span(&self) -> Span { self.span }
}

impl SignatureSummary {
    pub fn new(node_id: SyntaxNodeId, params: Vec<ParamSummary>, return_type: Option<TypeRefSummary>, span: Span) -> Self {
        Self { node_id, params, return_type, span }
    }
    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn params(&self) -> &[ParamSummary] { &self.params }
    pub fn return_type(&self) -> Option<&TypeRefSummary> { self.return_type.as_ref() }
    pub fn span(&self) -> Span { self.span }
}

impl MemberSummary {
    pub fn new_field(node_id: SyntaxNodeId, name: Name, field_type: TypeRefSummary, span: Span) -> Self {
        Self { node_id, name: Some(name), kind: MemberKind::Field, signature: None, field_type: Some(field_type), body: None, span }
    }
    pub fn new_callable(node_id: SyntaxNodeId, name: Option<Name>, kind: MemberKind, signature: SignatureSummary, body: Option<SyntaxNodeId>, span: Span) -> Self {
        Self { node_id, name, kind, signature: Some(signature), field_type: None, body, span }
    }
    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn name(&self) -> Option<&Name> { self.name.as_ref() }
    pub fn kind(&self) -> MemberKind { self.kind }
    pub fn signature(&self) -> Option<&SignatureSummary> { self.signature.as_ref() }
    pub fn field_type(&self) -> Option<&TypeRefSummary> { self.field_type.as_ref() }
    pub fn body(&self) -> Option<SyntaxNodeId> { self.body }
    pub fn span(&self) -> Span { self.span }
}

impl ItemSummary {
    pub fn new(node_id: SyntaxNodeId, name: Name, kind: ItemKind, public: bool, span: Span) -> Self {
        Self { node_id, name, kind, public, generic_params: Vec::new(), implements: Vec::new(), members: Vec::new(), span }
    }
    pub fn with_members(mut self, members: Vec<MemberSummary>) -> Self {
        self.members = members;
        self
    }
    pub fn with_generic_params(mut self, generic_params: Vec<Name>) -> Self {
        self.generic_params = generic_params;
        self
    }
    pub fn with_implements(mut self, implements: Vec<TypeRefSummary>) -> Self {
        self.implements = implements;
        self
    }
    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn name(&self) -> &Name { &self.name }
    pub fn kind(&self) -> ItemKind { self.kind }
    pub fn is_public(&self) -> bool { self.public }
    pub fn generic_params(&self) -> &[Name] { &self.generic_params }
    pub fn implements(&self) -> &[TypeRefSummary] { &self.implements }
    pub fn members(&self) -> &[MemberSummary] { &self.members }
    pub fn fields(&self) -> impl Iterator<Item = &MemberSummary> {
        self.members.iter().filter(|member| member.kind() == MemberKind::Field)
    }
    pub fn span(&self) -> Span { self.span }
}

impl CheckImportSummary {
    pub fn new(node_id: SyntaxNodeId, module_path: ModulePathSummary, binders: Vec<Name>, span: Span) -> Self {
        Self { node_id, module_path, binders, span }
    }
    pub fn node_id(&self) -> SyntaxNodeId { self.node_id }
    pub fn module_path(&self) -> &ModulePathSummary { &self.module_path }
    pub fn binders(&self) -> &[Name] { &self.binders }
    pub fn span(&self) -> Span { self.span }
}

impl CheckModuleSummary {
    pub fn new(
        file_id: FileId,
        module_path: ModulePathSummary,
        imports: Vec<CheckImportSummary>,
        items: Vec<ItemSummary>,
        structurally_valid: bool,
    ) -> Self {
        Self { file_id, module_path, imports, items, structurally_valid }
    }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn module_path(&self) -> &ModulePathSummary { &self.module_path }
    pub fn imports(&self) -> &[CheckImportSummary] { &self.imports }
    pub fn items(&self) -> &[ItemSummary] { &self.items }
    pub fn structurally_valid(&self) -> bool { self.structurally_valid }
}
```

Add the span helper:

```rust
fn span_covering_names(names: &[Name]) -> Span {
    let first = names.first().expect("module paths require at least one segment");
    let last = names.last().expect("module paths require at least one segment");
    Span::new(first.span().file_id(), first.span().start(), last.span().end())
}
```

Add to `src/check/mod.rs`:

```rust
pub mod summary;
```

- [ ] **Step 4: Run focused test**

```bash
cargo test check::summary::tests::module_summary_exposes_imports_items_and_structural_validity
```

Expected result: test passes.

**Acceptance Criteria:**
- Summary structs own all names as `String`.
- Every summary item carries source spans and original `SyntaxNodeId`.
- `MemberSummary::signature()` is explicitly optional because fields do not have signatures.
- `CheckModuleSummary::structurally_valid()` is defined and tested.
- `TypeRefSummary` preserves `read`/`mut`/`own` access qualifiers and the `unique` type qualifier separately.

---

### Task 3: Summary Extraction

**Files:**
- Modify: `src/check/summary.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Extract module declarations, imports, items, members, signatures, fields, and body node references from parsed files.

- [ ] **Step 1: Write failing summary extraction test**

Add to `tests/check.rs`:

```rust
#[test]
fn check_collects_semantic_summaries() {
    let dir = temp_dir("check-summary");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module app.root\nuse { Console } from io\npub unique class Driver { uart: Console fn write(read self, read bytes: Bytes) -> U32 { return 0 } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok());
    let summaries = result.summaries();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].module_path().as_dotted(), "app.root");
    assert_eq!(summaries[0].imports()[0].binders()[0].text(), "Console");
    assert_eq!(summaries[0].items()[0].name().text(), "Driver");
    assert_eq!(summaries[0].items()[0].members().len(), 2);
}
```

Add this CST-shape smoke test to `src/check/summary.rs` before writing extraction code:

```rust
#[cfg(test)]
mod extraction_tests {
    use super::*;
    use crate::check::cst::CstView;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use crate::syntax::{SyntaxKind, parse_file};
    use std::path::PathBuf;

    #[test]
    fn parser_shape_matches_summary_extractor_expectations() {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            "module root\nuse { Console } from io\npub data Bytes { value: U32 }\n".to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        let view = CstView::new(parsed.tree(), &lexed, &source);

        let root = parsed.tree().root();
        let module_decl = view.first_child(root, SyntaxKind::ModuleDecl).unwrap();
        let use_decl = view.first_child(root, SyntaxKind::UseDecl).unwrap();
        let public_item = view.first_child(root, SyntaxKind::PublicItem).unwrap();
        let data_decl = view.first_child(public_item, SyntaxKind::DataDecl).unwrap();

        assert!(view.first_child(module_decl, SyntaxKind::ModulePath).is_some());
        assert!(view.first_child(use_decl, SyntaxKind::UseBinderList).is_some());
        assert!(view.first_child(use_decl, SyntaxKind::ModulePath).is_some());
        assert!(view.first_child(data_decl, SyntaxKind::FieldDecl).is_some());
    }
}
```

- [ ] **Step 2: Run test and verify failure**

```bash
cargo test --test check check_collects_semantic_summaries
cargo test check::summary::extraction_tests::parser_shape_matches_summary_extractor_expectations
```

Expected result: compilation fails because `CheckResult::summaries` and summary extraction do not exist.

- [ ] **Step 3: Add extraction entry point**

Add to `src/check/summary.rs`:

```rust
use std::path::Path;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::LexedFile;
use crate::source::SourceFile;
use crate::syntax::{ParsedSyntax, SyntaxKind, SyntaxNodeId};

use super::cst::{CstView, TokenText};

pub fn summarize_checked_module(
    parsed: &ParsedSyntax,
    lexed: &LexedFile,
    source: &SourceFile,
    source_root: &Path,
) -> (CheckModuleSummary, Vec<Diagnostic>) {
    let view = CstView::new(parsed.tree(), lexed, source);
    let root = parsed.tree().root();
    let mut diagnostics = Vec::new();

    let module_path = find_module_decl(&view, root)
        .and_then(|node| module_path_from_node(&view, node))
        .unwrap_or_else(|| module_path_from_file(source, source_root));

    let mut imports = Vec::new();
    let mut items = Vec::new();
    let mut structurally_valid = true;

    for child in view.child_nodes(root) {
        match view.node_kind(child) {
            SyntaxKind::UseDecl => {
                if let Some(import) = summarize_import(&view, child) {
                    imports.push(import);
                } else {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(child, &view, "could not summarize import"));
                }
            }
            SyntaxKind::PublicItem => match summarize_public_item(&view, child) {
                Some(item) => items.push(item),
                None => {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(child, &view, "could not summarize public item"));
                }
            },
            SyntaxKind::DataDecl
            | SyntaxKind::LayoutDataDecl
            | SyntaxKind::ClassDecl
            | SyntaxKind::UniqueClassDecl
            | SyntaxKind::InterfaceDecl
            | SyntaxKind::ErrorDecl
            | SyntaxKind::ImageDecl
            | SyntaxKind::HostImageDecl => match summarize_item(&view, child, false) {
                Some(item) => items.push(item),
                None => {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(child, &view, "could not summarize item"));
                }
            },
            SyntaxKind::RecoveryNode => {
                structurally_valid = false;
                diagnostics.push(invalid_summary(child, &view, "parser recovery reached top level"));
            }
            _ => {}
        }
    }

    if contains_recovery(&view, root) {
        structurally_valid = false;
    }

    (
        CheckModuleSummary::new(parsed.file_id(), module_path, imports, items, structurally_valid),
        diagnostics,
    )
}
```

- [ ] **Step 4: Add exact extraction helpers**

Add these helpers in `summary.rs`:

```rust
fn find_module_decl(view: &CstView<'_>, root: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(root)
        .into_iter()
        .find(|node| view.node_kind(*node) == SyntaxKind::ModuleDecl)
}

fn module_path_from_node(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ModulePathSummary> {
    let path_node = view.first_child(node, SyntaxKind::ModulePath)?;
    module_path_from_path_node(view, path_node)
}

fn module_path_from_path_node(view: &CstView<'_>, path_node: SyntaxNodeId) -> Option<ModulePathSummary> {
    let segments = view
        .identifiers_in_node(path_node)
        .into_iter()
        .map(name_from_token)
        .collect::<Vec<_>>();
    if segments.is_empty() {
        None
    } else {
        Some(ModulePathSummary::from_names(segments))
    }
}

fn module_path_from_file(source: &SourceFile, source_root: &Path) -> ModulePathSummary {
    let relative = source.path().strip_prefix(source_root).unwrap_or(source.path());
    let mut names = Vec::new();
    for component in relative.with_extension("").components() {
        let text = component.as_os_str().to_string_lossy();
        if !text.is_empty() {
            names.push(Name::new(text, source.span()));
        }
    }
    if names.is_empty() {
        names.push(Name::new("root", source.span()));
    }
    ModulePathSummary::from_names(names)
}

fn summarize_public_item(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ItemSummary> {
    view.child_nodes(node)
        .into_iter()
        .find_map(|child| summarize_item(view, child, true))
}

fn summarize_item(view: &CstView<'_>, node: SyntaxNodeId, public: bool) -> Option<ItemSummary> {
    let kind = item_kind(view.node_kind(node))?;
    let name = item_name(view, node, kind)?;
    let mut item = ItemSummary::new(node, name, kind, public, view.node_span(node));
    let members = summarize_members(view, node, kind);
    item = item.with_members(members);
    let implements = view
        .first_child(node, SyntaxKind::ImplementsClause)
        .map(|implements| {
            view.child_nodes(implements)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::TypeRef)
                .filter_map(|child| summarize_type_ref(view, child))
                .collect()
        })
        .unwrap_or_default();
    Some(item.with_implements(implements))
}

fn item_kind(kind: SyntaxKind) -> Option<ItemKind> {
    match kind {
        SyntaxKind::DataDecl => Some(ItemKind::Data),
        SyntaxKind::LayoutDataDecl => Some(ItemKind::LayoutData),
        SyntaxKind::ClassDecl => Some(ItemKind::Class),
        SyntaxKind::UniqueClassDecl => Some(ItemKind::UniqueClass),
        SyntaxKind::InterfaceDecl => Some(ItemKind::Interface),
        SyntaxKind::ErrorDecl => Some(ItemKind::Error),
        SyntaxKind::ImageDecl => Some(ItemKind::Image),
        SyntaxKind::HostImageDecl => Some(ItemKind::HostImage),
        _ => None,
    }
}

fn item_name(view: &CstView<'_>, node: SyntaxNodeId, kind: ItemKind) -> Option<Name> {
    let identifiers = view.identifiers_in_node(node);
    let index = if kind == ItemKind::LayoutData { 1 } else { 0 };
    identifiers.get(index).copied().map(name_from_token)
}

fn summarize_members(view: &CstView<'_>, item: SyntaxNodeId, _kind: ItemKind) -> Vec<MemberSummary> {
    view.child_nodes(item)
        .into_iter()
        .filter_map(|child| match view.node_kind(child) {
            SyntaxKind::FieldDecl => summarize_field(view, child),
            SyntaxKind::MethodDecl => summarize_callable(view, child, MemberKind::Method),
            SyntaxKind::ConstructorDecl => summarize_callable(view, child, MemberKind::Constructor),
            SyntaxKind::TestDecl => summarize_callable(view, child, MemberKind::Test),
            SyntaxKind::PhaseDecl => summarize_callable(view, child, MemberKind::Phase),
            _ => None,
        })
        .collect()
}

fn summarize_field(view: &CstView<'_>, node: SyntaxNodeId) -> Option<MemberSummary> {
    let name = view.first_identifier_text(node).map(name_from_token)?;
    let ty = view
        .first_child(node, SyntaxKind::TypeRef)
        .and_then(|type_node| summarize_type_ref(view, type_node))?;
    Some(MemberSummary::new_field(node, name, ty, view.node_span(node)))
}

fn summarize_callable(view: &CstView<'_>, node: SyntaxNodeId, kind: MemberKind) -> Option<MemberSummary> {
    let name = match kind {
        MemberKind::Constructor => None,
        _ => view.first_identifier_text(node).map(name_from_token),
    };
    let signature = summarize_signature(view, node)?;
    let body = view.first_child(node, SyntaxKind::Block);
    Some(MemberSummary::new_callable(node, name, kind, signature, body, view.node_span(node)))
}

fn summarize_signature(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SignatureSummary> {
    let param_list = view.first_child(node, SyntaxKind::ParamList);
    let params = param_list
        .map(|list| {
            view.child_nodes(list)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::Param)
                .filter_map(|param| summarize_param(view, param))
                .collect()
        })
        .unwrap_or_default();
    let return_type = view
        .first_child(node, SyntaxKind::ReturnType)
        .and_then(|return_node| view.first_child(return_node, SyntaxKind::TypeRef))
        .and_then(|type_node| summarize_type_ref(view, type_node));
    Some(SignatureSummary::new(node, params, return_type, view.node_span(node)))
}

fn summarize_param(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ParamSummary> {
    let name = view.first_identifier_text(node).map(name_from_token)?;
    let access = access_in_node(view, node).unwrap_or(AccessMode::Read);
    let ty = view
        .first_child(node, SyntaxKind::TypeRef)
        .and_then(|type_node| summarize_type_ref(view, type_node))?;
    Some(ParamSummary::new(node, name, access, ty, view.node_span(node)))
}

fn summarize_type_ref(view: &CstView<'_>, node: SyntaxNodeId) -> Option<TypeRefSummary> {
    let access = access_in_node(view, node);
    let unique = unique_in_node(view, node);
    let path = view.identifiers_in_node(node).into_iter().map(name_from_token).collect::<Vec<_>>();
    if path.is_empty() {
        return None;
    }
    let args = view
        .first_child(node, SyntaxKind::GenericArgList)
        .map(|arg_list| {
            view.child_nodes(arg_list)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::TypeRef)
                .filter_map(|child| summarize_type_ref(view, child))
                .collect()
        })
        .unwrap_or_default();
    Some(TypeRefSummary::new(node, access, unique, path, args, view.node_span(node)))
}

fn access_in_node(view: &CstView<'_>, node: SyntaxNodeId) -> Option<AccessMode> {
    for token in view.child_tokens(node) {
        let text = view.token_text(token).text();
        match text {
            "read" => return Some(AccessMode::Read),
            "mut" => return Some(AccessMode::Mut),
            "own" => return Some(AccessMode::Own),
            _ => {}
        }
    }
    None
}

fn unique_in_node(view: &CstView<'_>, node: SyntaxNodeId) -> bool {
    view.child_tokens(node)
        .into_iter()
        .any(|token| view.token_text(token).text() == "unique")
}

fn summarize_import(view: &CstView<'_>, node: SyntaxNodeId) -> Option<CheckImportSummary> {
    let module_path = view
        .first_child(node, SyntaxKind::ModulePath)
        .and_then(|path_node| module_path_from_path_node(view, path_node))?;
    let binders = view
        .first_child(node, SyntaxKind::UseBinderList)
        .map(|list| {
            view.child_nodes(list)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::UseBinder)
                .filter_map(|binder| view.first_identifier_text(binder).map(name_from_token))
                .collect()
        })
        .unwrap_or_default();
    Some(CheckImportSummary::new(node, module_path, binders, view.node_span(node)))
}

fn name_from_token(token: TokenText<'_>) -> Name {
    Name::new(token.text(), token.span())
}

fn invalid_summary(node: SyntaxNodeId, view: &CstView<'_>, message: &'static str) -> Diagnostic {
    Diagnostic::builder(Severity::Error, DiagnosticCode::SummaryInvalidModule, message)
        .primary(view.node_span(node), message)
        .finish()
}

fn contains_recovery(view: &CstView<'_>, node: SyntaxNodeId) -> bool {
    view.node_kind(node) == SyntaxKind::RecoveryNode
        || view
            .child_nodes(node)
            .into_iter()
            .any(|child| contains_recovery(view, child))
}
```

Remove unused imports after compiling. Keep all helper functions private except `summarize_checked_module`.

- [ ] **Step 5: Wire summaries into `CheckResult`**

Update `src/check/mod.rs`:

```rust
pub mod summary;

use crate::check::summary::{CheckModuleSummary, summarize_checked_module};

pub struct CheckResult {
    source_map: SourceMap,
    parsed: Vec<ParsedSyntax>,
    summaries: Vec<CheckModuleSummary>,
    diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn summaries(&self) -> &[CheckModuleSummary] {
        &self.summaries
    }
}
```

In `check_root`, after parsing:

```rust
let source_root = source_map
    .files()
    .first()
    .and_then(|file| file.path().parent())
    .unwrap_or_else(|| Path::new("."));
let lexed_by_file: std::collections::BTreeMap<_, _> = discovered
    .lexed_files()
    .iter()
    .map(|lexed| (lexed.file_id(), lexed))
    .collect();
let mut summaries = Vec::new();
for parsed_file in &parsed {
    if let (Some(lexed), Some(source)) = (
        lexed_by_file.get(&parsed_file.file_id()),
        source_map.get(parsed_file.file_id()),
    ) {
        let (summary, summary_diagnostics) =
            summarize_checked_module(parsed_file, lexed, source, source_root);
        diagnostics.extend(summary_diagnostics);
        summaries.push(summary);
    }
}
```

- [ ] **Step 6: Run focused test**

```bash
cargo test --test check check_collects_semantic_summaries
```

Expected result: test passes.

**Acceptance Criteria:**
- Summaries are produced for parsed files in source map order.
- A top-level `RecoveryNode` marks the module structurally invalid and emits `W-SUMMARY-INVALID`.
- Valid independent items are still summarized when a different item is invalid.
- `CheckResult::summaries()` is stable public API for later plans.

---

### Task 4: Name Suggestion Utility

**Files:**
- Create: `src/check/suggest.rs`
- Modify: `src/check/mod.rs`

**Description:** Add deterministic edit-distance suggestions for later resolution and type diagnostics.

- [ ] **Step 1: Write failing suggestion tests**

Create `src/check/suggest.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_counts_insert_delete_and_replace() {
        assert_eq!(edit_distance("Console", "Consol"), 1);
        assert_eq!(edit_distance("Driver", "Diver"), 1);
        assert_eq!(edit_distance("abc", "xyz"), 3);
    }

    #[test]
    fn nearest_name_uses_distance_then_lexical_order() {
        let candidates = ["Console", "Consola", "Driver"];
        assert_eq!(nearest_name("Consol", candidates.iter().copied()), Some("Console"));
        assert_eq!(nearest_name("zzzz", candidates.iter().copied()), None);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test check::suggest::tests
```

Expected result: compilation fails until the module is exported.

- [ ] **Step 3: Implement edit distance**

Use handwritten dynamic programming:

```rust
pub fn edit_distance(left: &str, right: &str) -> usize {
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_ch) in left_chars.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right_chars.iter().enumerate() {
            let replace_cost = if left_ch == right_ch { 0 } else { 1 };
            let delete = previous[right_index + 1] + 1;
            let insert = current[right_index] + 1;
            let replace = previous[right_index] + replace_cost;
            current[right_index + 1] = delete.min(insert).min(replace);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}

pub fn nearest_name<'a, I>(needle: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best: Option<(usize, &'a str)> = None;
    for candidate in candidates {
        let distance = edit_distance(needle, candidate);
        if distance > 2 {
            continue;
        }
        match best {
            Some((best_distance, best_name))
                if distance > best_distance
                    || (distance == best_distance && candidate >= best_name) => {}
            _ => best = Some((distance, candidate)),
        }
    }
    best.map(|(_, name)| name)
}
```

Add to `src/check/mod.rs`:

```rust
pub mod suggest;
```

- [ ] **Step 4: Run focused tests**

```bash
cargo test check::suggest::tests
```

Expected result: tests pass.

**Acceptance Criteria:**
- Suggestions are deterministic.
- Maximum suggestion distance is 2.
- No external crate is added.

---

### Task 5: Module Resolution

**Files:**
- Create: `src/check/resolve.rs`
- Modify: `src/check/mod.rs`
- Modify: `tests/check.rs`

**Description:** Build deterministic symbol tables, validate imports and exports, catch duplicates, and define `ResolvedModule` explicitly.

- [ ] **Step 1: Write failing resolver tests**

Add to `tests/check.rs`:

```rust
#[test]
fn check_reports_duplicate_top_level_names() {
    let dir = temp_dir("check-duplicates");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub data Bytes { value: U32 }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-RESOLVE-DUPLICATE]"));
    assert!(rendered.contains("first declaration is here"));
}

#[test]
fn check_validates_imported_public_names() {
    let dir = temp_dir("check-imports");
    write_file(&dir, "io.wrela", "module io\nclass Hidden {}\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console, Hidden, Missing } from io\npub data Bytes { value: U32 }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-RESOLVE-PRIVATE"));
    assert!(rendered.contains("W-RESOLVE-IMPORT"));
    assert!(!rendered.contains("Console is missing"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because resolver diagnostics do not exist.

- [ ] **Step 3: Add resolver data structures**

Create `src/check/resolve.rs`:

```rust
use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::source::{FileId, Span};

use super::summary::{CheckImportSummary, ItemKind, CheckModuleSummary, Name};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModuleId(u32);

impl ModuleId {
    pub const fn new(raw: u32) -> Self { Self(raw) }
    pub const fn raw(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ItemId {
    module: ModuleId,
    index: u32,
}

impl ItemId {
    pub const fn new(module: ModuleId, index: u32) -> Self { Self { module, index } }
    pub const fn module(self) -> ModuleId { self.module }
    pub const fn index(self) -> u32 { self.index }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Data,
    LayoutData,
    Interface,
    Class,
    UniqueClass,
    Error,
    Image,
    HostImage,
    BuiltinType,
}

#[derive(Clone, Debug)]
pub struct ResolvedItem {
    id: ItemId,
    name: Name,
    kind: SymbolKind,
    public: bool,
    span: Span,
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    id: ModuleId,
    file_id: FileId,
    path: String,
    items: Vec<ResolvedItem>,
    local_symbols: BTreeMap<String, ItemId>,
    imported_symbols: BTreeMap<String, ItemId>,
}

#[derive(Clone, Debug)]
pub struct ResolvedGraph {
    modules: Vec<ResolvedModule>,
    diagnostics: Vec<Diagnostic>,
}
```

Add getters:

```rust
impl ResolvedItem {
    pub fn id(&self) -> ItemId { self.id }
    pub fn name(&self) -> &Name { &self.name }
    pub fn kind(&self) -> SymbolKind { self.kind }
    pub fn is_public(&self) -> bool { self.public }
    pub fn span(&self) -> Span { self.span }
}

impl ResolvedModule {
    pub fn id(&self) -> ModuleId { self.id }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn path(&self) -> &str { &self.path }
    pub fn items(&self) -> &[ResolvedItem] { &self.items }
    pub fn resolve_local(&self, name: &str) -> Option<ItemId> { self.local_symbols.get(name).copied() }
    pub fn resolve_imported(&self, name: &str) -> Option<ItemId> { self.imported_symbols.get(name).copied() }
}

impl ResolvedGraph {
    pub fn modules(&self) -> &[ResolvedModule] { &self.modules }
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }
    pub fn module_by_path(&self, path: &str) -> Option<&ResolvedModule> {
        self.modules.iter().find(|module| module.path() == path)
    }
    pub fn item(&self, id: ItemId) -> Option<&ResolvedItem> {
        self.modules
            .get(id.module().raw() as usize)
            .and_then(|module| module.items().get(id.index() as usize))
    }
}
```

- [ ] **Step 4: Implement local module indexing**

Use this deterministic indexing flow:

```rust
pub fn resolve_modules(summaries: &[CheckModuleSummary]) -> ResolvedGraph {
    let valid_summaries = summaries
        .iter()
        .filter(|summary| summary.structurally_valid())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut path_to_module = BTreeMap::<String, ModuleId>::new();
    let mut modules = Vec::new();

    for (module_index, summary) in valid_summaries.iter().enumerate() {
        let module_id = ModuleId::new(module_index as u32);
        let path = summary.module_path().as_dotted();
        if let Some(first) = path_to_module.get(&path).copied() {
            diagnostics.push(duplicate_module(summary, first));
            continue;
        }
        path_to_module.insert(path.clone(), module_id);
        modules.push(index_module(module_id, summary, &mut diagnostics));
    }

    validate_imports(&mut modules, &path_to_module, &valid_summaries, &mut diagnostics);

    ResolvedGraph { modules, diagnostics }
}

fn index_module(
    module_id: ModuleId,
    summary: &CheckModuleSummary,
    diagnostics: &mut Vec<Diagnostic>,
) -> ResolvedModule {
    let mut local_symbols = BTreeMap::new();
    let mut items = Vec::new();
    for item in summary.items() {
        let item_id = ItemId::new(module_id, items.len() as u32);
        if let Some(first_id) = local_symbols.get(item.name().text()).copied() {
            let first_span = items[first_id.index() as usize].span();
            diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolveDuplicate,
                    format!("duplicate item `{}`", item.name().text()),
                )
                .primary(item.name().span(), "duplicate declaration")
                .secondary(first_span, "first declaration is here")
                .finish(),
            );
        } else {
            local_symbols.insert(item.name().text().to_string(), item_id);
        }
        items.push(ResolvedItem {
            id: item_id,
            name: item.name().clone(),
            kind: symbol_kind(item.kind()),
            public: item.is_public(),
            span: item.span(),
        });
    }
    ResolvedModule {
        id: module_id,
        file_id: summary.file_id(),
        path: summary.module_path().as_dotted(),
        items,
        local_symbols,
        imported_symbols: BTreeMap::new(),
    }
}
```

Add:

```rust
fn symbol_kind(kind: ItemKind) -> SymbolKind {
    match kind {
        ItemKind::Data => SymbolKind::Data,
        ItemKind::LayoutData => SymbolKind::LayoutData,
        ItemKind::Class => SymbolKind::Class,
        ItemKind::UniqueClass => SymbolKind::UniqueClass,
        ItemKind::Interface => SymbolKind::Interface,
        ItemKind::Error => SymbolKind::Error,
        ItemKind::Image => SymbolKind::Image,
        ItemKind::HostImage => SymbolKind::HostImage,
    }
}
```

- [ ] **Step 5: Implement import validation**

Use explicit binder imports only. Wildcard imports are parse-unsupported for this plan.

```rust
fn validate_imports(
    modules: &mut [ResolvedModule],
    path_to_module: &BTreeMap<String, ModuleId>,
    summaries: &[&CheckModuleSummary],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for summary in summaries {
        let Some(module_index) = path_to_module
            .get(&summary.module_path().as_dotted())
            .map(|id| id.raw() as usize)
        else {
            continue;
        };
        for import in summary.imports() {
            validate_one_import(module_index, import, modules, path_to_module, diagnostics);
        }
    }
}

fn validate_one_import(
    importing_index: usize,
    import: &CheckImportSummary,
    modules: &mut [ResolvedModule],
    path_to_module: &BTreeMap<String, ModuleId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let imported_path = import.module_path().as_dotted();
    let Some(imported_id) = path_to_module.get(&imported_path).copied() else {
        diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveImport,
                format!("module `{imported_path}` is not reachable"),
            )
            .primary(import.module_path().span(), "imported module was not found")
            .finish(),
        );
        return;
    };

    let imported_index = imported_id.raw() as usize;
    for binder in import.binders() {
        let imported_item = modules[imported_index]
            .resolve_local(binder.text())
            .and_then(|id| modules[imported_index].items().get(id.index() as usize))
            .map(|item| (item.id(), item.is_public(), item.span()));

        match imported_item {
            Some((item_id, true, _item_span)) => {
                if modules[importing_index].resolve_local(binder.text()).is_some()
                    || modules[importing_index].resolve_imported(binder.text()).is_some()
                {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::ResolveDuplicate,
                            format!("duplicate imported name `{}`", binder.text()),
                        )
                        .primary(binder.span(), "import conflicts with an existing name")
                        .finish(),
                    );
                } else {
                    modules[importing_index]
                        .imported_symbols
                        .insert(binder.text().to_string(), item_id);
                }
            }
            Some((_item_id, false, item_span)) => diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolvePrivate,
                    format!("`{}` is private in module `{imported_path}`", binder.text()),
                )
                .primary(binder.span(), "private item imported here")
                .secondary(item_span, "item is declared without `pub`")
                .finish(),
            ),
            None => diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolveImport,
                    format!("module `{imported_path}` does not export `{}`", binder.text()),
                )
                .primary(binder.span(), "missing exported item")
                .finish(),
            ),
        }
    }
}
```

Add `duplicate_module`:

```rust
fn duplicate_module(summary: &CheckModuleSummary, _first: ModuleId) -> Diagnostic {
    Diagnostic::builder(
        Severity::Error,
        DiagnosticCode::ResolveDuplicate,
        format!("duplicate module `{}`", summary.module_path().as_dotted()),
    )
    .primary(summary.module_path().span(), "module path is already used")
    .finish()
}
```

- [ ] **Step 6: Wire resolver into pipeline**

Update `src/check/mod.rs`:

```rust
pub mod resolve;

use crate::check::resolve::{ResolvedGraph, resolve_modules};

pub struct CheckResult {
    source_map: SourceMap,
    parsed: Vec<ParsedSyntax>,
    summaries: Vec<CheckModuleSummary>,
    resolved_graph: ResolvedGraph,
    diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn resolved_graph(&self) -> &ResolvedGraph {
        &self.resolved_graph
    }
}
```

After summaries are collected:

```rust
let resolved_graph = resolve_modules(&summaries);
diagnostics.extend(resolved_graph.diagnostics().iter().cloned());
```

- [ ] **Step 7: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- `ResolvedModule` is defined with getters.
- Structurally invalid modules are skipped by resolver; independent valid modules still resolve.
- Duplicate local item diagnostics include primary and secondary spans.
- Import validation distinguishes missing, private, and duplicate imports.
- Resolver diagnostics are deterministic.

---

### Task 6: JSON And Handoff Verification

**Files:**
- Modify: `src/check/json.rs`
- Modify: `tests/check.rs`

**Description:** Ensure JSON output carries summary/resolution diagnostics and source hashes without changing the schema name.

- [ ] **Step 1: Add JSON diagnostic test**

Add to `tests/check.rs`:

```rust
#[test]
fn check_json_includes_resolve_code_and_source_hash() {
    let dir = temp_dir("check-json-resolve");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub data Bytes { value: U32 }\n",
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
    assert!(json.contains("\"code\":\"W-RESOLVE-DUPLICATE\""));
    assert!(json.contains("\"sourceHash\""));
    assert!(json.contains("\"schema\":\"wrela.check.v1\""));
}
```

- [ ] **Step 2: Run focused test**

```bash
cargo test --test check check_json_includes_resolve_code_and_source_hash
```

Expected result: test passes after `render_diagnostic` renders non-parse diagnostics exactly like parse diagnostics.

- [ ] **Step 3: Render secondary and related locations in JSON**

Replace the hard-coded empty `secondary` and `related` arrays in `src/check/json.rs` with:

```rust
out.push_str("\"secondary\":");
render_labels(out, diagnostic.secondary(), source_map);
comma(out);
out.push_str("\"related\":");
render_related(out, diagnostic.related(), source_map);
```

Add helpers:

```rust
fn render_labels(out: &mut String, labels: &[DiagnosticLabel], source_map: &SourceMap) {
    out.push('[');
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_label(out, label.span(), label.message(), source_map);
    }
    out.push(']');
}

fn render_related(out: &mut String, related: &[RelatedLocation], source_map: &SourceMap) {
    out.push('[');
    for (index, location) in related.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_label(out, location.span(), location.message(), source_map);
    }
    out.push(']');
}
```

Update the imports in `src/check/json.rs`:

```rust
use crate::diagnostic::{
    Diagnostic, DiagnosticGroupRole, DiagnosticLabel, RelatedLocation, Severity,
};
```

- [ ] **Step 4: Run quality gate**

```bash
./scripts/quality-gate.sh
```

Expected result: all checks pass.

- [ ] **Step 5: Commit**

```bash
git add src/check src/diagnostic.rs tests/check.rs
git commit -m "feat: add semantic summaries and resolver -Codex Automated"
```

**Acceptance Criteria:**
- `CheckResult` exposes summaries and resolved graph.
- JSON remains the default `check` output.
- Human diagnostics show summary and resolver labels.
- Quality gate passes.
- Phase A is not required here when this child plan is executed as part of the parent check delivery; run Phase A here only if this child plan is handed to the user independently.
