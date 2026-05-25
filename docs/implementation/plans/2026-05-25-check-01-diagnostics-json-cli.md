# Check 01: Diagnostics, JSON, And CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the diagnostic infrastructure and read-only `wrela check` command shell that later semantic phases build on.

**Architecture:** `diagnostic.rs` owns diagnostic data and human rendering helpers. `source.rs` owns locations and source hashes. `check/mod.rs` owns the pipeline result and orchestration shell. `check/json.rs` owns handwritten JSON rendering. `command.rs` remains the only module that writes output.

**Tech Stack:** Rust 2024, standard library only, existing `discover_from_root`, `parse_files_parallel`, and CST parser.

---

## File Ownership

```text
src/
  diagnostic.rs          structured diagnostics, codes, human renderer
  source.rs              cloneable source map, line/column, source hashes
  syntax/parse.rs        parser diagnostic builder callsite
  syntax/syntax_kind.rs  syntax error kind list
  check/mod.rs           CheckResult, CheckFormat, check_root
  check/json.rs          JSON renderer for wrela.check.v1
  command.rs             CLI flag parsing and output only
  lib.rs                 exports check module
tests/
  check.rs               CLI and public check behavior
docs/design/
  diagnostic-codes.md    diagnostic code registry
```

## Real Parallel Work

- Task 1 runs first and owns `src/diagnostic.rs`.
- After Task 1, Task 2 (`src/source.rs`) and Task 3 (`docs/design/diagnostic-codes.md`, parser code mapping) may run in parallel.
- Task 4 depends on Tasks 1-3 and owns `src/check/*`, `src/command.rs`, `src/lib.rs`, and `tests/check.rs`.
- Task 5 runs last and verifies the full product shell.

Subagents may run the focused `cargo test` commands listed in their task with a timeout set in the execution tool. Recommended focused-command timeout: 120 seconds. The orchestrator runs `./scripts/quality-gate.sh` sequentially after each task branch is merged.

---

### Task 1: Structured Diagnostic Data

**Files:**
- Modify: `src/diagnostic.rs`

**Description:** Replace the compact-only diagnostic model with structured diagnostic data while preserving compatibility constructors used by lexer, discovery, and parser code.

- [ ] **Step 1: Write failing diagnostic data tests**

Add these tests to `src/diagnostic.rs`:

```rust
#[test]
fn structured_diagnostic_stores_codes_labels_groups_and_fixes() {
    let span = Span::new(FileId::new(0), 10, 16);
    let fix_span = Span::new(FileId::new(0), 10, 16);
    let group = DiagnosticGroupId::new(7);

    let diagnostic = Diagnostic::builder(
        Severity::Error,
        DiagnosticCode::ResolveUnknownName,
        "unknown name `Consol`",
    )
    .primary(span, "`Consol` is not in scope")
    .secondary(Span::new(FileId::new(0), 1, 8), "available import is declared here")
    .related(span, "lookup failed in this scope")
    .note("names are resolved after imports are validated")
    .help("did you mean `Console`?")
    .suggestion(
        "replace with `Console`",
        Applicability::Likely,
        vec![SourceEdit::replace(fix_span, "Console")],
    )
    .root_cause_group(group)
    .finish();

    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(diagnostic.code(), Some(DiagnosticCode::ResolveUnknownName));
    assert_eq!(diagnostic.code_string(), Some("W-RESOLVE-NAME"));
    assert_eq!(diagnostic.phase(), DiagnosticPhase::Resolve);
    assert_eq!(diagnostic.span(), Some(span));
    assert_eq!(diagnostic.primary().unwrap().span(), span);
    assert_eq!(diagnostic.secondary().len(), 1);
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(diagnostic.notes(), &["names are resolved after imports are validated"]);
    assert_eq!(diagnostic.help(), &["did you mean `Console`?"]);
    assert_eq!(diagnostic.suggestions()[0].applicability(), Applicability::Likely);
    assert_eq!(diagnostic.suggestions()[0].edits()[0].replacement(), "Console");
    assert_eq!(diagnostic.group().unwrap().role(), DiagnosticGroupRole::RootCause);
}

#[test]
fn compatibility_constructors_keep_existing_span_api() {
    let span = Span::new(FileId::new(2), 4, 9);
    let diagnostic = Diagnostic::error(span, "bad token");

    assert_eq!(diagnostic.code(), None);
    assert_eq!(diagnostic.phase(), DiagnosticPhase::Internal);
    assert_eq!(diagnostic.span(), Some(span));
    assert_eq!(diagnostic.message(), "bad token");
    assert_eq!(diagnostic.render_compact(), "error[file=2 4..9]: bad token");
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test diagnostic::tests
```

Expected result: compilation fails because the new diagnostic types and getters do not exist.

- [ ] **Step 3: Add exact diagnostic data types**

