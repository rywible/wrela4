# CST Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first production CST parser for Wrela, including broad syntax coverage, attached trivia, recovery, deterministic parallel parsing, and a `wrela parse <root.wrela>` inspection command.

**Architecture:** Discovery remains root-driven and continues using `syntax::imports::parse_import_summary` while the reachable graph expands. Full parsing runs after discovery over each reachable `LexedFile`, creating immutable `ParsedSyntax` CST artifacts. The parser is handwritten recursive descent with rust-analyzer-style CST checkpoints for postfix and binary expression wrapping, and typed views are derived from the CST.

**Tech Stack:** Rust 2024 edition, Rust standard library only, Cargo test harness, handwritten lexer, handwritten CST parser, handwritten CLI parser.

---

## Locked Decisions

- No external crate dependencies are added.
- Production `src/` code must not use `unsafe`, `todo!()`, or `unimplemented!()`.
- Parser invariant panics may use `expect` only for impossible internal states; malformed source input must produce diagnostics and recovery nodes, not panics.
- The first full parser is handwritten recursive descent with Pratt expression parsing.
- The parser targets declarations, types, blocks, statements, expressions, and recovery in this plan.
- The parser produces a lossless CST with syntax nodes, token elements carrying attached trivia, and parser error nodes.
- Trivia attaches to token elements as leading or trailing trivia ranges. Trivia is not a peer child in ordinary CST walks.
- `LexedFile` remains the raw token and trivia source. The CST stores token/trivia indices and spans, not copied source text.
- V1 imports accept explicit imported names only. Aliases and wildcards are parser errors.
- `wrela parse <root.wrela>` is the parser CLI surface. `wrela check` is outside this plan.
- `parse_files_parallel` matches the lexer v1 pattern: scoped threads over chunks sized from `std::thread::available_parallelism()`, followed by deterministic `FileId` sorting.

## Grammar Locked Decisions

- `true`, `false`, `None`, `Self`, `self`, and `_` are identifier tokens. The parser classifies them by source text when a specific grammar position needs that meaning.
- `in` remains a contextual identifier in `for <name> in <expr> <block>`; the lexer keyword set is not changed in this plan.
- Parameter access qualifiers before a parameter name are `read`, `mut`, and `own`. `unique` is a type qualifier after `:`, as in `host: unique MacOSHost`.
- Match arms use `=>` (`Punct::FatArrow`), not `->`.
- Match patterns accepted in this plan: identifier/path patterns, integer literals, string literals, `_`, and integer ranges using `..` or `..=`.
- Equality and comparison chains parse left-associatively in the CST. Type checking will reject semantically invalid chains such as `a == b == c`.
- Call arguments use `Arg` for positional arguments and `NamedArg` only for `Identifier = Expr`. `a.b = 1` is not a named argument.
- Semicolons are optional statement separators. The parser uses token boundaries and statement-start tokens, not newline trivia, to end statements.
- `let <name>` without either `: <Type>` or `= <expr>` is invalid and emits `expected type annotation or initializer in let statement`.
- `try expr else return expr` is the only accepted `try ... else` form in this plan. `try expr else expr` emits `expected return after else in try expression`.
- `reduce` syntax is `reduce <expr> as <row>, <acc>: <Type> = <expr> <block>`.
- `scan` syntax is `scan <expr> as <index> until <expr> <block>`.
- `assert value <expr>` and `assert same <expr>` are the only assertion statement forms in this plan.
- `pub` is represented as a `PubModifier` child preceding the item node inside a `PublicItem` node.
- Method, constructor, phase, and test bodies are parsed as real `Block` nodes with statements from the first task that introduces bodies. There is no raw balanced-token body mode.
- `trap` and `with` remain lexed keywords but are outside the parser grammar in this plan. In item or statement positions they recover through the normal unexpected-token path.
- EOF is emitted as a CST token element so token order stays explicit; source reconstruction skips the EOF token text because it has zero source length.

## Parser Diagnostic Catalog

Tests may assert these exact messages. New parser diagnostics added by this plan must be added here.

```text
expected item
expected identifier
expected import binder list
expected from in use import
expected module path after from
import aliases are not supported in v1
wildcard imports are not supported in v1
expected type
expected type annotation or initializer in let statement
expected expression
expected return after else in try expression
expected match arm
expected assert kind
expected '('
expected ')'
expected '['
expected ']'
expected '{'
expected '}'
expected ':'
expected ','
expected '='
expected '>'
expected data after layout
expected target in image declaration
expected class after unique
expected image after host
expected 'in' in for statement
expected as in repeat statement
expected as in drain statement
expected as in reduce expression
expected as in scan expression
expected until in scan expression
unexpected token in class body
unexpected token in statement
unexpected token
expression too deep
```

## Planned File Structure

```text
src/
  command.rs
  lexer/
    kind.rs
    lex.rs
  syntax/
    mod.rs
    imports.rs
    syntax_kind.rs
    cst.rs
    parse.rs
    types.rs
    expr.rs
    recovery.rs
    lower.rs
tests/
  parser.rs
fixtures/
  parser/
    imports/root.wrela
    imports/app/console.wrela
    declarations-top.wrela
    declarations-members.wrela
    statements-control.wrela
    assertions-reduce-scan.wrela
    parser_harness_smoke.wrela
    recovery.wrela
docs/
  design/compiler-pipeline.md
  design/locked-decisions.md
  design/supported-wrela-subset.md
  implementation/README.md
```

## Public API Shape

```rust
pub fn syntax::parse_file(lexed: &LexedFile, source: &SourceFile) -> ParsedSyntax;

pub fn syntax::parse_files_parallel(
    lexed_files: &[LexedFile],
    source_map: &SourceMap,
) -> Vec<ParsedSyntax>;
```

```rust
pub struct ParsedSyntax {
    file_id: FileId,
    tree: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
}
```

## Parallel Work Map

- Task 1 and Task 2 can run in parallel.
- Task 3 depends on Task 1.
- Task 3.5 depends on Task 3.
- Task 4 depends on Task 3.5.
- Task 5 depends on Task 3.
- Task 6 depends on Task 3.
- Task 7 depends on Task 5 and Task 6.
- Task 8 depends on Task 4 and Task 7.
- Task 9 depends on Task 2 and Task 8.
- Task 10 depends on Task 8.
- Task 11 depends on Task 9 and Task 10.
- Task 12 depends on Task 11.
- Task 13 depends on Task 12.
- Task 14 depends on Task 13.
- Task 15 is the final review gate.

Task 1 creates all `src/syntax/*.rs` module files and all `pub mod` entries. Later tasks do not edit `src/syntax/mod.rs` unless their task explicitly says to add a public re-export in a named section. This keeps parallel branch conflicts mechanical.

Tasks 9 and 10 both touch `parse_stmt` dispatch. Merge Task 9 first, then Task 10 applies the `Assert` arm as a small explicit patch.

## Subagent Git Discipline

Parallel subagents work in separate branches or worktrees named for the task, such as `codex/task-06-expressions`. Each task commit uses `-Codex Automated`. The integration owner merges task branches in dependency order and re-runs that task's focused verification after conflict resolution.

Every task follows this execution loop even when snippets are grouped by file for readability:

1. Add or update the focused test and run it to confirm the expected failure.
2. Implement the production code in the listed files.
3. Run the focused verification command and then `./scripts/quality-gate.sh`.
4. Commit only that task's files with the task's commit message.

---

## Phase 1: Foundations

### Task 1: Add CST Modules, Kinds, Builder, And Checkpoints

**Files:**
- Modify: `src/syntax/mod.rs`
- Create: `src/syntax/syntax_kind.rs`
- Create: `src/syntax/cst.rs`
- Create empty compiling modules: `src/syntax/parse.rs`, `src/syntax/types.rs`, `src/syntax/expr.rs`, `src/syntax/recovery.rs`, `src/syntax/lower.rs`

**Description:** Add all syntax modules, CST node/token/error kinds, storage types, a builder with checkpoints, exact source reconstruction, and edge-case tests.

- [ ] **Step 1: Update `src/syntax/mod.rs` once**

```rust
pub mod cst;
pub mod expr;
pub mod imports;
pub mod lower;
pub mod parse;
pub mod recovery;
pub mod syntax_kind;
pub mod types;

pub use cst::{ElementRange, ParsedSyntax, SyntaxElement, SyntaxNodeId, SyntaxTokenId, SyntaxTree};
pub use imports::{ImportEdge, ImportSummary, ModulePath, parse_import_summary};
pub use syntax_kind::{SyntaxErrorKind, SyntaxKind};
```