Use this model in `src/diagnostic.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPhase {
    Lex,
    Parse,
    Summary,
    Resolve,
    Type,
    Ownership,
    Effect,
    Layout,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    ParseExpectedItem,
    ParseExpectedIdentifier,
    ParseExpectedImportBinderList,
    ParseExpectedFrom,
    ParseExpectedModulePath,
    ParseInvalidImportBinder,
    ParseExpectedType,
    ParseExpectedExpression,
    ParseExpectedReturnAfterElse,
    ParseExpectedMatchArm,
    ParseExpectedAssertKind,
    ParseExpectedToken,
    ParseUnexpectedToken,
    ParseMissingCloseDelimiter,
    ParseExpressionTooDeep,
    SummaryInvalidModule,
    ResolveDuplicate,
    ResolveImport,
    ResolvePrivate,
    ResolveUnknownName,
    ResolveWrongKind,
    TypeUnknownType,
    TypeMismatch,
    TypeReturn,
    TypeCall,
    TypeArgument,
    OwnershipMove,
    OwnershipAccess,
    EffectUnsupported,
    LayoutInvalid,
    CheckUnsupported,
    CheckIo,
}

impl DiagnosticCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ParseExpectedItem => "W-PARSE-ITEM",
            Self::ParseExpectedIdentifier => "W-PARSE-IDENT",
            Self::ParseExpectedImportBinderList => "W-PARSE-IMPORT-BINDERS",
            Self::ParseExpectedFrom => "W-PARSE-FROM",
            Self::ParseExpectedModulePath => "W-PARSE-MODULE-PATH",
            Self::ParseInvalidImportBinder => "W-PARSE-IMPORT-BINDER",
            Self::ParseExpectedType => "W-PARSE-EXPECTED-TYPE",
            Self::ParseExpectedExpression => "W-PARSE-EXPR",
            Self::ParseExpectedReturnAfterElse => "W-PARSE-RETURN-ELSE",
            Self::ParseExpectedMatchArm => "W-PARSE-MATCH-ARM",
            Self::ParseExpectedAssertKind => "W-PARSE-ASSERT-KIND",
            Self::ParseExpectedToken => "W-PARSE-TOKEN",
            Self::ParseUnexpectedToken => "W-PARSE-UNEXPECTED",
            Self::ParseMissingCloseDelimiter => "W-PARSE-DELIM",
            Self::ParseExpressionTooDeep => "W-PARSE-DEPTH",
            Self::SummaryInvalidModule => "W-SUMMARY-INVALID",
            Self::ResolveDuplicate => "W-RESOLVE-DUPLICATE",
            Self::ResolveImport => "W-RESOLVE-IMPORT",
            Self::ResolvePrivate => "W-RESOLVE-PRIVATE",
            Self::ResolveUnknownName => "W-RESOLVE-NAME",
            Self::ResolveWrongKind => "W-RESOLVE-KIND",
            Self::TypeUnknownType => "W-TYPE-UNKNOWN",
            Self::TypeMismatch => "W-TYPE-MISMATCH",
            Self::TypeReturn => "W-TYPE-RETURN",
            Self::TypeCall => "W-TYPE-CALL",
            Self::TypeArgument => "W-TYPE-ARG",
            Self::OwnershipMove => "W-OWN-MOVE",
            Self::OwnershipAccess => "W-OWN-ACCESS",
            Self::EffectUnsupported => "W-EFFECT-UNSUPPORTED",
            Self::LayoutInvalid => "W-LAYOUT-INVALID",
            Self::CheckUnsupported => "W-CHECK-UNSUPPORTED",
            Self::CheckIo => "W-CHECK-IO",
        }
    }

    pub const fn phase(self) -> DiagnosticPhase {
        match self {
            Self::ParseExpectedItem
            | Self::ParseExpectedIdentifier
            | Self::ParseExpectedImportBinderList
            | Self::ParseExpectedFrom
            | Self::ParseExpectedModulePath
            | Self::ParseInvalidImportBinder
            | Self::ParseExpectedType
            | Self::ParseExpectedExpression
            | Self::ParseExpectedReturnAfterElse
            | Self::ParseExpectedMatchArm
            | Self::ParseExpectedAssertKind
            | Self::ParseExpectedToken
            | Self::ParseUnexpectedToken
            | Self::ParseMissingCloseDelimiter
            | Self::ParseExpressionTooDeep => DiagnosticPhase::Parse,
            Self::SummaryInvalidModule => DiagnosticPhase::Summary,
            Self::ResolveDuplicate
            | Self::ResolveImport
            | Self::ResolvePrivate
            | Self::ResolveUnknownName
            | Self::ResolveWrongKind => DiagnosticPhase::Resolve,
            Self::TypeUnknownType
            | Self::TypeMismatch
            | Self::TypeReturn
            | Self::TypeCall
            | Self::TypeArgument => DiagnosticPhase::Type,
            Self::OwnershipMove | Self::OwnershipAccess => DiagnosticPhase::Ownership,
            Self::EffectUnsupported => DiagnosticPhase::Effect,
            Self::LayoutInvalid => DiagnosticPhase::Layout,
            Self::CheckUnsupported | Self::CheckIo => DiagnosticPhase::Internal,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    span: Span,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatedLocation {
    span: Span,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Applicability {
    Certain,
    Likely,
    Maybe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEdit {
    span: Span,
    replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedFix {
    title: String,
    applicability: Applicability,
    edits: Vec<SourceEdit>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticGroupId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticGroupRole {
    RootCause,
    Cascade,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticGroup {
    id: DiagnosticGroupId,
    role: DiagnosticGroupRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    code: Option<DiagnosticCode>,
    phase: DiagnosticPhase,
    message: String,
    primary: Option<DiagnosticLabel>,
    secondary: Vec<DiagnosticLabel>,
    related: Vec<RelatedLocation>,
    notes: Vec<String>,
    help: Vec<String>,
    suggestions: Vec<SuggestedFix>,
    group: Option<DiagnosticGroup>,
}
```

Add getters for every field used by tests and renderers:

```rust
impl Diagnostic {
    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn code(&self) -> Option<DiagnosticCode> {
        self.code
    }

    pub fn code_string(&self) -> Option<&'static str> {
        self.code.map(DiagnosticCode::code)
    }

    pub fn phase(&self) -> DiagnosticPhase {
        self.phase
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> Option<Span> {
        self.primary.as_ref().map(DiagnosticLabel::span)
    }

    pub fn primary(&self) -> Option<&DiagnosticLabel> {
        self.primary.as_ref()
    }

    pub fn secondary(&self) -> &[DiagnosticLabel] {
        &self.secondary
    }

    pub fn related(&self) -> &[RelatedLocation] {
        &self.related
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn help(&self) -> &[String] {
        &self.help
    }

    pub fn suggestions(&self) -> &[SuggestedFix] {
        &self.suggestions
    }

    pub fn group(&self) -> Option<DiagnosticGroup> {
        self.group
    }
}

impl DiagnosticLabel {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl RelatedLocation {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl SourceEdit {
    pub fn replace(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

impl SuggestedFix {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn applicability(&self) -> Applicability {
        self.applicability
    }

    pub fn edits(&self) -> &[SourceEdit] {
        &self.edits
    }
}

impl DiagnosticGroupId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl DiagnosticGroup {
    pub const fn id(self) -> DiagnosticGroupId {
        self.id
    }

    pub const fn role(self) -> DiagnosticGroupRole {
        self.role
    }
}
```

- [ ] **Step 4: Add the builder**

Use a consuming builder so call sites stay concise:

```rust
pub struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl Diagnostic {
    pub fn builder(
        severity: Severity,
        code: DiagnosticCode,
        message: impl Into<String>,
    ) -> DiagnosticBuilder {
        DiagnosticBuilder {
            diagnostic: Diagnostic {
                severity,
                code: Some(code),
                phase: code.phase(),
                message: message.into(),
                primary: None,
                secondary: Vec::new(),
                related: Vec::new(),
                notes: Vec::new(),
                help: Vec::new(),
                suggestions: Vec::new(),
                group: None,
            },
        }
    }
}

impl DiagnosticBuilder {
    pub fn primary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.diagnostic.primary = Some(DiagnosticLabel::new(span, label));
        self
    }

    pub fn secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.diagnostic.secondary.push(DiagnosticLabel::new(span, label));
        self
    }

    pub fn related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.diagnostic.related.push(RelatedLocation::new(span, message));
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic.notes.push(note.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic.help.push(help.into());
        self
    }

    pub fn suggestion(
        mut self,
        title: impl Into<String>,
        applicability: Applicability,
        edits: Vec<SourceEdit>,
    ) -> Self {
        self.diagnostic.suggestions.push(SuggestedFix {
            title: title.into(),
            applicability,
            edits,
        });
        self
    }

    pub fn root_cause_group(mut self, id: DiagnosticGroupId) -> Self {
        self.diagnostic.group = Some(DiagnosticGroup {
            id,
            role: DiagnosticGroupRole::RootCause,
        });
        self
    }

    pub fn cascade_group(mut self, id: DiagnosticGroupId) -> Self {
        self.diagnostic.group = Some(DiagnosticGroup {
            id,
            role: DiagnosticGroupRole::Cascade,
        });
        self
    }

    pub fn finish(self) -> Diagnostic {
        self.diagnostic
    }
}
```

- [ ] **Step 5: Preserve compatibility constructors**

Compatibility constructors must produce structured diagnostics with no code and `DiagnosticPhase::Internal`:

```rust
impl Diagnostic {
    pub fn new(severity: Severity, span: Option<Span>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            severity,
            code: None,
            phase: DiagnosticPhase::Internal,
            primary: span.map(|span| DiagnosticLabel::new(span, "")),
            secondary: Vec::new(),
            related: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            suggestions: Vec::new(),
            group: None,
            message,
        }
    }

    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, Some(span), message)
    }

    pub fn unspanned_error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, None, message)
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, Some(span), message)
    }
}
```

- [ ] **Step 6: Run focused tests**

```bash
cargo test diagnostic::tests
```

Expected result: both tests pass.

**Acceptance Criteria:**
- All existing diagnostic call sites compile.
- Lexer, discovery, and parser call sites using `Diagnostic::error`, `Diagnostic::warning`, and `Diagnostic::unspanned_error` keep compiling without semantic changes.
- `Diagnostic::span()` remains `Option<Span>`.
- Every getter used by tests and renderers exists.
- `has_errors` treats only `Severity::Error` as an error.

---

### Task 2: Source Locations And Hashes