- [ ] **Step 2: Add `src/syntax/syntax_kind.rs`**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxKind {
    Module,
    PublicItem,
    PubModifier,
    ModuleDecl,
    UseDecl,
    UseBinderList,
    UseBinder,
    ModulePath,
    DataDecl,
    LayoutDataDecl,
    ClassDecl,
    UniqueClassDecl,
    InterfaceDecl,
    ErrorDecl,
    ImageDecl,
    HostImageDecl,
    PhaseDecl,
    ImplementsClause,
    GenericParamList,
    GenericParam,
    FieldDecl,
    MethodDecl,
    ConstructorDecl,
    TestDecl,
    ParamList,
    Param,
    ReturnType,
    TypeRef,
    GenericArgList,
    Block,
    LetStmt,
    ReturnStmt,
    ExprStmt,
    MatchStmt,
    MatchArm,
    MatchPattern,
    RepeatStmt,
    ForStmt,
    DrainStmt,
    LoopStmt,
    AssertStmt,
    NameExpr,
    LiteralExpr,
    ParenExpr,
    PrefixExpr,
    BinaryExpr,
    CallExpr,
    ArgList,
    Arg,
    NamedArg,
    FieldExpr,
    IndexExpr,
    TryExpr,
    ReduceExpr,
    ScanExpr,
    RecoveryNode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxErrorKind {
    ExpectedItem,
    ExpectedIdentifier,
    ExpectedImportBinderList,
    ExpectedFrom,
    ExpectedModulePath,
    InvalidImportBinder,
    ExpectedType,
    ExpectedExpression,
    ExpectedReturnAfterElse,
    ExpectedMatchArm,
    ExpectedAssertKind,
    ExpectedToken,
    UnexpectedToken,
    MissingCloseDelimiter,
}
```

- [ ] **Step 3: Add CST storage to `src/syntax/cst.rs`**

Use `u32` ids and ranges. `SyntaxToken` stores its own span so builder checkpoints can compute wrapper spans without consulting the lexer.

```rust
use crate::diagnostic::Diagnostic;
use crate::lexer::{LexedFile, TokenKind};
use crate::source::{FileId, SourceFile, Span};

use super::syntax_kind::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxNodeId(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxTokenId(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenIndex(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriviaRange { start: u32, end: u32 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementRange { start: u32, end: u32 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Checkpoint { depth: usize, child_index: usize }

impl SyntaxNodeId { pub const fn new(raw: u32) -> Self { Self(raw) } pub const fn raw(self) -> u32 { self.0 } }
impl SyntaxTokenId { pub const fn new(raw: u32) -> Self { Self(raw) } pub const fn raw(self) -> u32 { self.0 } }
impl TokenIndex { pub const fn new(raw: u32) -> Self { Self(raw) } pub const fn raw(self) -> u32 { self.0 } }
impl TriviaRange { pub const fn new(start: u32, end: u32) -> Self { Self { start, end } } pub const fn start(self) -> u32 { self.start } pub const fn end(self) -> u32 { self.end } }
impl ElementRange { pub const fn new(start: u32, end: u32) -> Self { Self { start, end } } pub const fn start(self) -> u32 { self.start } pub const fn end(self) -> u32 { self.end } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxElement {
    Node(SyntaxNodeId),
    Token(SyntaxTokenId),
    Error(SyntaxErrorNode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxErrorNode { kind: SyntaxErrorKind, span: Span }

impl SyntaxErrorNode {
    pub const fn new(kind: SyntaxErrorKind, span: Span) -> Self { Self { kind, span } }
    pub const fn kind(self) -> SyntaxErrorKind { self.kind }
    pub const fn span(self) -> Span { self.span }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxToken {
    token: TokenIndex,
    span: Span,
    leading_trivia: TriviaRange,
    trailing_trivia: TriviaRange,
}

impl SyntaxToken {
    pub const fn new(token: TokenIndex, span: Span, leading_trivia: TriviaRange, trailing_trivia: TriviaRange) -> Self {
        Self { token, span, leading_trivia, trailing_trivia }
    }
    pub const fn token(self) -> TokenIndex { self.token }
    pub const fn span(self) -> Span { self.span }
    pub const fn leading_trivia(self) -> TriviaRange { self.leading_trivia }
    pub const fn trailing_trivia(self) -> TriviaRange { self.trailing_trivia }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    kind: SyntaxKind,
    span: Span,
    children: ElementRange,
}

impl SyntaxNode {
    pub const fn new(kind: SyntaxKind, span: Span, children: ElementRange) -> Self { Self { kind, span, children } }
    pub const fn kind(&self) -> SyntaxKind { self.kind }
    pub const fn span(&self) -> Span { self.span }
    pub const fn children(&self) -> ElementRange { self.children }
}

#[derive(Debug)]
pub struct SyntaxTree {
    file_id: FileId,
    root: SyntaxNodeId,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    tokens: Vec<SyntaxToken>,
}

impl SyntaxTree {
    pub fn new(file_id: FileId, root: SyntaxNodeId, nodes: Vec<SyntaxNode>, elements: Vec<SyntaxElement>, tokens: Vec<SyntaxToken>) -> Self {
        Self { file_id, root, nodes, elements, tokens }
    }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn root(&self) -> SyntaxNodeId { self.root }
    pub fn node(&self, id: SyntaxNodeId) -> &SyntaxNode { &self.nodes[id.raw() as usize] }
    pub fn elements(&self, range: ElementRange) -> &[SyntaxElement] { &self.elements[range.start() as usize..range.end() as usize] }
    pub fn token(&self, id: SyntaxTokenId) -> SyntaxToken { self.tokens[id.raw() as usize] }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn token_count(&self) -> usize { self.tokens.len() }
}

#[derive(Debug)]
pub struct ParsedSyntax {
    file_id: FileId,
    tree: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
}

impl ParsedSyntax {
    pub fn new(file_id: FileId, tree: SyntaxTree, diagnostics: Vec<Diagnostic>) -> Self { Self { file_id, tree, diagnostics } }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn tree(&self) -> &SyntaxTree { &self.tree }
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }
}
```

- [ ] **Step 4: Add checkpoint-capable builder to `src/syntax/cst.rs`**

```rust
#[derive(Debug)]
struct NodeFrame {
    kind: SyntaxKind,
    anchor: Span,
    children: Vec<SyntaxElement>,
}

#[derive(Debug)]
pub struct SyntaxTreeBuilder {
    file_id: FileId,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    tokens: Vec<SyntaxToken>,
    stack: Vec<NodeFrame>,
    root: Option<SyntaxNodeId>,
}

impl SyntaxTreeBuilder {
    pub fn new(file_id: FileId) -> Self {
        Self { file_id, nodes: Vec::new(), elements: Vec::new(), tokens: Vec::new(), stack: Vec::new(), root: None }
    }

    pub fn start_node(&mut self, kind: SyntaxKind, anchor: Span) {
        self.stack.push(NodeFrame { kind, anchor, children: Vec::new() });
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let frame = self.stack.last().expect("checkpoint requires an open node");
        Checkpoint { depth: self.stack.len(), child_index: frame.children.len() }
    }

    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind, anchor: Span) {
        assert_eq!(checkpoint.depth, self.stack.len(), "checkpoint depth must match current node");
        let parent = self.stack.last_mut().expect("checkpoint requires parent frame");
        let children = parent.children.split_off(checkpoint.child_index);
        self.stack.push(NodeFrame { kind, anchor, children });
    }

    pub fn token(&mut self, token: SyntaxToken) {
        let id = SyntaxTokenId::new(self.tokens.len() as u32);
        self.tokens.push(token);
        self.push_child(SyntaxElement::Token(id));
    }

    pub fn error(&mut self, kind: SyntaxErrorKind, span: Span) {
        self.push_child(SyntaxElement::Error(SyntaxErrorNode::new(kind, span)));
    }

    pub fn finish_node(&mut self) -> SyntaxNodeId {
        let frame = self.stack.pop().expect("finish_node requires open node");
        let span = self.children_span(&frame.children).unwrap_or(frame.anchor);
        let start = self.elements.len() as u32;
        self.elements.extend(frame.children);
        let end = self.elements.len() as u32;
        let id = SyntaxNodeId::new(self.nodes.len() as u32);
        self.nodes.push(SyntaxNode::new(frame.kind, span, ElementRange::new(start, end)));
        if let Some(parent) = self.stack.last_mut() {
            parent.children.push(SyntaxElement::Node(id));
        } else {
            self.root = Some(id);
        }
        id
    }

    pub fn finish(self) -> SyntaxTree {
        SyntaxTree::new(self.file_id, self.root.expect("syntax tree root must be finished"), self.nodes, self.elements, self.tokens)
    }

    fn push_child(&mut self, element: SyntaxElement) {
        self.stack.last_mut().expect("syntax child requires open node").children.push(element);
    }

    fn children_span(&self, children: &[SyntaxElement]) -> Option<Span> {
        let first = children.first().and_then(|child| self.element_span(*child))?;
        let last = children.last().and_then(|child| self.element_span(*child))?;
        Some(Span::new(self.file_id, first.start(), last.end()))
    }

    fn element_span(&self, element: SyntaxElement) -> Option<Span> {
        match element {
            SyntaxElement::Node(id) => Some(self.nodes[id.raw() as usize].span()),
            SyntaxElement::Token(id) => Some(self.tokens[id.raw() as usize].span()),
            SyntaxElement::Error(error) => Some(error.span()),
        }
    }
}
```

- [ ] **Step 5: Add source reconstruction helper**

```rust
impl SyntaxTree {
    pub fn source_text(&self, lexed: &LexedFile, source: &SourceFile) -> String {
        let mut text = String::new();
        self.push_node_text(self.root, lexed, source, &mut text);
        text
    }

    fn push_node_text(&self, node: SyntaxNodeId, lexed: &LexedFile, source: &SourceFile, out: &mut String) {
        for element in self.elements(self.node(node).children()) {
            match *element {
                SyntaxElement::Node(child) => self.push_node_text(child, lexed, source, out),
                SyntaxElement::Token(id) => {
                    let token = self.token(id);
                    push_trivia(out, lexed, source, token.leading_trivia());
                    let raw = lexed.tokens()[token.token().raw() as usize];
                    if raw.kind() != TokenKind::Eof {
                        push_span(out, source, raw.span());
                    }
                    push_trivia(out, lexed, source, token.trailing_trivia());
                }
                SyntaxElement::Error(_) => {}
            }
        }
    }
}

fn push_trivia(out: &mut String, lexed: &LexedFile, source: &SourceFile, range: TriviaRange) {
    for index in range.start()..range.end() {
        push_span(out, source, lexed.trivia()[index as usize].span());
    }
}

fn push_span(out: &mut String, source: &SourceFile, span: Span) {
    out.push_str(&source.text()[span.start() as usize..span.end() as usize]);
}
```

- [ ] **Step 6: Add CST builder tests**

Add tests for nested nodes, checkpoints, error ordering, and empty-node anchoring.

```rust
#[test]
fn checkpoint_wraps_existing_child() {
    let file_id = FileId::new(0);
    let anchor = Span::new(file_id, 0, 0);
    let mut builder = SyntaxTreeBuilder::new(file_id);
    builder.start_node(SyntaxKind::Module, anchor);
    let checkpoint = builder.checkpoint();
    builder.token(SyntaxToken::new(TokenIndex::new(0), Span::new(file_id, 0, 3), TriviaRange::new(0, 0), TriviaRange::new(0, 0)));
    builder.start_node_at(checkpoint, SyntaxKind::FieldExpr, anchor);
    let field = builder.finish_node();
    let root = builder.finish_node();
    let tree = builder.finish();

    assert_eq!(tree.node(field).kind(), SyntaxKind::FieldExpr);
    assert!(matches!(tree.elements(tree.node(root).children())[0], SyntaxElement::Node(id) if id == field));
}

#[test]
fn empty_node_uses_anchor_span() {
    let file_id = FileId::new(0);
    let anchor = Span::new(file_id, 8, 8);
    let mut builder = SyntaxTreeBuilder::new(file_id);
    builder.start_node(SyntaxKind::Module, anchor);
    let root = builder.finish_node();
    let tree = builder.finish();

    assert_eq!(tree.node(root).span(), anchor);
}

#[test]
fn error_element_preserves_order_and_span() {
    let file_id = FileId::new(0);
    let mut builder = SyntaxTreeBuilder::new(file_id);
    builder.start_node(SyntaxKind::Module, Span::new(file_id, 0, 0));
    builder.token(SyntaxToken::new(TokenIndex::new(0), Span::new(file_id, 0, 1), TriviaRange::new(0, 0), TriviaRange::new(0, 0)));
    builder.error(SyntaxErrorKind::ExpectedIdentifier, Span::new(file_id, 1, 1));
    let root = builder.finish_node();
    let tree = builder.finish();
    let children = tree.elements(tree.node(root).children());

    assert!(matches!(children[0], SyntaxElement::Token(_)));
    assert!(matches!(children[1], SyntaxElement::Error(error) if error.span() == Span::new(file_id, 1, 1)));
}
```

- [ ] **Step 7: Verify**

```bash
cargo test syntax::cst
./scripts/quality-gate.sh
```

- [ ] **Step 8: Commit**

```bash
git add src/syntax/mod.rs src/syntax/syntax_kind.rs src/syntax/cst.rs src/syntax/parse.rs src/syntax/types.rs src/syntax/expr.rs src/syntax/recovery.rs src/syntax/lower.rs
git commit -m "feat: add CST parser foundation -Codex Automated"
```

**Acceptance Criteria:**

- All syntax modules exist and compile.
- CST storage exposes stable ids, ranges, nodes, token elements, error nodes, and `ParsedSyntax`.
- Builder checkpoints wrap prior elements so postfix/binary expression parsers can preserve structural ownership.
- Empty nodes anchor at the provided parser cursor span, not `0..0`.
- Error nodes preserve source-order placement and span.

---

### Task 2: Add Lexer Range Punctuation

**Files:**
- Modify: `src/lexer/kind.rs`
- Modify: `src/lexer/lex.rs`

**Description:** Add lexer support for `..` and `..=` and verify ordinary dotted module paths still produce single-dot tokens.

- [ ] **Step 1: Add punctuation variants**

Insert `DotDot` and `DotDotEq` immediately after `Dot`; do not reorder existing variants.

```rust
pub enum Punct {
    Amp,
    AmpAmp,
    Arrow,
    Bang,
    BangEq,
    Caret,
    Colon,
    Comma,
    Dot,
    DotDot,
    DotDotEq,
    Eq,
    EqEq,
    FatArrow,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Minus,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Percent,
    Pipe,
    PipePipe,
    Plus,
    Semicolon,
    Slash,
    Star,
    Tilde,
}
```

- [ ] **Step 2: Scan longest dot punctuations first**

```rust
[b'.', b'.', b'=', ..] => (Punct::DotDotEq, 3),
[b'.', b'.', ..] => (Punct::DotDot, 2),
[b'.', ..] => (Punct::Dot, 1),
```

- [ ] **Step 3: Add lexer tests**

```rust
#[test]
fn lexes_range_punctuation() {
    let lexed = lex("2..=15\n0..count");
    let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert!(kinds.contains(&TokenKind::Punct(Punct::DotDotEq)));
    assert!(kinds.contains(&TokenKind::Punct(Punct::DotDot)));
}

#[test]
fn lexes_dotted_module_path_without_range_tokens() {
    let lexed = lex("from app.console");
    let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert!(kinds.contains(&TokenKind::Punct(Punct::Dot)));
    assert!(!kinds.contains(&TokenKind::Punct(Punct::DotDot)));
    assert!(!kinds.contains(&TokenKind::Punct(Punct::DotDotEq)));
}
```

- [ ] **Step 4: Verify**

```bash
cargo test lexer::lex::tests::lexes_range_punctuation
cargo test lexer::lex::tests::lexes_dotted_module_path_without_range_tokens
./scripts/quality-gate.sh
```

- [ ] **Step 5: Commit**

```bash
git add src/lexer/kind.rs src/lexer/lex.rs
git commit -m "feat: lex range punctuation -Codex Automated"
```

**Acceptance Criteria:**

- `2..=15` lexes with `Punct::DotDotEq`.
- `0..count` lexes with `Punct::DotDot`.
- `app.console` still lexes with `Punct::Dot`.

---

## Phase 2: Parser Core And Imports

### Task 3: Add Parser Core, Cursor Helpers, Diagnostics, And Trivia Attachment

**Files:**
- Modify: `src/syntax/mod.rs`
- Modify: `src/syntax/parse.rs`

**Description:** Add `parse_file`, `Parser`, shared expectations, lookahead, EOF-pinned bumping, parser diagnostics, contextual keyword checks, close-delimiter diagnostics, and trivia attachment. These helpers are prerequisites for type, expression, declaration, and statement tasks.

- [ ] **Step 1: Add public parse re-export**

```rust
pub use parse::parse_file;
```

- [ ] **Step 2: Add parser core**

```rust
use crate::diagnostic::Diagnostic;
use crate::lexer::{Keyword, LexedFile, Punct, Token, TokenKind, TriviaKind};
use crate::source::{SourceFile, Span};

use super::cst::{ParsedSyntax, SyntaxToken, SyntaxTreeBuilder, TokenIndex, TriviaRange};
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

pub fn parse_file(lexed: &LexedFile, source: &SourceFile) -> ParsedSyntax {
    Parser::new(lexed, source).parse_module()
}

pub(crate) struct Parser<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) lexed: &'a LexedFile,
    pub(crate) tokens: &'a [Token],
    pub(crate) token_index: usize,
    pub(crate) trivia_index: usize,
    eof_emitted: bool,
    pub(crate) builder: SyntaxTreeBuilder,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(lexed: &'a LexedFile, source: &'a SourceFile) -> Self {
        Self {
            source,
            lexed,
            tokens: lexed.tokens(),
            token_index: 0,
            trivia_index: 0,
            eof_emitted: false,
            builder: SyntaxTreeBuilder::new(lexed.file_id()),
            diagnostics: Vec::new(),
        }
    }

    fn parse_module(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        while !self.at(TokenKind::Eof) {
            self.parse_item();
        }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }

    pub(crate) fn parse_item(&mut self) {
        self.parse_error_item();
    }

    pub(crate) fn parse_error_item(&mut self) {
        let span = self.peek().span();
        self.start_node(SyntaxKind::RecoveryNode);
        self.diagnostic(span, "expected item");
        self.builder.error(SyntaxErrorKind::ExpectedItem, span);
        self.bump();
        self.finish_node();
    }
}
```

- [ ] **Step 3: Add shared cursor and builder wrappers**

```rust
impl<'a> Parser<'a> {
    pub(crate) fn peek(&self) -> Token { self.tokens[self.token_index] }
    pub(crate) fn peek_n(&self, offset: usize) -> Token { self.tokens[(self.token_index + offset).min(self.tokens.len() - 1)] }
    pub(crate) fn at(&self, kind: TokenKind) -> bool { self.peek().kind() == kind }
    pub(crate) fn checkpoint(&self) -> super::cst::Checkpoint { self.builder.checkpoint() }
    pub(crate) fn start_node(&mut self, kind: SyntaxKind) { self.builder.start_node(kind, self.peek().span()); }
    pub(crate) fn start_node_at(&mut self, checkpoint: super::cst::Checkpoint, kind: SyntaxKind) { self.builder.start_node_at(checkpoint, kind, self.peek().span()); }
    pub(crate) fn finish_node(&mut self) { self.builder.finish_node(); }

    pub(crate) fn bump(&mut self) {
        let token_index = self.token_index;
        let token = self.tokens[token_index];
        if token.kind() == TokenKind::Eof && self.eof_emitted {
            return;
        }
        let leading = self.take_leading_trivia();
        let trailing = self.take_trailing_trivia(token);
        self.builder.token(SyntaxToken::new(TokenIndex::new(token_index as u32), token.span(), leading, trailing));
        if token.kind() == TokenKind::Eof {
            self.eof_emitted = true;
        } else {
            self.token_index += 1;
        }
    }
}
```

- [ ] **Step 4: Add trivia attachment policy**

Policy: when bumping a token, leading trivia consumes every unclaimed trivia item before the token. Trailing trivia then consumes trivia after the token through the first newline, including comments and whitespace before that newline. Indentation and blank lines after the trailing newline become leading trivia for the next token. This preserves exact reconstruction for file-leading whitespace, indented blocks, and comments.

```rust
impl<'a> Parser<'a> {
    fn take_leading_trivia(&mut self) -> TriviaRange {
        let start = self.trivia_index as u32;
        let token_start = self.peek().span().start();
        while self.lexed.trivia().get(self.trivia_index).is_some_and(|trivia| {
            trivia.span().end() <= token_start
        }) {
            self.trivia_index += 1;
        }
        TriviaRange::new(start, self.trivia_index as u32)
    }

    fn take_trailing_trivia(&mut self, token: Token) -> TriviaRange {
        let start = self.trivia_index as u32;
        while let Some(trivia) = self.lexed.trivia().get(self.trivia_index) {
            if trivia.span().start() < token.span().end() {
                break;
            }
            if trivia.kind() == TriviaKind::Newline {
                self.trivia_index += 1;
                break;
            }
            self.trivia_index += 1;
        }
        TriviaRange::new(start, self.trivia_index as u32)
    }
}
```

- [ ] **Step 5: Add shared expectations and diagnostics**

```rust
impl<'a> Parser<'a> {
    pub(crate) fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        if self.peek().kind() == TokenKind::Keyword(keyword) { self.bump(); true } else { false }
    }

    pub(crate) fn eat_punct(&mut self, punct: Punct) -> bool {
        if self.peek().kind() == TokenKind::Punct(punct) { self.bump(); true } else { false }
    }

    pub(crate) fn expect_keyword(&mut self, keyword: Keyword, label: &'static str) -> bool {
        if self.eat_keyword(keyword) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> bool {
        if self.peek().kind() == TokenKind::Identifier {
            self.bump();
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedIdentifier, "expected identifier");
            false
        }
    }

    pub(crate) fn expect_contextual_identifier(&mut self, text: &'static str, label: &'static str) -> bool {
        if self.peek().kind() == TokenKind::Identifier && self.token_text(self.peek()) == text {
            self.bump();
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_punct(&mut self, punct: Punct, label: &'static str) -> bool {
        if self.eat_punct(punct) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_close_punct(&mut self, punct: Punct, label: &'static str) -> bool {
        if self.eat_punct(punct) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::MissingCloseDelimiter, label);
            false
        }
    }

    pub(crate) fn error_at_current(&mut self, kind: SyntaxErrorKind, message: &'static str) {
        let span = self.peek().span();
        self.diagnostic(span, message);
        self.builder.error(kind, span);
    }

    pub(crate) fn diagnostic(&mut self, span: Span, message: &'static str) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }

    pub(crate) fn token_text(&self, token: Token) -> &str {
        &self.source.text()[token.span().start() as usize..token.span().end() as usize]
    }
}
```

- [ ] **Step 6: Add reconstruction test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    #[test]
    fn parse_file_reconstructs_source_with_attached_trivia_and_indentation() {
        let text = "  use { Console } from app.console // trailing\n\n    module app.root\n";
        let source = SourceFile::new(FileId::new(0), PathBuf::from("root.wrela"), text.to_string());
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);

        assert_eq!(parsed.tree().source_text(&lexed, &source), text);
    }
}
```

- [ ] **Step 7: Verify**

```bash
cargo test syntax::parse
./scripts/quality-gate.sh
```

- [ ] **Step 8: Commit**

```bash
git add src/syntax/mod.rs src/syntax/parse.rs
git commit -m "feat: add parser core -Codex Automated"
```

**Acceptance Criteria:**

- `parse_file` exists and returns `ParsedSyntax`.
- `syntax::parse_file` is re-exported for integration tests through the public crate API.
- Shared helpers `expect_identifier`, `expect_punct`, `expect_close_punct`, `expect_keyword`, `expect_contextual_identifier`, `error_at_current`, `peek_n`, and EOF-pinned `bump` exist before type/expression tasks.
- Parser diagnostics use catalog messages.
- Source reconstruction preserves file-leading whitespace, trailing comments, blank lines, and indentation.

---

### Task 3.5: Add Parser Integration Test Harness

**Files:**
- Create: `tests/parser.rs`
- Create: `fixtures/parser/parser_harness_smoke.wrela`

**Description:** Add shared integration-test helpers that compile immediately after Task 3. Helpers that require future parser entry points (`parse_type_text`, `parse_expr_text`, and `parse_block_text`) are added by the tasks that introduce those entry points.

- [ ] **Step 1: Create empty smoke fixture**

`fixtures/parser/parser_harness_smoke.wrela`:

```wrela
```

- [ ] **Step 2: Add harness helpers**

```rust
use std::path::PathBuf;

use wrela::diagnostic::has_errors;
use wrela::lexer::lex_file;
use wrela::source::{FileId, SourceFile};
use wrela::syntax::{parse_file, ElementRange, ParsedSyntax, SyntaxElement, SyntaxKind, SyntaxTree};

fn parser_fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/parser").join(rel)
}

fn source_from_text(name: &str, text: &str) -> SourceFile {
    SourceFile::new(FileId::new(0), PathBuf::from(name), text.to_string())
}

fn parse_text(text: &str) -> ParsedSyntax {
    let source = source_from_text("inline.wrela", text);
    let lexed = lex_file(&source);
    parse_file(&lexed, &source)
}

fn parse_fixture(rel: &str) -> ParsedSyntax {
    let path = parser_fixture(rel);
    let text = std::fs::read_to_string(&path).expect("parser fixture is readable");
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);
    parse_file(&lexed, &source)
}

fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(tree: &SyntaxTree, node: wrela::syntax::SyntaxNodeId, kind: SyntaxKind) -> bool {
        if tree.node(node).kind() == kind {
            return true;
        }
        tree.elements(tree.node(node).children()).iter().any(|element| {
            matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind))
        })
    }
    walk(tree, tree.root(), kind)
}

fn count_nodes(tree: &SyntaxTree, kind: SyntaxKind) -> usize {
    fn walk(tree: &SyntaxTree, node: wrela::syntax::SyntaxNodeId, kind: SyntaxKind) -> usize {
        let self_count = usize::from(tree.node(node).kind() == kind);
        self_count
            + tree
                .elements(tree.node(node).children())
                .iter()
                .map(|element| match *element {
                    SyntaxElement::Node(child) => walk(tree, child, kind),
                    _ => 0,
                })
                .sum::<usize>()
    }
    walk(tree, tree.root(), kind)
}

fn child_kinds(tree: &SyntaxTree, range: ElementRange) -> Vec<SyntaxKind> {
    tree.elements(range)
        .iter()
        .filter_map(|element| match *element {
            SyntaxElement::Node(id) => Some(tree.node(id).kind()),
            _ => None,
        })
        .collect()
}
```

- [ ] **Step 3: Add harness smoke test**

```rust
#[test]
fn parser_harness_smoke_parses_empty_fixture() {
    let parsed = parse_fixture("parser_harness_smoke.wrela");
    let inline = parse_text("");

    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::Module));
    assert_eq!(count_nodes(inline.tree(), SyntaxKind::Module), 1);
    let root = inline.tree().node(inline.tree().root());
    assert!(child_kinds(inline.tree(), root.children()).is_empty());
}
```

- [ ] **Step 4: Verify**

```bash
cargo test --test parser parser_harness_smoke_parses_empty_fixture
./scripts/quality-gate.sh
```

- [ ] **Step 5: Commit**

```bash
git add tests/parser.rs fixtures/parser/parser_harness_smoke.wrela
git commit -m "test: add parser integration harness -Codex Automated"
```

**Acceptance Criteria:**

- `tests/parser.rs` compiles immediately after Task 3.
- Shared helpers `parser_fixture`, `parse_text`, `parse_fixture`, `tree_contains`, `count_nodes`, `child_kinds`, and `source_from_text` are defined once.
- No helper in this task calls `parse_type_ref`, `parse_expr`, or `parse_block`.

---

### Task 4: Parse Module And Import Declarations

**Files:**
- Modify: `src/syntax/parse.rs`
- Create: `fixtures/parser/imports/root.wrela`
- Create: `fixtures/parser/imports/app/console.wrela`
- Modify: `tests/parser.rs`

**Description:** Parse `module` and v1 `use` declarations, including exact alias and wildcard rejection, missing binder-list diagnostics, and agreement with the existing discovery import parser.

- [ ] **Step 1: Add fixtures**

`fixtures/parser/imports/root.wrela`:

```wrela
use { Console } from app.console
module app.root
```

`fixtures/parser/imports/app/console.wrela`:

```wrela
data Console {}
```

- [ ] **Step 2: Implement import grammar**

```rust
fn parse_item(&mut self) {
    match self.peek().kind() {
        TokenKind::Keyword(Keyword::Module) => self.parse_module_decl(),
        TokenKind::Keyword(Keyword::Use) => self.parse_use_decl(),
        _ => self.parse_error_item(),
    }
}

fn parse_module_decl(&mut self) {
    self.start_node(SyntaxKind::ModuleDecl);
    self.bump();
    self.parse_module_path();
    self.finish_node();
}

fn parse_use_decl(&mut self) {
    self.start_node(SyntaxKind::UseDecl);
    self.bump();
    if self.eat_punct(Punct::OpenBrace) {
        self.start_node(SyntaxKind::UseBinderList);
        if self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) {
            self.parse_use_binder();
            while self.eat_punct(Punct::Comma) {
                if self.peek().kind() == TokenKind::Punct(Punct::CloseBrace) { break; }
                self.parse_use_binder();
            }
        }
        self.finish_node();
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    } else {
        let span = self.peek().span();
        self.diagnostic(span, "expected import binder list");
        self.builder.error(SyntaxErrorKind::ExpectedImportBinderList, span);
    }
    if self.eat_keyword(Keyword::From) {
        self.parse_module_path_after_from();
    } else {
        let span = self.peek().span();
        self.diagnostic(span, "expected from in use import");
        self.builder.error(SyntaxErrorKind::ExpectedFrom, span);
    }
    self.finish_node();
}

fn parse_use_binder(&mut self) {
    self.start_node(SyntaxKind::UseBinder);
    if self.peek().kind() == TokenKind::Punct(Punct::Star) {
        let span = self.peek().span();
        self.diagnostic(span, "wildcard imports are not supported in v1");
        self.builder.error(SyntaxErrorKind::InvalidImportBinder, span);
        self.bump();
    } else if self.expect_identifier() && self.eat_keyword(Keyword::As) {
        let span = self.peek().span();
        self.diagnostic(span, "import aliases are not supported in v1");
        self.builder.error(SyntaxErrorKind::InvalidImportBinder, span);
        self.expect_identifier();
    }
    self.finish_node();
}

fn parse_module_path_after_from(&mut self) {
    if self.peek().kind() != TokenKind::Identifier {
        let span = self.peek().span();
        self.diagnostic(span, "expected module path after from");
        self.builder.error(SyntaxErrorKind::ExpectedModulePath, span);
        return;
    }
    self.parse_module_path();
}

fn parse_module_path(&mut self) {
    self.start_node(SyntaxKind::ModulePath);
    self.expect_identifier();
    while self.eat_punct(Punct::Dot) {
        self.expect_identifier();
    }
    self.finish_node();
}
```

- [ ] **Step 3: Add tests**

```rust
#[test]
fn parses_imports_after_discovery() {
    let result = discover_from_root(parser_fixture("imports/root.wrela"));
    assert!(!has_errors(result.diagnostics()));
    let source = result.source_map().files().first().unwrap();
    let lexed = result.lexed_files().first().unwrap();
    let parsed = parse_file(lexed, source);

    assert!(!has_errors(parsed.diagnostics()));
    assert_eq!(parsed.tree().source_text(lexed, source), source.text());
}

#[test]
fn rejects_import_aliases_and_wildcards_in_v1() {
    for (text, expected) in [
        ("use { Console as Terminal } from app.console\n", "import aliases are not supported in v1"),
        ("use { * } from app.console\n", "wildcard imports are not supported in v1"),
    ] {
        let source = SourceFile::new(FileId::new(0), PathBuf::from("bad_import.wrela"), text.to_string());
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        assert!(parsed.diagnostics().iter().any(|diagnostic| diagnostic.message() == expected));
    }
}

#[test]
fn cst_import_path_agrees_with_discovery_import_parser() {
    let text = "use { Console } from app.console\n";
    let source = SourceFile::new(FileId::new(0), PathBuf::from("root.wrela"), text.to_string());
    let lexed = lex_file(&source);
    let summary = parse_import_summary(&lexed, &source);
    let parsed = parse_file(&lexed, &source);

    assert_eq!(summary.imports()[0].module().as_dotted(), "app.console");
    assert!(!has_errors(parsed.diagnostics()));
}
```

- [ ] **Step 4: Verify**

```bash
cargo test --test parser parses_imports_after_discovery
cargo test --test parser rejects_import_aliases_and_wildcards_in_v1
cargo test --test parser cst_import_path_agrees_with_discovery_import_parser
./scripts/quality-gate.sh
```

- [ ] **Step 5: Commit**

```bash
git add src/syntax/parse.rs tests/parser.rs fixtures/parser/imports/root.wrela fixtures/parser/imports/app/console.wrela
git commit -m "feat: parse module and use declarations -Codex Automated"
```

**Acceptance Criteria:**

- Missing binder list emits `expected import binder list`.
- Alias imports emit `import aliases are not supported in v1`.
- Wildcard imports emit `wildcard imports are not supported in v1`.
- Valid import paths still agree with `parse_import_summary`.

---

## Phase 3: Types, Expressions, And Blocks

### Task 5: Parse Types And Generic Lists

**Files:**
- Modify: `src/syntax/types.rs`

**Description:** Parse type references, access-qualified types, generic params, and generic args. This task uses helpers from Task 3 and does not require declaration parsing.

- [ ] **Step 1: Add failing type parser unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::has_errors;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn parse_type_text(text: &str) -> ParsedSyntax {
        let source = SourceFile::new(FileId::new(0), PathBuf::from("type.wrela"), text.to_string());
        let lexed = lex_file(&source);
        Parser::new(&lexed, &source).parse_type_for_test()
    }

    #[test]
    fn parses_generic_type_arguments() {
        let parsed = parse_type_text("Table[Session, 4096]");
        assert!(!has_errors(parsed.diagnostics()));
    }

    #[test]
    fn parses_access_qualified_type() {
        let parsed = parse_type_text("unique MacOSHost");
        assert!(!has_errors(parsed.diagnostics()));
    }
}
```

Run: `cargo test syntax::types::tests::parses_generic_type_arguments`

Expected: FAIL because `parse_type_ref` and `parse_type_for_test` do not exist.

- [ ] **Step 2: Implement type parser helpers**

```rust
use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};
#[cfg(test)]
use super::cst::ParsedSyntax;

impl<'a> Parser<'a> {
    pub(crate) fn parse_type_ref(&mut self) {
        self.start_node(SyntaxKind::TypeRef);
        if matches!(self.peek().kind(), TokenKind::Keyword(Keyword::Read) | TokenKind::Keyword(Keyword::Mut) | TokenKind::Keyword(Keyword::Own) | TokenKind::Keyword(Keyword::Unique)) {
            self.bump();
        }
        if self.expect_identifier() {
            while self.eat_punct(Punct::Dot) { self.expect_identifier(); }
            if self.eat_punct(Punct::OpenBracket) {
                self.start_node(SyntaxKind::GenericArgList);
                if self.peek().kind() != TokenKind::Punct(Punct::CloseBracket) {
                    self.parse_generic_arg();
                    while self.eat_punct(Punct::Comma) {
                        if self.peek().kind() == TokenKind::Punct(Punct::CloseBracket) { break; }
                        self.parse_generic_arg();
                    }
                }
                self.finish_node();
                self.expect_close_punct(Punct::CloseBracket, "expected ']'");
            }
        } else {
            let span = self.peek().span();
            self.diagnostic(span, "expected type");
            self.builder.error(SyntaxErrorKind::ExpectedType, span);
        }
        self.finish_node();
    }

    pub(crate) fn parse_generic_param_list(&mut self) {
        if !self.eat_punct(Punct::Less) { return; }
        self.start_node(SyntaxKind::GenericParamList);
        self.parse_generic_param();
        while self.eat_punct(Punct::Comma) {
            if self.peek().kind() == TokenKind::Punct(Punct::Greater) { break; }
            self.parse_generic_param();
        }
        self.finish_node();
        self.expect_punct(Punct::Greater, "expected '>'");
    }

    fn parse_generic_param(&mut self) {
        self.start_node(SyntaxKind::GenericParam);
        self.expect_identifier();
        if self.eat_punct(Punct::Colon) { self.parse_type_ref(); }
        self.finish_node();
    }

    fn parse_generic_arg(&mut self) {
        if self.peek().kind() == TokenKind::IntLiteral { self.bump(); } else { self.parse_type_ref(); }
    }
}
```

- [ ] **Step 3: Add test-only type entry in `src/syntax/types.rs`**

```rust
#[cfg(test)]
impl<'a> Parser<'a> {
    pub(crate) fn parse_type_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_type_ref();
        while !self.at(TokenKind::Eof) { self.bump(); }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }
}
```

- [ ] **Step 4: Verify**

```bash
cargo test syntax::types
./scripts/quality-gate.sh
```

- [ ] **Step 5: Commit**

```bash
git add src/syntax/types.rs
git commit -m "feat: parse type references -Codex Automated"
```

**Acceptance Criteria:**

- Access-qualified types parse.
- Generic type args and const integer args parse.
- Generic param lists parse.

---

### Task 6: Parse Expressions With CST Checkpoints

**Files:**
- Modify: `src/syntax/expr.rs`

**Description:** Add Pratt expression parsing without redundant outer `Expr` wrappers. Postfix and binary expressions use builder checkpoints so the CST preserves ownership of left-hand expressions.

- [ ] **Step 1: Add failing expression unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::has_errors;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn parse_expr_text(text: &str) -> ParsedSyntax {
        let source = SourceFile::new(FileId::new(0), PathBuf::from("expr.wrela"), text.to_string());
        let lexed = lex_file(&source);
        Parser::new(&lexed, &source).parse_expr_for_test()
    }

    #[test]
    fn postfix_wraps_receiver_with_checkpoints() {
        let parsed = parse_expr_text("foo.bar(x)");

        assert!(!has_errors(parsed.diagnostics()));
        assert!(parsed.tree().node_count() > 0);
        assert!(tree_contains(parsed.tree(), SyntaxKind::CallExpr));
        assert!(tree_contains(parsed.tree(), SyntaxKind::FieldExpr));
    }

    #[test]
    fn named_arg_requires_bare_identifier() {
        let parsed = parse_expr_text("f(name = 1, a.b)");
        assert!(!has_errors(parsed.diagnostics()));
        assert!(tree_contains(parsed.tree(), SyntaxKind::NamedArg));
        assert!(tree_contains(parsed.tree(), SyntaxKind::Arg));
    }

    #[test]
    fn try_else_requires_return() {
        let parsed = parse_expr_text("try load() else recover()");
        assert!(parsed.diagnostics().iter().any(|diagnostic| diagnostic.message() == "expected return after else in try expression"));
    }

    fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
        fn walk(tree: &SyntaxTree, node: SyntaxNodeId, kind: SyntaxKind) -> bool {
            tree.node(node).kind() == kind
                || tree.elements(tree.node(node).children()).iter().any(|element| {
                    matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind))
                })
        }
        walk(tree, tree.root(), kind)
    }
}
```

Run: `cargo test syntax::expr::tests::postfix_wraps_receiver_with_checkpoints`

Expected: FAIL because `parse_expr` and `parse_expr_for_test` do not exist.

- [ ] **Step 2: Implement expression parser**

```rust
use crate::lexer::{Keyword, Punct, TokenKind};

use super::cst::Checkpoint;
#[cfg(test)]
use super::cst::{ParsedSyntax, SyntaxElement, SyntaxNodeId, SyntaxTree};
use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum BindingPower { Lowest = 0, Or = 1, And = 2, Compare = 3, Add = 4, Mul = 5, Prefix = 6, Postfix = 7 }

impl<'a> Parser<'a> {
    pub(crate) fn parse_expr(&mut self) { self.parse_expr_bp(BindingPower::Lowest); }

    pub(crate) fn parse_expr_bp(&mut self, min_bp: BindingPower) {
        let checkpoint = self.checkpoint();
        self.parse_prefix_or_atom();
        loop {
            if self.parse_postfix(checkpoint) { continue; }
            let Some((left_bp, right_bp)) = infix_binding_power(self.peek().kind()) else { break; };
            if left_bp < min_bp { break; }
            self.start_node_at(checkpoint, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_expr_bp(right_bp);
            self.finish_node();
        }
    }

    fn parse_prefix_or_atom(&mut self) {
        match self.peek().kind() {
            TokenKind::Identifier => self.parse_name_expr(),
            TokenKind::IntLiteral | TokenKind::StringLiteral => self.parse_literal_expr(),
            TokenKind::Punct(Punct::OpenParen) => self.parse_paren_expr(),
            TokenKind::Punct(Punct::Bang) | TokenKind::Punct(Punct::Minus) | TokenKind::Keyword(Keyword::Read) | TokenKind::Keyword(Keyword::Mut) | TokenKind::Keyword(Keyword::Own) => self.parse_prefix_expr(),
            TokenKind::Keyword(Keyword::Try) => self.parse_try_expr(),
            _ => self.error_at_current(SyntaxErrorKind::ExpectedExpression, "expected expression"),
        }
    }

    fn parse_name_expr(&mut self) { self.start_node(SyntaxKind::NameExpr); self.bump(); self.finish_node(); }
    fn parse_literal_expr(&mut self) { self.start_node(SyntaxKind::LiteralExpr); self.bump(); self.finish_node(); }
    fn parse_prefix_expr(&mut self) { self.start_node(SyntaxKind::PrefixExpr); self.bump(); self.parse_expr_bp(BindingPower::Prefix); self.finish_node(); }
    fn parse_paren_expr(&mut self) { self.start_node(SyntaxKind::ParenExpr); self.bump(); self.parse_expr(); self.expect_close_punct(Punct::CloseParen, "expected ')'"); self.finish_node(); }

    fn parse_try_expr(&mut self) {
        self.start_node(SyntaxKind::TryExpr);
        self.bump();
        self.parse_expr_bp(BindingPower::Prefix);
        if self.eat_keyword(Keyword::Else) {
            if self.eat_keyword(Keyword::Return) { self.parse_expr(); } else { self.error_at_current(SyntaxErrorKind::ExpectedReturnAfterElse, "expected return after else in try expression"); }
        }
        self.finish_node();
    }
}
```

- [ ] **Step 3: Implement postfix and argument parsing**

```rust
impl<'a> Parser<'a> {
    fn parse_postfix(&mut self, checkpoint: Checkpoint) -> bool {
        match self.peek().kind() {
            TokenKind::Punct(Punct::OpenParen) => {
                self.start_node_at(checkpoint, SyntaxKind::CallExpr);
                self.bump();
                self.parse_arg_list();
                self.expect_close_punct(Punct::CloseParen, "expected ')'");
                self.finish_node();
                true
            }
            TokenKind::Punct(Punct::Dot) => {
                self.start_node_at(checkpoint, SyntaxKind::FieldExpr);
                self.bump();
                self.expect_identifier();
                self.finish_node();
                true
            }
            TokenKind::Punct(Punct::OpenBracket) => {
                self.start_node_at(checkpoint, SyntaxKind::IndexExpr);
                self.bump();
                self.parse_expr();
                self.expect_close_punct(Punct::CloseBracket, "expected ']'");
                self.finish_node();
                true
            }
            _ => false,
        }
    }

    fn parse_arg_list(&mut self) {
        self.start_node(SyntaxKind::ArgList);
        while self.peek().kind() != TokenKind::Punct(Punct::CloseParen) && self.peek().kind() != TokenKind::Eof {
            if self.peek().kind() == TokenKind::Identifier && self.peek_n(1).kind() == TokenKind::Punct(Punct::Eq) {
                self.start_node(SyntaxKind::NamedArg);
                self.bump();
                self.bump();
                self.parse_expr();
                self.finish_node();
            } else {
                self.start_node(SyntaxKind::Arg);
                self.parse_expr();
                self.finish_node();
            }
            if !self.eat_punct(Punct::Comma) { break; }
        }
        self.finish_node();
    }
}
```

- [ ] **Step 4: Implement precedence table**

```rust
fn infix_binding_power(kind: TokenKind) -> Option<(BindingPower, BindingPower)> {
    match kind {
        TokenKind::Punct(Punct::PipePipe) => Some((BindingPower::Or, BindingPower::And)),
        TokenKind::Punct(Punct::AmpAmp) => Some((BindingPower::And, BindingPower::Compare)),
        TokenKind::Punct(Punct::EqEq) | TokenKind::Punct(Punct::BangEq) | TokenKind::Punct(Punct::Less) | TokenKind::Punct(Punct::LessEq) | TokenKind::Punct(Punct::Greater) | TokenKind::Punct(Punct::GreaterEq) => Some((BindingPower::Compare, BindingPower::Add)),
        TokenKind::Punct(Punct::Plus) | TokenKind::Punct(Punct::Minus) => Some((BindingPower::Add, BindingPower::Mul)),
        TokenKind::Punct(Punct::Star) | TokenKind::Punct(Punct::Slash) | TokenKind::Punct(Punct::Percent) => Some((BindingPower::Mul, BindingPower::Prefix)),
        _ => None,
    }
}
```

- [ ] **Step 5: Add test-only expression entry**

```rust
#[cfg(test)]
impl<'a> Parser<'a> {
    pub(crate) fn parse_expr_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_expr();
        while !self.at(TokenKind::Eof) { self.bump(); }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }
}
```

- [ ] **Step 6: Verify**

```bash
cargo test syntax::expr
./scripts/quality-gate.sh
```

- [ ] **Step 7: Commit**

```bash
git add src/syntax/expr.rs
git commit -m "feat: parse expressions with checkpoints -Codex Automated"
```

**Acceptance Criteria:**

- Expression CST uses concrete nodes instead of a redundant generic `Expr` wrapper.
- Postfix and binary expressions wrap their left-hand side with checkpoints.
- Named arguments require bare identifier left sides.
- `try ... else` without `return` produces the catalog diagnostic.

---

### Task 7: Parse Basic Blocks And Statement Bodies

**Files:**
- Modify: `src/syntax/parse.rs`

**Description:** Add real block parsing and basic statements before declaration bodies are introduced. This prevents raw balanced-token body parsing.

- [ ] **Step 1: Add failing block parser unit test**

Add this to the existing `#[cfg(test)] mod tests` in `src/syntax/parse.rs` from Task 3:

```rust
use crate::diagnostic::has_errors;

fn parse_block_text(text: &str) -> ParsedSyntax {
    let source = SourceFile::new(FileId::new(0), PathBuf::from("block.wrela"), text.to_string());
    let lexed = lex_file(&source);
    Parser::new(&lexed, &source).parse_block_for_test()
}

#[test]
fn parses_basic_block_statements() {
    let parsed = parse_block_text("{ let result = worker.run(input = 1) return result }");

    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::LetStmt));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ReturnStmt));
}

#[test]
fn let_requires_type_or_initializer() {
    let parsed = parse_block_text("{ let dangling }");
    assert!(parsed.diagnostics().iter().any(|diagnostic| diagnostic.message() == "expected type annotation or initializer in let statement"));
}
```

Run: `cargo test syntax::parse::tests::parses_basic_block_statements`

Expected: FAIL because `parse_block` and `parse_block_for_test` do not exist.

- [ ] **Step 2: Implement block and basic statements**

```rust
pub(crate) fn parse_block(&mut self) {
    self.start_node(SyntaxKind::Block);
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        self.parse_stmt();
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    self.finish_node();
}

pub(crate) fn parse_stmt(&mut self) {
    match self.peek().kind() {
        TokenKind::Keyword(Keyword::Let) => self.parse_let_stmt(),
        TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt(),
        _ => self.parse_expr_stmt(),
    }
}

fn parse_let_stmt(&mut self) {
    self.start_node(SyntaxKind::LetStmt);
    self.bump();
    self.expect_identifier();
    let has_type = if self.eat_punct(Punct::Colon) { self.parse_type_ref(); true } else { false };
    let has_initializer = if self.eat_punct(Punct::Eq) { self.parse_expr(); true } else { false };
    if !has_type && !has_initializer {
        self.error_at_current(SyntaxErrorKind::ExpectedToken, "expected type annotation or initializer in let statement");
    }
    self.eat_punct(Punct::Semicolon);
    self.finish_node();
}

fn parse_return_stmt(&mut self) {
    self.start_node(SyntaxKind::ReturnStmt);
    self.bump();
    if !self.at_statement_boundary() { self.parse_expr(); }
    self.eat_punct(Punct::Semicolon);
    self.finish_node();
}

fn parse_expr_stmt(&mut self) {
    self.start_node(SyntaxKind::ExprStmt);
    self.parse_expr();
    self.eat_punct(Punct::Semicolon);
    self.finish_node();
}
```

- [ ] **Step 3: Add statement boundary helper**

```rust
pub(crate) fn at_statement_boundary(&self) -> bool {
    matches!(
        self.peek().kind(),
        TokenKind::Eof
            | TokenKind::Punct(Punct::CloseBrace)
            | TokenKind::Keyword(Keyword::Let)
            | TokenKind::Keyword(Keyword::Return)
            | TokenKind::Keyword(Keyword::Match)
            | TokenKind::Keyword(Keyword::Repeat)
            | TokenKind::Keyword(Keyword::For)
            | TokenKind::Keyword(Keyword::Drain)
            | TokenKind::Keyword(Keyword::Loop)
            | TokenKind::Keyword(Keyword::Assert)
    )
}
```

- [ ] **Step 4: Add test-only block entry and local tree walker**

```rust
#[cfg(test)]
impl<'a> Parser<'a> {
    pub(crate) fn parse_block_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_block();
        while !self.at(TokenKind::Eof) { self.bump(); }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }
}

#[cfg(test)]
fn tree_contains(tree: &super::cst::SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(tree: &super::cst::SyntaxTree, node: super::cst::SyntaxNodeId, kind: SyntaxKind) -> bool {
        tree.node(node).kind() == kind
            || tree.elements(tree.node(node).children()).iter().any(|element| {
                matches!(*element, super::cst::SyntaxElement::Node(child) if walk(tree, child, kind))
            })
    }
    walk(tree, tree.root(), kind)
}
```

- [ ] **Step 5: Verify**

```bash
cargo test syntax::parse::tests::parses_basic_block_statements
cargo test syntax::parse::tests::let_requires_type_or_initializer
./scripts/quality-gate.sh
```

- [ ] **Step 6: Commit**

```bash
git add src/syntax/parse.rs
git commit -m "feat: parse basic blocks and statements -Codex Automated"
```

**Acceptance Criteria:**

- Blocks contain real statement nodes.
- There is no raw balanced-token block parser.
- Basic let, return, and expression statements parse and reconstruct source.
- `let <name>` without type or initializer emits the catalog diagnostic.

---

## Phase 4: Declarations And Control Flow

### Task 8: Parse Top-Level Declarations And Members

**Files:**
- Modify: `src/syntax/parse.rs`
- Create: `fixtures/parser/declarations-top.wrela`
- Create: `fixtures/parser/declarations-members.wrela`
- Modify: `tests/parser.rs`

**Description:** Parse every top-level declaration and member helper invoked by `parse_item`. This task includes complete helper bodies and uses `parse_block` for all bodies.

- [ ] **Step 1: Add failing declaration fixtures and tests**

`fixtures/parser/declarations-top.wrela`:

```wrela
pub data Packet {
    len: U32
    payload: Bytes[256]
}

layout C data WirePacket {
    tag: U8
    len: U16
}

interface Console {
    fn write(read self, bytes: read Bytes) -> Result
}

error IoError {
    code: U32
}

image Firmware target AArch64 {
    phase boot(host: unique MacOSHost) {
        return
    }
}

host image HostTests {
    phase run(host: unique MacOSHost) {
        return
    }
}
```

`fixtures/parser/declarations-members.wrela`:

```wrela
class ConsoleTests<C: Console> implements Console {
    device: Uart

    constructor(port: unique Port) {
        return
    }

    fn write(read self, bytes: read Bytes) -> Result {
        return ok
    }

    asm fn flush(mut self) -> None {
        return
    }

    test "writes bytes" {
        return
    }
}

unique class UartDriver {
    port: Port

    constructor(port: unique Port) {
        return
    }
}
```

```rust
#[test]
fn parses_top_level_declaration_forms() {
    let parsed = parse_fixture("declarations-top.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::PublicItem,
        SyntaxKind::DataDecl,
        SyntaxKind::LayoutDataDecl,
        SyntaxKind::InterfaceDecl,
        SyntaxKind::ErrorDecl,
        SyntaxKind::ImageDecl,
        SyntaxKind::HostImageDecl,
        SyntaxKind::PhaseDecl,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
}

#[test]
fn parses_member_declaration_forms() {
    let parsed = parse_fixture("declarations-members.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::ClassDecl,
        SyntaxKind::UniqueClassDecl,
        SyntaxKind::ImplementsClause,
        SyntaxKind::FieldDecl,
        SyntaxKind::ConstructorDecl,
        SyntaxKind::MethodDecl,
        SyntaxKind::TestDecl,
        SyntaxKind::Block,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
}
```

Run: `cargo test --test parser parses_top_level_declaration_forms`

Expected: FAIL because declaration parsers do not exist yet.

- [ ] **Step 2: Implement public item and top-level dispatch**

```rust
fn parse_item(&mut self) {
    match self.peek().kind() {
        TokenKind::Keyword(Keyword::Pub) => self.parse_pub_item(),
        TokenKind::Keyword(Keyword::Module) => self.parse_module_decl(),
        TokenKind::Keyword(Keyword::Use) => self.parse_use_decl(),
        TokenKind::Keyword(Keyword::Data) => self.parse_data_decl(),
        TokenKind::Keyword(Keyword::Layout) => self.parse_layout_data_decl(),
        TokenKind::Keyword(Keyword::Class) => self.parse_class_decl(),
        TokenKind::Keyword(Keyword::Unique) => self.parse_unique_class_decl(),
        TokenKind::Keyword(Keyword::Interface) => self.parse_interface_decl(),
        TokenKind::Keyword(Keyword::Error) => self.parse_error_decl(),
        TokenKind::Keyword(Keyword::Image) => self.parse_image_decl(),
        TokenKind::Keyword(Keyword::Host) => self.parse_host_image_decl(),
        _ => self.parse_error_item(),
    }
}

fn parse_pub_item(&mut self) {
    self.start_node(SyntaxKind::PublicItem);
    self.start_node(SyntaxKind::PubModifier);
    self.bump();
    self.finish_node();
    self.parse_item();
    self.finish_node();
}
```

- [ ] **Step 3: Implement data, layout data, interface, and error declarations**

```rust
fn parse_data_decl(&mut self) {
    self.start_node(SyntaxKind::DataDecl);
    self.bump();
    self.expect_identifier();
    self.parse_generic_param_list();
    self.parse_field_block();
    self.finish_node();
}

fn parse_layout_data_decl(&mut self) {
    self.start_node(SyntaxKind::LayoutDataDecl);
    self.bump(); // layout
    self.expect_identifier(); // layout ABI, such as C
    self.expect_keyword(Keyword::Data, "expected data after layout");
    self.expect_identifier();
    self.parse_field_block();
    self.finish_node();
}

fn parse_interface_decl(&mut self) {
    self.start_node(SyntaxKind::InterfaceDecl);
    self.bump();
    self.expect_identifier();
    self.parse_generic_param_list();
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        self.parse_method_signature_decl();
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    self.finish_node();
}

fn parse_error_decl(&mut self) {
    self.start_node(SyntaxKind::ErrorDecl);
    self.bump();
    self.expect_identifier();
    self.parse_field_block();
    self.finish_node();
}
```

- [ ] **Step 4: Implement class, unique class, implements, fields, and params**

```rust
fn parse_unique_class_decl(&mut self) {
    self.start_node(SyntaxKind::UniqueClassDecl);
    self.bump();
    self.expect_keyword(Keyword::Class, "expected class after unique");
    self.parse_class_tail();
    self.finish_node();
}

fn parse_class_decl(&mut self) {
    self.start_node(SyntaxKind::ClassDecl);
    self.bump();
    self.parse_class_tail();
    self.finish_node();
}

fn parse_class_tail(&mut self) {
    self.expect_identifier();
    self.parse_generic_param_list();
    self.parse_implements_clause();
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        self.parse_member();
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
}

fn parse_implements_clause(&mut self) {
    if self.eat_keyword(Keyword::Implements) {
        self.start_node(SyntaxKind::ImplementsClause);
        self.parse_type_ref();
        while self.eat_punct(Punct::Comma) { self.parse_type_ref(); }
        self.finish_node();
    }
}

fn parse_field_block(&mut self) {
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        self.parse_field_decl();
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
}

fn parse_field_decl(&mut self) {
    self.start_node(SyntaxKind::FieldDecl);
    self.expect_identifier();
    self.expect_punct(Punct::Colon, "expected ':'");
    self.parse_type_ref();
    self.finish_node();
}
```

- [ ] **Step 5: Implement methods, constructors, tests, phases, images, and host images**

```rust
fn parse_member(&mut self) {
    match self.peek().kind() {
        TokenKind::Keyword(Keyword::Constructor) => self.parse_constructor_decl(),
        TokenKind::Keyword(Keyword::Fn) | TokenKind::Keyword(Keyword::Asm) => self.parse_method_decl(),
        TokenKind::Keyword(Keyword::Test) => self.parse_test_decl(),
        TokenKind::Identifier if self.peek_n(1).kind() == TokenKind::Punct(Punct::Colon) => self.parse_field_decl(),
        _ => self.parse_member_error(),
    }
}

fn parse_method_decl(&mut self) { self.start_node(SyntaxKind::MethodDecl); self.parse_method_head(); self.parse_block(); self.finish_node(); }
fn parse_constructor_decl(&mut self) { self.start_node(SyntaxKind::ConstructorDecl); self.bump(); self.parse_param_list(); if self.eat_punct(Punct::Arrow) { self.parse_return_type(); } self.parse_block(); self.finish_node(); }
fn parse_test_decl(&mut self) { self.start_node(SyntaxKind::TestDecl); self.bump(); if self.peek().kind() == TokenKind::StringLiteral { self.bump(); } else { self.expect_identifier(); } self.parse_block(); self.finish_node(); }
fn parse_phase_decl(&mut self) { self.start_node(SyntaxKind::PhaseDecl); self.bump(); self.expect_identifier(); self.parse_param_list(); self.parse_block(); self.finish_node(); }

fn parse_method_signature_decl(&mut self) { self.start_node(SyntaxKind::MethodDecl); self.parse_method_head(); self.finish_node(); }

fn parse_method_head(&mut self) {
    if self.eat_keyword(Keyword::Asm) { self.expect_keyword(Keyword::Fn, "expected item"); } else { self.expect_keyword(Keyword::Fn, "expected item"); }
    self.expect_identifier();
    self.parse_generic_param_list();
    self.parse_param_list();
    if self.eat_punct(Punct::Arrow) { self.parse_return_type(); }
}

fn parse_return_type(&mut self) { self.start_node(SyntaxKind::ReturnType); self.parse_type_ref(); self.finish_node(); }

fn parse_param_list(&mut self) {
    self.start_node(SyntaxKind::ParamList);
    self.expect_punct(Punct::OpenParen, "expected '('");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseParen) && self.peek().kind() != TokenKind::Eof {
        self.start_node(SyntaxKind::Param);
        if matches!(self.peek().kind(), TokenKind::Keyword(Keyword::Read) | TokenKind::Keyword(Keyword::Mut) | TokenKind::Keyword(Keyword::Own)) {
            self.bump();
        }
        self.expect_identifier();
        if self.eat_punct(Punct::Colon) { self.parse_type_ref(); }
        self.finish_node();
        if !self.eat_punct(Punct::Comma) { break; }
    }
    self.expect_close_punct(Punct::CloseParen, "expected ')'");
    self.finish_node();
}

fn parse_image_decl(&mut self) {
    self.start_node(SyntaxKind::ImageDecl);
    self.bump();
    self.expect_identifier();
    self.expect_keyword(Keyword::Target, "expected target in image declaration");
    self.parse_type_ref();
    self.parse_image_body();
    self.finish_node();
}

fn parse_host_image_decl(&mut self) {
    self.start_node(SyntaxKind::HostImageDecl);
    self.bump();
    self.expect_keyword(Keyword::Image, "expected image after host");
    self.expect_identifier();
    self.parse_image_body();
    self.finish_node();
}

fn parse_image_body(&mut self) {
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        if self.peek().kind() == TokenKind::Keyword(Keyword::Phase) { self.parse_phase_decl(); } else { self.parse_error_item(); }
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
}

fn parse_member_error(&mut self) {
    let span = self.peek().span();
    self.start_node(SyntaxKind::RecoveryNode);
    self.diagnostic(span, "unexpected token in class body");
    self.builder.error(SyntaxErrorKind::UnexpectedToken, span);
    self.bump();
    self.finish_node();
}
```

- [ ] **Step 6: Verify**

```bash
cargo test --test parser parses_top_level_declaration_forms
cargo test --test parser parses_member_declaration_forms
./scripts/quality-gate.sh
```

- [ ] **Step 7: Commit**

```bash
git add src/syntax/parse.rs tests/parser.rs fixtures/parser/declarations-top.wrela fixtures/parser/declarations-members.wrela
git commit -m "feat: parse declarations and members -Codex Automated"
```

**Acceptance Criteria:**

- Every helper invoked by `parse_item` and `parse_member` is implemented in this task or an earlier prerequisite (`parse_error_item` comes from Task 3).
- `pub` wraps the item in `PublicItem` and contains a `PubModifier`.
- Interface method signatures omit bodies.
- Methods, constructors, tests, and phases parse real `Block` nodes.
- `host image` consumes both keywords and produces `HostImageDecl`.

---

### Task 9: Parse Match, Patterns, And Loop Forms

**Files:**
- Modify: `src/syntax/parse.rs`
- Create: `fixtures/parser/statements-control.wrela`
- Modify: `tests/parser.rs`

**Description:** Add `match`, match arms, patterns, `repeat`, `for`, `drain`, and `loop` statement forms.

- [ ] **Step 1: Add failing control-flow fixture and test**

`fixtures/parser/statements-control.wrela`:

```wrela
class Control {
    fn run(read self, raw: U32) -> U32 {
        match raw {
            0 => return 0
            2..=15 => return raw
            _ => return 99
        }

        repeat 4 as i { worker.step(i) }
        for row in table.rows(mask) { worker.row(row) }
        drain queue.up_to(8) as packet { worker.packet(packet) }
        loop { return 0 }
    }
}
```

```rust
#[test]
fn parses_control_statement_forms() {
    let parsed = parse_fixture("statements-control.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::MatchStmt,
        SyntaxKind::MatchArm,
        SyntaxKind::RepeatStmt,
        SyntaxKind::ForStmt,
        SyntaxKind::DrainStmt,
        SyntaxKind::LoopStmt,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
}
```

Run: `cargo test --test parser parses_control_statement_forms`

Expected: FAIL because control-flow statement parsers do not exist yet.

- [ ] **Step 2: Extend statement dispatcher**

```rust
match self.peek().kind() {
    TokenKind::Keyword(Keyword::Let) => self.parse_let_stmt(),
    TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt(),
    TokenKind::Keyword(Keyword::Match) => self.parse_match_stmt(),
    TokenKind::Keyword(Keyword::Repeat) => self.parse_repeat_stmt(),
    TokenKind::Keyword(Keyword::For) => self.parse_for_stmt(),
    TokenKind::Keyword(Keyword::Drain) => self.parse_drain_stmt(),
    TokenKind::Keyword(Keyword::Loop) => self.parse_loop_stmt(),
    _ => self.parse_expr_stmt(),
}
```

- [ ] **Step 3: Implement match and patterns**

```rust
fn parse_match_stmt(&mut self) {
    self.start_node(SyntaxKind::MatchStmt);
    self.bump();
    self.parse_expr();
    self.expect_punct(Punct::OpenBrace, "expected '{'");
    while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) && self.peek().kind() != TokenKind::Eof {
        self.parse_match_arm();
    }
    self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    self.finish_node();
}

fn parse_match_arm(&mut self) {
    self.start_node(SyntaxKind::MatchArm);
    self.parse_match_pattern();
    self.expect_punct(Punct::FatArrow, "expected match arm");
    if self.peek().kind() == TokenKind::Punct(Punct::OpenBrace) { self.parse_block(); } else { self.parse_stmt(); }
    self.finish_node();
}

fn parse_match_pattern(&mut self) {
    self.start_node(SyntaxKind::MatchPattern);
    match self.peek().kind() {
        TokenKind::Identifier | TokenKind::IntLiteral | TokenKind::StringLiteral => {
            self.bump();
            while self.eat_punct(Punct::Dot) { self.expect_identifier(); }
            if self.eat_punct(Punct::DotDot) || self.eat_punct(Punct::DotDotEq) {
                if self.peek().kind() == TokenKind::IntLiteral { self.bump(); } else { self.expect_identifier(); }
            }
        }
        _ => self.error_at_current(SyntaxErrorKind::ExpectedMatchArm, "expected match arm"),
    }
    self.finish_node();
}
```

- [ ] **Step 4: Implement loop forms**

```rust
fn parse_repeat_stmt(&mut self) { self.start_node(SyntaxKind::RepeatStmt); self.bump(); self.parse_expr(); self.expect_keyword(Keyword::As, "expected as in repeat statement"); self.expect_identifier(); self.parse_block(); self.finish_node(); }
fn parse_for_stmt(&mut self) { self.start_node(SyntaxKind::ForStmt); self.bump(); self.expect_identifier(); self.expect_contextual_identifier("in", "expected 'in' in for statement"); self.parse_expr(); self.parse_block(); self.finish_node(); }
fn parse_drain_stmt(&mut self) { self.start_node(SyntaxKind::DrainStmt); self.bump(); self.parse_expr(); self.expect_keyword(Keyword::As, "expected as in drain statement"); self.expect_identifier(); self.parse_block(); self.finish_node(); }
fn parse_loop_stmt(&mut self) { self.start_node(SyntaxKind::LoopStmt); self.bump(); self.parse_block(); self.finish_node(); }
```

- [ ] **Step 5: Verify**

```bash
cargo test --test parser parses_control_statement_forms
./scripts/quality-gate.sh
```

- [ ] **Step 6: Commit**

```bash
git add src/syntax/parse.rs tests/parser.rs fixtures/parser/statements-control.wrela
git commit -m "feat: parse control-flow statements -Codex Automated"
```

**Acceptance Criteria:**

- Match arms use `Punct::FatArrow`.
- Range patterns consume `Punct::DotDot` or `Punct::DotDotEq`.
- `_` wildcard pattern is accepted as identifier text.
- Repeat, for, drain, and loop statements produce concrete CST nodes.

---

### Task 10: Parse Assertions, Reduce, And Scan

**Files:**
- Modify: `src/syntax/parse.rs`
- Modify: `src/syntax/expr.rs`
- Create: `fixtures/parser/assertions-reduce-scan.wrela`
- Modify: `tests/parser.rs`

**Description:** Add assertion statements and `reduce`/`scan` block expressions with locked grammar.

- [ ] **Step 1: Add failing assertions/reduce/scan fixture and test**

`fixtures/parser/assertions-reduce-scan.wrela`:

```wrela
class Reductions {
    fn run(read self, packets: Table[Packet, 256]) -> U64 {
        let total = reduce packets.rows(valid) as row, acc: U64 = 0 {
            return acc + packets.len[row]
        }

        let found = scan packets as i until packets.len[i] == 0 {
            observer.observe(i)
        }

        assert value total == 0
        assert same service_a.parser == service_b.parser
        return total
    }
}
```

```rust
#[test]
fn parses_assertions_reduce_and_scan() {
    let parsed = parse_fixture("assertions-reduce-scan.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ReduceExpr));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ScanExpr));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::AssertStmt), 2);
}
```

Run: `cargo test --test parser parses_assertions_reduce_and_scan`

Expected: FAIL because assertion dispatch and reduce/scan expressions do not exist yet.

- [ ] **Step 2: Add `Assert` to statement dispatch**

```rust
match self.peek().kind() {
    TokenKind::Keyword(Keyword::Let) => self.parse_let_stmt(),
    TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt(),
    TokenKind::Keyword(Keyword::Match) => self.parse_match_stmt(),
    TokenKind::Keyword(Keyword::Repeat) => self.parse_repeat_stmt(),
    TokenKind::Keyword(Keyword::For) => self.parse_for_stmt(),
    TokenKind::Keyword(Keyword::Drain) => self.parse_drain_stmt(),
    TokenKind::Keyword(Keyword::Loop) => self.parse_loop_stmt(),
    TokenKind::Keyword(Keyword::Assert) => self.parse_assert_stmt(),
    _ => self.parse_expr_stmt(),
}
```

- [ ] **Step 3: Add assertions**

```rust
fn parse_assert_stmt(&mut self) {
    self.start_node(SyntaxKind::AssertStmt);
    self.bump();
    if self.eat_keyword(Keyword::Value) || self.eat_keyword(Keyword::Same) {
        self.parse_expr();
    } else {
        self.error_at_current(SyntaxErrorKind::ExpectedAssertKind, "expected assert kind");
    }
    self.finish_node();
}
```

- [ ] **Step 4: Add reduce and scan branches in `parse_prefix_or_atom`**

```rust
TokenKind::Keyword(Keyword::Reduce) => self.parse_reduce_expr(),
TokenKind::Keyword(Keyword::Scan) => self.parse_scan_expr(),
```

- [ ] **Step 5: Implement reduce and scan**

```rust
fn parse_reduce_expr(&mut self) {
    self.start_node(SyntaxKind::ReduceExpr);
    self.bump();
    self.parse_expr_bp(BindingPower::Postfix);
    self.expect_keyword(Keyword::As, "expected as in reduce expression");
    self.expect_identifier();
    self.expect_punct(Punct::Comma, "expected ','");
    self.expect_identifier();
    self.expect_punct(Punct::Colon, "expected ':'");
    self.parse_type_ref();
    self.expect_punct(Punct::Eq, "expected '='");
    self.parse_expr();
    self.parse_block();
    self.finish_node();
}

fn parse_scan_expr(&mut self) {
    self.start_node(SyntaxKind::ScanExpr);
    self.bump();
    self.parse_expr_bp(BindingPower::Postfix);
    self.expect_keyword(Keyword::As, "expected as in scan expression");
    self.expect_identifier();
    self.expect_keyword(Keyword::Until, "expected until in scan expression");
    self.parse_expr();
    self.parse_block();
    self.finish_node();
}
```

- [ ] **Step 6: Verify**

```bash
cargo test --test parser parses_assertions_reduce_and_scan
./scripts/quality-gate.sh
```

- [ ] **Step 7: Commit**

```bash
git add src/syntax/parse.rs src/syntax/expr.rs tests/parser.rs fixtures/parser/assertions-reduce-scan.wrela
git commit -m "feat: parse assertions reduce and scan -Codex Automated"
```

**Acceptance Criteria:**

- `assert value` and `assert same` parse; other assert kinds emit `expected assert kind`.
- `reduce` requires `as row, acc: Type = init` and a block.
- `scan` requires `as index until expr` and a block.
- CST reconstruction remains exact.

---

## Phase 5: Recovery, Parallel Parsing, CLI, Docs

### Task 11: Add Item, Member, Statement, And Delimiter Recovery

**Files:**
- Modify: `src/syntax/recovery.rs`
- Modify: `src/syntax/parse.rs`
- Create: `fixtures/parser/recovery.wrela`
- Modify: `tests/parser.rs`

**Description:** Add recovery at top-level item, member, statement, and delimiter boundaries. Recovery inserts error nodes only when at least one token is skipped.

- [ ] **Step 1: Add failing recovery fixture and test**

`fixtures/parser/recovery.wrela`:

```wrela
class Broken {
    fn first(read self) -> None {
        let dangling
        return
    }

    trap unexpected

    fn second(read self) -> None {
        return
    }
}

class After {
    fn ok(read self) -> None {
        return
    }
}
```

```rust
#[test]
fn recovers_after_bad_statement_and_parses_following_items() {
    let parsed = parse_fixture("recovery.wrela");

    assert!(has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::RecoveryNode));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::ClassDecl), 2);
    assert!(count_nodes(parsed.tree(), SyntaxKind::MethodDecl) >= 3);
    assert_eq!(parsed.diagnostics()[0].message(), "expected type annotation or initializer in let statement");
}
```

Run: `cargo test --test parser recovers_after_bad_statement_and_parses_following_items`

Expected: FAIL because recovery still stops too early around bad members.

- [ ] **Step 2: Add recovery helpers**

```rust
use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::SyntaxKind;

impl<'a> Parser<'a> {
    pub(crate) fn recover_to_statement_boundary(&mut self) {
        if self.at_statement_start() || matches!(self.peek().kind(), TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof) {
            return;
        }
        self.start_node(SyntaxKind::RecoveryNode);
        while !self.at_statement_start() && !matches!(self.peek().kind(), TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof) {
            self.bump();
        }
        self.finish_node();
    }

    pub(crate) fn consume_to_item_boundary(&mut self) {
        while !self.at_item_start() && self.peek().kind() != TokenKind::Eof {
            self.bump();
        }
    }

    pub(crate) fn consume_to_member_boundary(&mut self) {
        while !self.at_member_start() && !matches!(self.peek().kind(), TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof) {
            self.bump();
        }
    }
}
```

- [ ] **Step 3: Add sync predicates**

```rust
impl<'a> Parser<'a> {
    fn at_item_start(&self) -> bool {
        matches!(self.peek().kind(), TokenKind::Keyword(Keyword::Pub) | TokenKind::Keyword(Keyword::Module) | TokenKind::Keyword(Keyword::Use) | TokenKind::Keyword(Keyword::Data) | TokenKind::Keyword(Keyword::Layout) | TokenKind::Keyword(Keyword::Class) | TokenKind::Keyword(Keyword::Unique) | TokenKind::Keyword(Keyword::Interface) | TokenKind::Keyword(Keyword::Error) | TokenKind::Keyword(Keyword::Image) | TokenKind::Keyword(Keyword::Host))
    }

    fn at_member_start(&self) -> bool {
        matches!(self.peek().kind(), TokenKind::Keyword(Keyword::Constructor) | TokenKind::Keyword(Keyword::Phase) | TokenKind::Keyword(Keyword::Fn) | TokenKind::Keyword(Keyword::Asm) | TokenKind::Keyword(Keyword::Test))
            || (self.peek().kind() == TokenKind::Identifier && self.peek_n(1).kind() == TokenKind::Punct(Punct::Colon))
    }

    fn at_statement_start(&self) -> bool {
        matches!(self.peek().kind(), TokenKind::Keyword(Keyword::Let) | TokenKind::Keyword(Keyword::Return) | TokenKind::Keyword(Keyword::Match) | TokenKind::Keyword(Keyword::Repeat) | TokenKind::Keyword(Keyword::For) | TokenKind::Keyword(Keyword::Drain) | TokenKind::Keyword(Keyword::Loop) | TokenKind::Keyword(Keyword::Assert))
    }
}
```

- [ ] **Step 4: Wire recovery at exact call sites**

Replace `parse_error_item` from Task 3:

```rust
pub(crate) fn parse_error_item(&mut self) {
    self.start_node(SyntaxKind::RecoveryNode);
    self.error_at_current(SyntaxErrorKind::ExpectedItem, "expected item");
    self.bump();
    self.consume_to_item_boundary();
    self.finish_node();
}
```

Replace `parse_member_error` from Task 8:

```rust
fn parse_member_error(&mut self) {
    self.start_node(SyntaxKind::RecoveryNode);
    self.error_at_current(SyntaxErrorKind::UnexpectedToken, "unexpected token in class body");
    self.bump();
    self.consume_to_member_boundary();
    self.finish_node();
}
```

Replace `parse_expr_stmt` from Task 7:

```rust
fn parse_expr_stmt(&mut self) {
    self.start_node(SyntaxKind::ExprStmt);
    let diagnostics_before = self.diagnostics.len();
    self.parse_expr();
    if self.diagnostics.len() > diagnostics_before {
        self.recover_to_statement_boundary();
    }
    self.eat_punct(Punct::Semicolon);
    self.finish_node();
}
```

Close-delimiter diagnostics already use `expect_close_punct` from Task 3, which emits `SyntaxErrorKind::MissingCloseDelimiter`; do not change close delimiters back to `expect_punct`.

- [ ] **Step 5: Verify**

```bash
cargo test --test parser recovers_after_bad_statement_and_parses_following_items
./scripts/quality-gate.sh
```

- [ ] **Step 6: Commit**

```bash
git add src/syntax/recovery.rs src/syntax/parse.rs tests/parser.rs fixtures/parser/recovery.wrela
git commit -m "feat: add parser recovery -Codex Automated"
```

**Acceptance Criteria:**

- Recovery does not create zero-child error nodes at sync tokens.
- Bad statements do not suppress following methods or top-level classes.
- Missing delimiters emit catalog diagnostics and error nodes.

---

### Task 12: Add Parallel Parsing And Typed Summary Views

**Files:**
- Modify: `src/syntax/mod.rs`
- Modify: `src/syntax/parse.rs`
- Modify: `src/syntax/lower.rs`

**Description:** Add deterministic `parse_files_parallel` and a narrow typed summary view for parse CLI output. The view counts top-level item kinds only; it does not extract import binders.

- [ ] **Step 1: Add failing parallel parse and summary tests**

```rust
use wrela::source::SourceMap;
use wrela::syntax::{parse_files_parallel, summarize_module};

#[test]
fn parses_files_parallel_in_file_id_order() {
    let mut source_map = SourceMap::new();
    source_map.load_file(parser_fixture("declarations-members.wrela")).unwrap();
    source_map.load_file(parser_fixture("declarations-top.wrela")).unwrap();
    let lexed_files: Vec<_> = source_map.files().iter().map(lex_file).collect();

    let parsed = parse_files_parallel(&lexed_files, &source_map);
    let ids: Vec<u32> = parsed.iter().map(|parsed| parsed.file_id().raw()).collect();

    assert_eq!(ids, vec![0, 1]);
}

#[test]
fn summarizes_module_counts_top_level_items() {
    let parsed = parse_fixture("declarations-top.wrela");
    let summary = summarize_module(&parsed);

    assert!(summary.item_count() >= 6);
    assert!(summary.node_count() > 0);
    assert!(summary.token_count() > 0);
}
```

Run: `cargo test --test parser parses_files_parallel_in_file_id_order`

Expected: FAIL because `parse_files_parallel` and `summarize_module` do not exist yet.

- [ ] **Step 2: Replace parser public re-export in `src/syntax/mod.rs`**

Replace the Task 3 line `pub use parse::parse_file;` with:

```rust
pub use lower::{summarize_module, ModuleSummary};
pub use parse::{parse_file, parse_files_parallel};
```

- [ ] **Step 3: Add chunked parallel parse API**

```rust
use crate::source::SourceMap;

pub fn parse_files_parallel(lexed_files: &[LexedFile], source_map: &SourceMap) -> Vec<ParsedSyntax> {
    if lexed_files.is_empty() {
        return Vec::new();
    }

    let worker_count = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(lexed_files.len());
    let chunk_size = lexed_files.len().div_ceil(worker_count);

    let mut parsed = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in lexed_files.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|lexed| {
                        let source = source_map.get(lexed.file_id()).expect("lexed file has source");
                        parse_file(lexed, source)
                    })
                    .collect::<Vec<_>>()
            }));
        }

        let mut parsed = Vec::with_capacity(lexed_files.len());
        for handle in handles {
            match handle.join() {
                Ok(mut chunk) => parsed.append(&mut chunk),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
        parsed
    });
    parsed.sort_by_key(|module| module.file_id().raw());
    parsed
}
```

- [ ] **Step 4: Add module summary**

```rust
use super::cst::{ParsedSyntax, SyntaxElement};
use super::syntax_kind::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleSummary {
    item_count: usize,
    node_count: usize,
    token_count: usize,
}

impl ModuleSummary {
    pub const fn new(item_count: usize, node_count: usize, token_count: usize) -> Self {
        Self { item_count, node_count, token_count }
    }
    pub const fn item_count(self) -> usize { self.item_count }
    pub const fn node_count(self) -> usize { self.node_count }
    pub const fn token_count(self) -> usize { self.token_count }
}

pub fn summarize_module(parsed: &ParsedSyntax) -> ModuleSummary {
    let tree = parsed.tree();
    let root = tree.node(tree.root());
    let item_count = tree
        .elements(root.children())
        .iter()
        .filter(|element| match *element {
            SyntaxElement::Node(id) => is_top_level_item(tree.node(id).kind()),
            _ => false,
        })
        .count();
    ModuleSummary::new(item_count, tree.node_count(), tree.token_count())
}

fn is_top_level_item(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PublicItem
            | SyntaxKind::ModuleDecl
            | SyntaxKind::UseDecl
            | SyntaxKind::DataDecl
            | SyntaxKind::LayoutDataDecl
            | SyntaxKind::ClassDecl
            | SyntaxKind::UniqueClassDecl
            | SyntaxKind::InterfaceDecl
            | SyntaxKind::ErrorDecl
            | SyntaxKind::ImageDecl
            | SyntaxKind::HostImageDecl
    )
}
```

- [ ] **Step 5: Verify**

```bash
cargo test --test parser parses_files_parallel_in_file_id_order
cargo test --test parser summarizes_module_counts_top_level_items
./scripts/quality-gate.sh
```

- [ ] **Step 6: Commit**

```bash
git add src/syntax/mod.rs src/syntax/parse.rs src/syntax/lower.rs tests/parser.rs
git commit -m "feat: parse files in parallel and summarize modules -Codex Automated"
```

**Acceptance Criteria:**

- Parallel parse output is sorted by `FileId`.
- Summary scope is documented as top-level item counting only.
- No global mutable parser state exists.

---

### Task 13: Add `wrela parse <root.wrela>`

**Files:**
- Modify: `src/command.rs`
- Modify: `tests/parser.rs`

**Description:** Add read-only parser inspection CLI with deterministic file summaries and parser diagnostics.

- [ ] **Step 1: Add failing CLI tests**

```rust
#[test]
fn surplus_parse_args_exit_two() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_with_io(
        vec![
            "wrela".to_string(),
            "parse".to_string(),
            "root.wrela".to_string(),
            "extra".to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(String::from_utf8(err).unwrap().contains("malformed command"));
    assert!(out.is_empty());
}

#[test]
fn help_lists_parse_command() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_with_io(vec!["wrela".to_string(), "help".to_string()], &mut out, &mut err);

    assert_eq!(code, 0);
    assert!(String::from_utf8(out).unwrap().contains("wrela parse <root.wrela>"));
    assert!(err.is_empty());
}
```

Integration test in `tests/parser.rs`:

```rust
#[test]
fn parse_command_reports_reachable_files() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "parse".to_string(),
            parser_fixture("imports/root.wrela").to_string_lossy().into_owned(),
        ],
        &mut out,
        &mut err,
    );

    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(out.lines().any(|line| line.contains("nodes=") && line.contains("tokens=") && line.contains("items=")));
}
```

- [ ] **Step 2: Add CLI arm and help text**

Update both help branches to print:

```rust
let _ = writeln!(out, "wrela parse <root.wrela>");
```

Add this match arm beside `lex`:

```rust
Some("parse") => match collected.get(2) {
    Some(path) => {
        if collected.len() > 3 {
            let _ = writeln!(err, "malformed command");
            2
        } else {
            parse_root(path, out, err)
        }
    }
    None => {
        let _ = writeln!(err, "missing root file path");
        2
    }
},
```

- [ ] **Step 3: Add parser imports**

```rust
use crate::syntax::{parse_files_parallel, summarize_module};
```

- [ ] **Step 4: Add `parse_root`**

```rust
fn parse_root<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let result = discover_from_root(path);
    let source_map = result.source_map();
    let discovery_diagnostics = result.diagnostics();
    let parsed = parse_files_parallel(result.lexed_files(), source_map);

    let source_root = source_map
        .files()
        .first()
        .and_then(|file| file.path().parent());

    for parsed_file in &parsed {
        let source = source_map.get(parsed_file.file_id()).expect("parsed file has source");
        let summary = summarize_module(parsed_file);
        let diagnostic_count = diagnostics_for_file(discovery_diagnostics, parsed_file.file_id())
            + parsed_file.diagnostics().len();
        let path = format_file_path(source_root, source);

        let _ = writeln!(
            out,
            "file {} {} nodes={} tokens={} items={} diagnostics={}",
            parsed_file.file_id().raw(),
            path,
            summary.node_count(),
            summary.token_count(),
            summary.item_count(),
            diagnostic_count
        );
    }

    print_diagnostics(out, discovery_diagnostics);
    for parsed_file in &parsed {
        print_diagnostics(out, parsed_file.diagnostics());
    }

    let has_parser_errors = parsed.iter().any(|parsed_file| has_errors(parsed_file.diagnostics()));
    if has_errors(discovery_diagnostics) || has_parser_errors { 1 } else { 0 }
}
```

Example stdout shape:

```text
file 0 root.wrela nodes=12 tokens=9 items=2 diagnostics=0
file 1 app/console.wrela nodes=4 tokens=4 items=1 diagnostics=0
```

- [ ] **Step 5: Verify**

```bash
cargo test command::tests::surplus_parse_args_exit_two
cargo test command::tests::help_lists_parse_command
cargo test --test parser parse_command_reports_reachable_files
./scripts/quality-gate.sh
```

- [ ] **Step 6: Commit**

```bash
git add src/command.rs tests/parser.rs
git commit -m "feat: add parse CLI command -Codex Automated"
```

**Acceptance Criteria:**

- `wrela parse <root.wrela>` discovers and parses reachable files.
- Parser diagnostics are printed only by `command.rs`.
- The command exits 1 on parse errors and 2 on malformed CLI usage.

---

### Task 14: Update Documentation

**Files:**
- Modify: `docs/design/compiler-pipeline.md`
- Modify: `docs/design/locked-decisions.md`
- Modify: `docs/design/supported-wrela-subset.md`
- Modify: `docs/implementation/README.md`

**Description:** Document the implemented parser, parser locked decisions, parse command, and supported syntax.

- [ ] **Step 1: Update locked decisions**

Add these rows to `docs/design/locked-decisions.md`:

```markdown
## Parser (implemented)

| Decision | Detail |
|----------|--------|
| Parser strategy | Handwritten recursive descent with Pratt expression parsing |
| Tree shape | Lossless CST first; typed views are derived from CST nodes |
| Trivia | Attached to token elements as leading/trailing ranges; not peer CST children |
| Coverage | V1 parser covers declarations, member bodies, statements, expressions, imports, and recovery |
| Imports | CST parser accepts explicit import binders only; aliases and wildcards are parser errors |
| Recovery | Malformed source produces diagnostics and recovery nodes, not parser panics |
| Parser parallelism | `parse_files_parallel` uses chunked scoped workers and deterministic `FileId` sorting |
| CLI surface | `wrela parse <root.wrela>` inspects parse output; `wrela check` remains out of scope |
```

- [ ] **Step 2: Update supported subset**

Replace the "Not supported yet" parser bullet in `docs/design/supported-wrela-subset.md` with:

```markdown
## Parser: CST syntax accepted

- Module declarations: `module app.root`
- Imports: `use { Name, Other } from module.segment`
- Top-level declarations: `data`, `layout <abi> data`, `class`, `unique class`, `interface`, `error`, `image`, `host image`, and `pub <item>`
- Members: fields, constructors, `fn`, `asm fn`, `test`, and image `phase`
- Types: dotted names, access-qualified types (`read`, `mut`, `own`, `unique`), generic arguments, and integer const generic arguments
- Statements: `let`, `return`, expression statements, `match`, `repeat`, `for`, `drain`, `loop`, `assert value`, and `assert same`
- Expressions: names, integer/string literals, parenthesized expressions, prefix operators, calls, named/positional args, field/index access, binary operators, `try ... else return`, `reduce`, and `scan`

Unsupported import forms:

- `use { Name as Alias } from module.segment`
- `use { * } from module.segment`
```

- [ ] **Step 3: Update pipeline and implementation status**

In `docs/design/compiler-pipeline.md`, add `Parse["parse_files_parallel\n(per reachable LexedFile)"]` between `Import` and `Merge`, and add this row:

```markdown
| `syntax::parse` | `LexedFile` + `SourceFile` | `ParsedSyntax` | Lossless CST + parser diagnostics |
```

Add public APIs:

```rust
pub fn syntax::parse_file(lexed: &LexedFile, source: &SourceFile) -> ParsedSyntax;
pub fn syntax::parse_files_parallel(lexed_files: &[LexedFile], source_map: &SourceMap) -> Vec<ParsedSyntax>;
```

In `docs/implementation/README.md`, replace the parser status row with:

```markdown
| [CST parser](plans/2026-05-24-cst-parser.md) | **Complete** | Lossless CST parser + `wrela parse` |
```

- [ ] **Step 4: Verify**

```bash
rg -n "Parser \\| Not started|Full parser / AST|not supported yet" docs
./scripts/quality-gate.sh
```

Expected: no stale doc says the parser is not started.

- [ ] **Step 5: Commit**

```bash
git add docs/design/compiler-pipeline.md docs/design/locked-decisions.md docs/design/supported-wrela-subset.md docs/implementation/README.md
git commit -m "docs: document CST parser support -Codex Automated"
```

**Acceptance Criteria:**

- Docs reflect implemented parser behavior.
- Docs do not imply `wrela check` exists.
- Quality gate passes.

---

### Task 15: Final Quality Gate And Review Packet

**Files:**
- Review artifacts generated under `docs/implementation/reviews/`

**Description:** Complete local verification and the repository's Phase A, Phase B, and Phase C review workflow.

- [ ] **Step 1: Run verification**

```bash
./scripts/quality-gate.sh
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

- [ ] **Step 2: Save verification log**

```bash
./scripts/plan-review-save-verification.sh docs/implementation/plans/2026-05-24-cst-parser.md .worktrees/feat-cst-parser
```

- [ ] **Step 3: Create Phase A self-review file**

Use the documented per-plan suffix `review-self`. Write `docs/implementation/reviews/2026-05-24-cst-parser-review-self.md` in the worktree with this verdict header after completing the self-review:

```markdown
## Verdict: APPROVED
```

The self-review must list checked diffs, verification output location, and any fixes made before approval.

- [ ] **Step 4: Verify Phase A**

```bash
./scripts/plan-review-check-phase-a.sh .worktrees/feat-cst-parser docs/implementation/plans/2026-05-24-cst-parser.md
```

- [ ] **Step 5: Run Phase B**

```bash
./scripts/plan-review.sh docs/implementation/plans/2026-05-24-cst-parser.md .worktrees/feat-cst-parser
```

- [ ] **Step 6: Loop on review findings**

For every blocking finding, apply the smallest production fix, add or update a targeted test, run `./scripts/quality-gate.sh`, commit, re-run Phase A, then re-run Phase B.

- [ ] **Step 7: Clean review artifacts after approvals**

```bash
./scripts/plan-review-cleanup.sh .worktrees/feat-cst-parser docs/implementation/plans/2026-05-24-cst-parser.md
```

- [ ] **Step 8: Commit final cleanup if any tracked files changed**

```bash
git status --short
git add -u
git commit -m "chore: clean parser review artifacts -Codex Automated"
```

Expected: if `git status --short` is clean after cleanup, skip the commit.

**Acceptance Criteria:**

- Quality gate and strict clean quality gate pass.
- Phase A self review has `## Verdict: APPROVED`.
- Claude and Codex Phase B reviews are approved or all blocking findings are resolved.
- Interim review artifacts are removed before merge.

## Self-Review Checklist

- Every helper invoked by a task is defined in that task or an earlier prerequisite.
- Shared parser integration helpers live in Task 3.5; type/expression/block-only test entries live in the task that introduces that parser entry.
- Every task dependency matches the helper functions it calls.
- `src/syntax/mod.rs` edits are centralized in Task 1.
- Later `src/syntax/mod.rs` edits are explicit public re-export replacements in Task 3 and Task 12 only.
- Expression parser uses checkpoints for postfix and binary wrapping.
- Trivia attachment preserves file-leading whitespace, comments through the first trailing newline, blank lines, and indentation.
- Tests assert `SyntaxKind` presence for declarations, expressions, statements, and recovery.
- Import aliases and wildcards have explicit diagnostics and tests.
- Review script invocations match script usage.
- `./scripts/quality-gate.sh` passes after this plan file is written.