**Files:**
- Modify: `src/source.rs`

**Description:** Add clone support, source line/column helpers, span text extraction, and deterministic source hashes for diagnostic JSON preconditions.

- [ ] **Step 1: Write failing source tests**

Add tests to `src/source.rs`:

```rust
#[test]
fn reports_one_based_line_and_column() {
    let file = SourceFile::new(
        FileId::new(0),
        PathBuf::from("root.wrela"),
        "first\nsecond\n".to_string(),
    );

    assert_eq!(file.line_col(0), SourceLocation::new(1, 1));
    assert_eq!(file.line_col(6), SourceLocation::new(2, 1));
    assert_eq!(file.line_col(12), SourceLocation::new(2, 7));
}

#[test]
fn returns_span_text_and_stable_hash() {
    let file = SourceFile::new(
        FileId::new(3),
        PathBuf::from("root.wrela"),
        "module app\n".to_string(),
    );
    let span = Span::new(FileId::new(3), 0, 6);

    assert_eq!(file.span_text(span), Some("module"));
    assert_eq!(file.source_hash(), SourceHash::from_text("module app\n"));
    let hash = file.source_hash().to_hex();
    assert_eq!(hash.len(), 16);
    assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test source::tests
```

Expected result: compilation fails because `SourceLocation`, `SourceHash`, and helper methods do not exist.

- [ ] **Step 3: Derive Clone and add helpers**

Update the source types:

```rust
#[derive(Clone, Debug)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    text: String,
    line_starts: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    line: u32,
    column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceHash(u64);
```

Use this line/column implementation:

```rust
impl SourceLocation {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    pub const fn line(self) -> u32 {
        self.line
    }

    pub const fn column(self) -> u32 {
        self.column
    }
}

impl SourceFile {
    pub fn line_col(&self, offset: u32) -> SourceLocation {
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        SourceLocation::new(line_index as u32 + 1, offset.saturating_sub(line_start) + 1)
    }

    pub fn span_text(&self, span: Span) -> Option<&str> {
        if span.file_id() != self.id {
            return None;
        }
        self.text.get(span.start() as usize..span.end() as usize)
    }

    pub fn source_hash(&self) -> SourceHash {
        SourceHash::from_text(&self.text)
    }
}
```

- [ ] **Step 4: Add FNV-1a hash**

Use 64-bit FNV-1a with fixed constants:

```rust
impl SourceHash {
    pub fn from_text(text: &str) -> Self {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(hash)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}
```

- [ ] **Step 5: Run focused tests**

```bash
cargo test source::tests
```

Expected result: both tests pass.

**Acceptance Criteria:**
- `SourceFile` and `SourceMap` are cloneable.
- Line and column values are 1-based.
- Hashes are stable across platforms.
- `span_text` returns `None` for spans from a different file.

---

### Task 3: Diagnostic Code Registry And Parser Codes

**Files:**
- Create: `docs/design/diagnostic-codes.md`
- Modify: `src/syntax/parse.rs`
- Modify: `src/syntax/syntax_kind.rs`
- Modify: `src/diagnostic.rs`

**Description:** Make diagnostic code ownership explicit and ensure parser diagnostics use parse-phase codes, including `W-PARSE-EXPECTED-TYPE`.

- [ ] **Step 1: Write the diagnostic code registry**

Create `docs/design/diagnostic-codes.md`:

```markdown
# Diagnostic Code Registry

Date: 2026-05-25

This registry owns Wrela diagnostic code strings. Codes are stable once released.
Retired codes remain listed with status `retired` and must not be reused.

## Format

Codes use `W-<PHASE>-<NAME>`.

Phases:

- `PARSE`
- `SUMMARY`
- `RESOLVE`
- `TYPE`
- `OWN`
- `EFFECT`
- `LAYOUT`
- `CHECK`

## Active Codes

| Code | Phase | Meaning |
|------|-------|---------|
| `W-PARSE-ITEM` | Parse | Expected a top-level item |
| `W-PARSE-IDENT` | Parse | Expected an identifier |
| `W-PARSE-IMPORT-BINDERS` | Parse | Expected an import binder list |
| `W-PARSE-FROM` | Parse | Expected `from` in an import |
| `W-PARSE-MODULE-PATH` | Parse | Expected a module path |
| `W-PARSE-IMPORT-BINDER` | Parse | Invalid import binder |
| `W-PARSE-EXPECTED-TYPE` | Parse | Expected a type syntax node |
| `W-PARSE-EXPR` | Parse | Expected an expression |
| `W-PARSE-RETURN-ELSE` | Parse | Expected `return` after `else` |
| `W-PARSE-MATCH-ARM` | Parse | Expected a match arm |
| `W-PARSE-ASSERT-KIND` | Parse | Expected an assert kind |
| `W-PARSE-TOKEN` | Parse | Expected a specific token |
| `W-PARSE-UNEXPECTED` | Parse | Unexpected token |
| `W-PARSE-DELIM` | Parse | Missing close delimiter |
| `W-PARSE-DEPTH` | Parse | Expression nesting is too deep |
| `W-SUMMARY-INVALID` | Summary | CST could not produce a trustworthy module summary |
| `W-RESOLVE-DUPLICATE` | Resolve | Duplicate symbol or import |
| `W-RESOLVE-IMPORT` | Resolve | Import target is missing |
| `W-RESOLVE-PRIVATE` | Resolve | Import target is private |
| `W-RESOLVE-NAME` | Resolve | Name could not be resolved |
| `W-RESOLVE-KIND` | Resolve | Name resolved to the wrong kind |
| `W-TYPE-UNKNOWN` | Type | Type name could not be resolved |
| `W-TYPE-MISMATCH` | Type | Expression type does not match expected type |
| `W-TYPE-RETURN` | Type | Return expression does not match signature |
| `W-TYPE-CALL` | Type | Call target is not callable |
| `W-TYPE-ARG` | Type | Call argument mismatch |
| `W-OWN-MOVE` | Ownership | Value is used after move or moved illegally |
| `W-OWN-ACCESS` | Ownership | Access mode is too weak for an operation |
| `W-EFFECT-UNSUPPORTED` | Effect | Effect cannot be checked for this construct |
| `W-LAYOUT-INVALID` | Layout | Layout declaration is illegal |
| `W-CHECK-UNSUPPORTED` | Check | Parsed construct is not semantically supported yet |
| `W-CHECK-IO` | Check | Checker could not read or write required diagnostic data |

## Retirement Policy

When a code is replaced, move it to this section with the release date and replacement.

No codes are retired yet.
```

- [ ] **Step 2: Write failing parser code test**

Add this test to `src/diagnostic.rs`:

```rust
#[test]
fn syntax_error_kind_expected_type_is_parse_phase() {
    assert_eq!(
        DiagnosticCode::from_syntax_error(SyntaxErrorKind::ExpectedType),
        DiagnosticCode::ParseExpectedType
    );
    assert_eq!(DiagnosticCode::ParseExpectedType.phase(), DiagnosticPhase::Parse);
    assert_eq!(DiagnosticCode::ParseExpectedType.code(), "W-PARSE-EXPECTED-TYPE");
}
```

Import `SyntaxErrorKind` in the diagnostic test module.

- [ ] **Step 3: Add syntax-code mapping**

Add to `DiagnosticCode`:

```rust
pub const fn from_syntax_error(kind: SyntaxErrorKind) -> Self {
    match kind {
        SyntaxErrorKind::ExpectedItem => Self::ParseExpectedItem,
        SyntaxErrorKind::ExpectedIdentifier => Self::ParseExpectedIdentifier,
        SyntaxErrorKind::ExpectedImportBinderList => Self::ParseExpectedImportBinderList,
        SyntaxErrorKind::ExpectedFrom => Self::ParseExpectedFrom,
        SyntaxErrorKind::ExpectedModulePath => Self::ParseExpectedModulePath,
        SyntaxErrorKind::InvalidImportBinder => Self::ParseInvalidImportBinder,
        SyntaxErrorKind::ExpectedType => Self::ParseExpectedType,
        SyntaxErrorKind::ExpectedExpression => Self::ParseExpectedExpression,
        SyntaxErrorKind::ExpectedReturnAfterElse => Self::ParseExpectedReturnAfterElse,
        SyntaxErrorKind::ExpectedMatchArm => Self::ParseExpectedMatchArm,
        SyntaxErrorKind::ExpectedAssertKind => Self::ParseExpectedAssertKind,
        SyntaxErrorKind::ExpectedToken => Self::ParseExpectedToken,
        SyntaxErrorKind::UnexpectedToken => Self::ParseUnexpectedToken,
        SyntaxErrorKind::MissingCloseDelimiter => Self::ParseMissingCloseDelimiter,
        SyntaxErrorKind::ExpressionTooDeep => Self::ParseExpressionTooDeep,
    }
}
```

- [ ] **Step 4: Update parser diagnostic callsite**

Change `Parser::error_at_current` and `Parser::diagnostic` so parser diagnostics are structured:

```rust
pub(crate) fn error_at_current(&mut self, kind: SyntaxErrorKind, message: &'static str) {
    let span = self.peek().span();
    self.diagnostic(kind, span, message);
    self.builder.error(kind, span);
}

pub(crate) fn diagnostic(&mut self, kind: SyntaxErrorKind, span: Span, message: &'static str) {
    let code = DiagnosticCode::from_syntax_error(kind);
    self.diagnostics.push(
        Diagnostic::builder(Severity::Error, code, message)
            .primary(span, message)
            .finish(),
    );
}
```

Update every direct `diagnostic` caller:

```rust
// src/syntax/items.rs
self.diagnostic(
    SyntaxErrorKind::ExpectedImportBinderList,
    span,
    "expected import binder list",
);
self.diagnostic(SyntaxErrorKind::ExpectedFrom, span, "expected from in use import");
self.diagnostic(
    SyntaxErrorKind::InvalidImportBinder,
    span,
    "wildcard imports are not supported in v1",
);
self.diagnostic(
    SyntaxErrorKind::InvalidImportBinder,
    span,
    "import aliases are not supported in v1",
);
self.diagnostic(
    SyntaxErrorKind::ExpectedModulePath,
    span,
    "expected module path after from",
);
```

Current direct callers are in `src/syntax/items.rs` inside import parsing. If more direct callers are added before implementation, update them in the same commit; do not leave any `self.diagnostic(span, message)` call sites.

- [ ] **Step 5: Run focused tests**

```bash
cargo test diagnostic::testssyntax_error_kind_expected_type_is_parse_phase
cargo test syntax
```

Expected result: tests pass and no parser diagnostic maps to a non-parse phase.

**Acceptance Criteria:**
- `W-PARSE-EXPECTED-TYPE` exists and is used for `SyntaxErrorKind::ExpectedType`.
- Parser diagnostics have primary spans.
- The registry contains every `DiagnosticCode` variant.
- No parser diagnostic uses a type-phase code.

---

### Task 4: Read-Only Check Pipeline, JSON Default, And CLI

**Files:**
- Create: `src/check/mod.rs`
- Create: `src/check/json.rs`
- Modify: `src/lib.rs`
- Modify: `src/command.rs`
- Create: `tests/check.rs`

**Description:** Add `wrela check` with a parse-only semantic shell. Later plans add summaries and semantic phases behind this stable command.

- [ ] **Step 1: Write failing integration tests**

Create `tests/check.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("wrela-{name}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn check_default_json_is_machine_readable() {
    let dir = temp_dir("check-json");
    let root = write_file(&dir, "root.wrela", "module root\npub data Bytes { value: U32 }\n");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"schema\":\"wrela.check.v1\""));
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"diagnostics\":[]"));
    assert!(json.contains("\"sourceFiles\""));
}

#[test]
fn check_human_renders_parse_diagnostic_with_source() {
    let dir = temp_dir("check-human");
    let root = write_file(&dir, "root.wrela", "module root\npub data Broken { field: }\n");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            "--human".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(err.is_empty());
    let rendered = String::from_utf8(out).unwrap();
    assert!(rendered.contains("error[W-PARSE-EXPECTED-TYPE]"));
    assert!(rendered.contains("-->"));
    assert!(rendered.contains("root.wrela"));
    assert!(rendered.contains("field:"));
}

#[test]
fn check_rejects_conflicting_format_flags() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            "--json".to_string(),
            "--human".to_string(),
            "root.wrela".to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(String::from_utf8(err).unwrap().contains("malformed command"));
}
```

- [ ] **Step 2: Run tests and verify failure**

```bash
cargo test --test check
```

Expected result: tests fail because `check` is not exported and the CLI does not recognize the command.

- [ ] **Step 3: Add `check` module shell**

Create `src/check/mod.rs`:

```rust
pub mod json;

use std::path::Path;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::discover::discover_from_root;
use crate::source::SourceMap;
use crate::syntax::{ParsedSyntax, parse_files_parallel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckFormat {
    Json,
    Human,
}

#[derive(Debug)]
pub struct CheckResult {
    source_map: SourceMap,
    parsed: Vec<ParsedSyntax>,
    diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn parsed(&self) -> &[ParsedSyntax] {
        &self.parsed
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

pub fn check_root(root: impl AsRef<Path>) -> CheckResult {
    let discovered = discover_from_root(root);
    let source_map = discovered.source_map().clone();
    let parsed = parse_files_parallel(discovered.lexed_files(), &source_map);

    let mut diagnostics = Vec::new();
    diagnostics.extend(discovered.diagnostics().iter().cloned());
    for parsed_file in &parsed {
        diagnostics.extend(parsed_file.diagnostics().iter().cloned());
    }

    CheckResult {
        source_map,
        parsed,
        diagnostics,
    }
}
```

Add to `src/lib.rs`:

```rust
pub mod check;
```

- [ ] **Step 4: Add JSON renderer**

Create `src/check/json.rs`:

```rust
use crate::check::CheckResult;
use crate::diagnostic::{Diagnostic, DiagnosticGroupRole, Severity};
use crate::source::{SourceFile, SourceMap, Span};

pub fn render_check_json(result: &CheckResult) -> String {
    let mut out = String::new();
    out.push('{');
    field_str(&mut out, "schema", "wrela.check.v1");
    comma(&mut out);
    field_bool(&mut out, "ok", result.ok());
    comma(&mut out);
    field_usize(&mut out, "checkedFileCount", result.source_map().files().len());
    comma(&mut out);
    out.push_str("\"sourceFiles\":");
    render_source_files(&mut out, result.source_map());
    comma(&mut out);
    out.push_str("\"diagnostics\":");
    render_diagnostics(&mut out, result.diagnostics(), result.source_map());
    out.push('}');
    out
}

fn render_source_files(out: &mut String, source_map: &SourceMap) {
    out.push('[');
    for (index, file) in source_map.files().iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_source_file(out, file);
    }
    out.push(']');
}

fn render_source_file(out: &mut String, file: &SourceFile) {
    out.push('{');
    field_u32(out, "id", file.id().raw());
    comma(out);
    let path = file.path().to_string_lossy().replace('\\', "/");
    field_str(out, "path", &path);
    comma(out);
    field_str(out, "hash", &file.source_hash().to_hex());
    out.push('}');
}

fn render_diagnostics(out: &mut String, diagnostics: &[Diagnostic], source_map: &SourceMap) {
    out.push('[');
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_diagnostic(out, diagnostic, source_map);
    }
    out.push(']');
}

fn render_diagnostic(out: &mut String, diagnostic: &Diagnostic, source_map: &SourceMap) {
    out.push('{');
    field_str(out, "severity", severity_text(diagnostic.severity()));
    comma(out);
    field_str(out, "phase", &format!("{:?}", diagnostic.phase()).to_ascii_lowercase());
    comma(out);
    out.push_str("\"code\":");
    match diagnostic.code_string() {
        Some(code) => json_string(out, code),
        None => out.push_str("null"),
    }
    comma(out);
    field_str(out, "message", diagnostic.message());
    comma(out);
    out.push_str("\"primary\":");
    match diagnostic.primary() {
        Some(label) => render_label(out, label.span(), label.message(), source_map),
        None => out.push_str("null"),
    }
    comma(out);
    out.push_str("\"secondary\":[]");
    comma(out);
    out.push_str("\"related\":[]");
    comma(out);
    out.push_str("\"notes\":[]");
    comma(out);
    out.push_str("\"help\":[]");
    comma(out);
    out.push_str("\"suggestions\":[]");
    comma(out);
    out.push_str("\"group\":");
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
    out.push('}');
}

fn render_label(out: &mut String, span: Span, message: &str, source_map: &SourceMap) {
    out.push('{');
    field_str(out, "message", message);
    comma(out);
    out.push_str("\"span\":");
    render_span(out, span, source_map);
    out.push('}');
}

fn render_span(out: &mut String, span: Span, source_map: &SourceMap) {
    out.push('{');
    field_u32(out, "fileId", span.file_id().raw());
    comma(out);
    field_u32(out, "start", span.start());
    comma(out);
    field_u32(out, "end", span.end());
    if let Some(file) = source_map.get(span.file_id()) {
        let start = file.line_col(span.start());
        let end = file.line_col(span.end());
        comma(out);
        field_u32(out, "startLine", start.line());
        comma(out);
        field_u32(out, "startColumn", start.column());
        comma(out);
        field_u32(out, "endLine", end.line());
        comma(out);
        field_u32(out, "endColumn", end.column());
        comma(out);
        field_str(out, "sourceHash", &file.source_hash().to_hex());
    }
    out.push('}');
}

fn severity_text(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Hint => "hint",
    }
}

fn group_role_text(role: DiagnosticGroupRole) -> &'static str {
    match role {
        DiagnosticGroupRole::RootCause => "rootCause",
        DiagnosticGroupRole::Cascade => "cascade",
    }
}

fn field_str(out: &mut String, key: &str, value: &str) {
    json_string(out, key);
    out.push(':');
    json_string(out, value);
}

fn field_bool(out: &mut String, key: &str, value: bool) {
    json_string(out, key);
    out.push(':');
    out.push_str(if value { "true" } else { "false" });
}

fn field_usize(out: &mut String, key: &str, value: usize) {
    json_string(out, key);
    out.push(':');
    out.push_str(&value.to_string());
}

fn field_u32(out: &mut String, key: &str, value: u32) {
    json_string(out, key);
    out.push(':');
    out.push_str(&value.to_string());
}

fn comma(out: &mut String) {
    out.push(',');
}

fn json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}
```

After this task, later plans will expand `render_diagnostic` arrays for secondary labels, related locations, notes, help, suggestions, and source edits. This task still creates valid JSON for the parse-only shell.

- [ ] **Step 5: Add human renderer**

Add `SourceMap` to the `src/diagnostic.rs` imports:

```rust
use crate::source::{SourceMap, Span};
```

Add the human renderer to `src/diagnostic.rs`:

```rust
pub fn render_diagnostics(diagnostics: &[Diagnostic], source_map: &SourceMap) -> String {
    let mut rendered = String::new();
    for diagnostic in diagnostics {
        rendered.push_str(&render_one_diagnostic(diagnostic, source_map));
    }
    rendered
}

fn render_one_diagnostic(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
    let mut rendered = String::new();
    rendered.push_str(diagnostic.severity().label());
    if let Some(code) = diagnostic.code_string() {
        rendered.push('[');
        rendered.push_str(code);
        rendered.push(']');
    }
    rendered.push_str(": ");
    rendered.push_str(diagnostic.message());
    rendered.push('\n');

    if let Some(primary) = diagnostic.primary() {
        render_label_block(&mut rendered, "-->", primary.span(), primary.message(), source_map);
    }

    for secondary in diagnostic.secondary() {
        render_label_block(
            &mut rendered,
            "   =",
            secondary.span(),
            secondary.message(),
            source_map,
        );
    }

    for related in diagnostic.related() {
        render_label_block(
            &mut rendered,
            "related:",
            related.span(),
            related.message(),
            source_map,
        );
    }

    for note in diagnostic.notes() {
        rendered.push_str("note: ");
        rendered.push_str(note);
        rendered.push('\n');
    }

    for help in diagnostic.help() {
        rendered.push_str("help: ");
        rendered.push_str(help);
        rendered.push('\n');
    }

    for suggestion in diagnostic.suggestions() {
        rendered.push_str("suggestion (");
        rendered.push_str(applicability_label(suggestion.applicability()));
        rendered.push_str("): ");
        rendered.push_str(suggestion.title());
        rendered.push('\n');
    }

    rendered
}

fn render_label_block(
    rendered: &mut String,
    prefix: &str,
    span: Span,
    label: &str,
    source_map: &SourceMap,
) {
    let Some(file) = source_map.get(span.file_id()) else {
        rendered.push_str(prefix);
        rendered.push_str(" <unknown source>\n");
        return;
    };

    let location = file.line_col(span.start());
    rendered.push_str(prefix);
    rendered.push(' ');
    rendered.push_str(&file.path().to_string_lossy());
    rendered.push(':');
    rendered.push_str(&location.line().to_string());
    rendered.push(':');
    rendered.push_str(&location.column().to_string());
    rendered.push('\n');

    let (line_start, line_end, line_text) = source_line(file.text(), span.start());
    let line_number = location.line();
    rendered.push_str(&format!("{line_number:>4} | {line_text}\n"));

    let start_column = span.start().saturating_sub(line_start) as usize;
    let end = span.end().min(line_end);
    let width = end.saturating_sub(span.start()).max(1) as usize;
    rendered.push_str("     | ");
    rendered.push_str(&" ".repeat(start_column));
    rendered.push_str(&"^".repeat(width));
    if !label.is_empty() {
        rendered.push(' ');
        rendered.push_str(label);
    }
    rendered.push('\n');
}

fn source_line(text: &str, offset: u32) -> (u32, u32, &str) {
    let bytes = text.as_bytes();
    let mut start = offset.min(bytes.len() as u32) as usize;
    while start > 0 && bytes[start - 1] != b'\n' {
        start -= 1;
    }
    let mut end = offset.min(bytes.len() as u32) as usize;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    (start as u32, end as u32, &text[start..end])
}

fn applicability_label(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::Certain => "certain",
        Applicability::Likely => "likely",
        Applicability::Maybe => "maybe",
    }
}
```

The underline is byte-column based for V1 because Wrela source fixtures are ASCII. Multi-line spans underline only the first source line in V1; related spans should carry additional source locations when a diagnostic needs more context.

- [ ] **Step 6: Wire command parsing**

Update help text in `command.rs` to include:

```text
wrela check [--json|--human] <root.wrela>
```

Add a `check` branch:

```rust
Some("check") => run_check_command(&collected, out, err),
```

Add:

```rust
fn run_check_command<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let mut format = crate::check::CheckFormat::Json;
    let mut saw_format_flag = false;
    let mut root: Option<&str> = None;

    for arg in &args[2..] {
        match arg.as_str() {
            "--json" if !saw_format_flag => {
                format = crate::check::CheckFormat::Json;
                saw_format_flag = true;
            }
            "--human" if !saw_format_flag => {
                format = crate::check::CheckFormat::Human;
                saw_format_flag = true;
            }
            "--json" | "--human" => {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
            value if root.is_none() => root = Some(value),
            _ => {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
        }
    }

    let Some(root) = root else {
        let _ = writeln!(err, "missing root file path");
        return 2;
    };

    let result = crate::check::check_root(root);
    match format {
        crate::check::CheckFormat::Json => {
            let _ = writeln!(out, "{}", crate::check::json::render_check_json(&result));
        }
        crate::check::CheckFormat::Human => {
            let _ = write!(
                out,
                "{}",
                crate::diagnostic::render_diagnostics(result.diagnostics(), result.source_map())
            );
        }
    }

    if result.ok() { 0 } else { 1 }
}
```

- [ ] **Step 7: Run focused tests**

```bash
cargo test --test check
```

Expected result: tests pass.

**Acceptance Criteria:**
- `wrela check <root.wrela>` emits JSON by default.
- `wrela check --json <root.wrela>` emits the same JSON shape.
- `wrela check --human <root.wrela>` emits rich text.
- `check` does not rewrite source files.
- `command.rs` remains the only output-writing module.

---

### Task 5: Verification And Handoff

**Files:**
- No production file ownership.

- [ ] **Step 1: Run quality gate**

```bash
./scripts/quality-gate.sh
```

Expected result: all checks pass.

- [ ] **Step 2: Update implementation README status**

If this plan is being landed independently, update `docs/implementation/README.md` so the check row notes that the diagnostic/CLI shell is complete and semantic phases remain in progress.

- [ ] **Step 3: Commit**

```bash
git add src/diagnostic.rs src/source.rs src/syntax/parse.rs src/syntax/syntax_kind.rs src/check src/command.rs src/lib.rs tests/check.rs docs/design/diagnostic-codes.md docs/implementation/README.md
git commit -m "feat: add check diagnostics shell -Codex Automated"
```

**Acceptance Criteria:**
- Focused tests pass.
- `./scripts/quality-gate.sh` passes.
- The plan has no references to canonical formatting as part of `check`.
- JSON default is documented and tested.
- Phase A is not required here when this child plan is executed as part of the parent check delivery; run Phase A here only if this child plan is handed to the user independently.
