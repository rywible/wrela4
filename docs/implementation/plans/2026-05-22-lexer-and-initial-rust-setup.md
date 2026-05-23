# Lexer And Initial Rust Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the initial zero-dependency Rust command center with a production-quality, lossless, trivia-preserving lexer and root-driven parallel source discovery.

**Architecture:** The first implementation is one Rust package named `wrela`, with the CLI and compiler nucleus in the same project. Lexing is file-local and mode-free; root-driven discovery expands from an explicit root image file through imports, lexing each frontier batch in parallel by file. All phase artifacts are immutable data, diagnostics are returned rather than printed by compiler phases, and merge order is deterministic.

**Tech Stack:** Rust 2024 edition, Rust standard library only, Cargo test harness, handwritten CLI parser, handwritten lexer, handwritten import-summary parser.

---

## Locked Decisions

- The package name is `wrela`; the binary name is `wrela`.
- `Cargo.toml` uses `edition = "2024"` and `rust-version = "1.85"`.
- There are no external crate dependencies.
- No manifests exist. The command receives a root image file path.
- Source reachability expands only through explicit `use ... from ...` imports in reachable source files.
- Import paths use dotted module syntax: `tests.ring_buffer` resolves to `<root-dir>/tests/ring_buffer.wrela`.
- The source root is the parent directory of the canonical root image file.
- Import module segments must be identifiers. Absolute paths, `..`, path separators, and file extensions inside imports are invalid.
- The lexer has no dev/release mode.
- Lexing is parallelized across files, not within a file.
- Lexing stores byte spans into the original source and does not copy token or trivia text.
- Every token and trivia span must start and end on UTF-8 character boundaries.
- Source text is UTF-8. Non-UTF-8 input is a source loading diagnostic.
- Diagnostics may be spanned or unspanned. Source-free failures must not invent a `FileId`.
- Comments and doc comments are trivia. Doc-comment attachment is not performed by the lexer.
- Block comments are nestable.
- Root-driven discovery canonicalizes loadable file paths before de-duplicating them.
- Production code must not use `unsafe`.
- Production code must not use `todo!()` or `unimplemented!()`.

## Planned File Structure

Create this structure:

```text
Cargo.toml
src/
  main.rs
  lib.rs
  command.rs
  diagnostic.rs
  discover.rs
  source.rs
  lexer/
    batch.rs
    kind.rs
    lex.rs
    mod.rs
    token.rs
    trivia.rs
  syntax/
    imports.rs
    mod.rs
tests/
  lexer.rs
fixtures/
  lexer/
    basic.wrela
    comments.wrela
    errors.wrela
    imports/
      root.wrela
      app/
        console.wrela
        storage.wrela
```

## Public API Shape

The core API after this plan:

```rust
pub fn lexer::lex_file(source: &SourceFile) -> LexedFile;
pub fn lexer::lex_files_parallel(files: &[SourceFile]) -> Vec<LexedFile>;
pub fn syntax::imports::parse_import_summary(lexed: &LexedFile, source: &SourceFile) -> ImportSummary;
pub fn discover::discover_from_root(root: impl AsRef<Path>) -> DiscoverResult;
```

`wrela dump tokens <file.wrela>` lexes one file and prints semantic tokens plus trivia.

`wrela lex <root.wrela>` discovers the reachable source graph from the root image file, lexes reachable files, and prints a deterministic per-file summary.

## Parallel Work Map

- Task 1 must run first.
- Task 2 depends on Task 1.
- Task 3 depends on Task 2 because diagnostics use `Span` and `FileId`.
- Task 4 depends on Task 3.
- Tasks 5, 6, and 7 are sequential because they build the same lexer behavior.
- Tasks 8 and 9 can run in parallel after Task 7.
- Task 10 depends on Tasks 8 and 9.
- Task 11 depends on Task 10.
- Task 12 depends on Task 11.
- Task 13 is the final quality gate.

Each task has a narrow ownership boundary. If two subagents touch the same file, the later task must re-read the current file before editing.

## Subagent Git Discipline

Parallel subagents must not commit directly to the same branch. Each subagent works in its own branch or worktree named for the task, such as `codex/task-08-parallel-lexing`, and task commit steps apply to that isolated branch. The integration owner merges completed task branches back in dependency order from the Parallel Work Map. If two completed branches touch the same file, the later merge owner re-runs that task's verification commands after resolving conflicts.

---

### Task 1: Create The Rust Package Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/command.rs`
- Create: `src/diagnostic.rs`
- Create: `src/discover.rs`
- Create: `src/source.rs`
- Create: `src/lexer/mod.rs`
- Create: `src/lexer/batch.rs`
- Create: `src/lexer/kind.rs`
- Create: `src/lexer/lex.rs`
- Create: `src/lexer/token.rs`
- Create: `src/lexer/trivia.rs`
- Create: `src/syntax/mod.rs`
- Create: `src/syntax/imports.rs`

**Description:** Establish a compiling zero-dependency Rust project with the final module names. This task creates real module boundaries but does not implement lexer behavior.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "wrela"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT"
publish = false

[dependencies]
```

- [ ] **Step 2: Write `src/lib.rs`**

```rust
pub mod command;
pub mod diagnostic;
pub mod discover;
pub mod lexer;
pub mod source;
pub mod syntax;
```

- [ ] **Step 3: Write `src/main.rs`**

```rust
fn main() {
    let code = wrela::command::run(std::env::args());
    std::process::exit(code);
}
```

- [ ] **Step 4: Write `src/command.rs`**

```rust
pub fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    run_with_io(args, &mut out, &mut err)
}

pub fn run_with_io<I, W, E>(args: I, out: &mut W, err: &mut E) -> i32
where
    I: IntoIterator<Item = String>,
    W: std::io::Write,
    E: std::io::Write,
{
    let collected: Vec<String> = args.into_iter().collect();

    match collected.get(1).map(String::as_str) {
        Some("help") | None => {
            let _ = writeln!(out, "wrela commands: help, version");
            0
        }
        Some("version") => {
            let _ = writeln!(out, "wrela 0.1.0");
            0
        }
        Some(other) => {
            let _ = writeln!(err, "unknown command: {other}");
            2
        }
    }
}
```

- [ ] **Step 5: Write empty module files with compiling module exports**

`src/lexer/mod.rs`:

```rust
pub mod batch;
pub mod kind;
pub mod lex;
pub mod token;
pub mod trivia;
```

`src/syntax/mod.rs`:

```rust
pub mod imports;
```

For `src/diagnostic.rs`, `src/discover.rs`, `src/source.rs`, `src/lexer/batch.rs`, `src/lexer/kind.rs`, `src/lexer/lex.rs`, `src/lexer/token.rs`, `src/lexer/trivia.rs`, and `src/syntax/imports.rs`, create empty files. Empty files are acceptable only in this skeleton task because the modules are immediately filled by later tasks.

- [ ] **Step 6: Run the initial build**

Run:

```bash
cargo check
```

Expected: `Finished` with no dependency downloads and no compile errors.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src
git commit -m "feat: create rust command center skeleton -Codex Automated"
```

**Acceptance Criteria:**

- `cargo check` succeeds.
- `cargo metadata --no-deps --format-version 1` lists no third-party dependencies.
- `cargo run -- help` exits 0 and mentions `help` and `version`; later tasks may expand the help text.
- `cargo run -- version` prints `wrela 0.1.0`.

---

### Task 2: Implement Source Files, Spans, And Source Maps

**Files:**
- Modify: `src/source.rs`

**Description:** Add immutable source-file storage, stable file IDs, byte spans, line-start tables, and deterministic source loading.

- [ ] **Step 1: Write tests in `src/source.rs`**

Add this test module at the bottom of `src/source.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn source_file_computes_line_starts() {
        let file = SourceFile::new(FileId::new(7), PathBuf::from("sample.wrela"), "one\rtwo\nthree\r\nfour".to_string());

        assert_eq!(file.line_starts(), &[0, 4, 8, 15]);
    }

    #[test]
    fn span_len_is_byte_based() {
        let span = Span::new(FileId::new(1), 2, 9);

        assert_eq!(span.len(), 7);
        assert!(!span.is_empty());
    }

    #[test]
    fn source_map_assigns_stable_ids() {
        let mut map = SourceMap::new();
        let first = map.add_loaded_file(PathBuf::from("a.wrela"), "a".to_string());
        let second = map.add_loaded_file(PathBuf::from("b.wrela"), "b".to_string());

        assert_eq!(first, FileId::new(0));
        assert_eq!(second, FileId::new(1));
        assert_eq!(map.get(first).unwrap().text(), "a");
        assert_eq!(map.get(second).unwrap().text(), "b");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test source::tests -- --nocapture
```

Expected: compile failure naming missing `SourceFile`, `FileId`, `Span`, or `SourceMap`.

- [ ] **Step 3: Implement `src/source.rs`**

Use this API and keep fields private except where the plan explicitly says otherwise:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileId(u32);

impl FileId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    file_id: FileId,
    start: u32,
    end: u32,
}

impl Span {
    pub const fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self { file_id, start, end }
    }

    pub const fn file_id(self) -> FileId {
        self.file_id
    }

    pub const fn start(self) -> u32 {
        self.start
    }

    pub const fn end(self) -> u32 {
        self.end
    }

    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(id: FileId, path: PathBuf, text: String) -> Self {
        let line_starts = compute_line_starts(&text);
        Self { id, path, text, line_starts }
    }

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_starts(&self) -> &[u32] {
        &self.line_starts
    }

    pub fn span(&self) -> Span {
        Span::new(self.id, 0, self.text.len() as u32)
    }
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add_loaded_file(&mut self, path: PathBuf, text: String) -> FileId {
        let id = FileId::new(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, text));
        id
    }

    pub fn load_file(&mut self, path: impl AsRef<Path>) -> io::Result<FileId> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(self.add_loaded_file(path.to_path_buf(), text))
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.raw() as usize)
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }
}

fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0];
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                starts.push((index + 1) as u32);
                index += 1;
            }
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                starts.push((index + 2) as u32);
                index += 2;
            }
            b'\r' => {
                starts.push((index + 1) as u32);
                index += 1;
            }
            _ => index += 1,
        }
    }

    starts
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test source::tests -- --nocapture
```

Expected: all `source::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add src/source.rs
git commit -m "feat: add source file model -Codex Automated"
```

**Acceptance Criteria:**

- `SourceFile` owns UTF-8 text and line starts.
- `Span` uses byte offsets.
- `SourceMap` assigns deterministic IDs in insertion order.
- No `unsafe`, `todo!()`, or `unimplemented!()` appears in `src/source.rs`.

---

### Task 3: Implement Diagnostics As Data

**Files:**
- Modify: `src/diagnostic.rs`

**Description:** Add diagnostic data structures used by lexing, import discovery, and the CLI. Compiler phases return diagnostics; they do not print directly.

- [ ] **Step 1: Write tests in `src/diagnostic.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};

    #[test]
    fn detects_errors() {
        let diagnostics = vec![
            Diagnostic::new(Severity::Warning, Some(Span::new(FileId::new(0), 1, 2)), "warning"),
            Diagnostic::new(Severity::Error, Some(Span::new(FileId::new(0), 2, 3)), "error"),
        ];

        assert!(has_errors(&diagnostics));
    }

    #[test]
    fn renders_single_line() {
        let diagnostic = Diagnostic::new(Severity::Error, Some(Span::new(FileId::new(3), 4, 9)), "bad token");

        assert_eq!(diagnostic.render_compact(), "error[file=3 4..9]: bad token");
    }

    #[test]
    fn renders_unspanned_diagnostics() {
        let diagnostic = Diagnostic::new(Severity::Error, None, "could not load root file");

        assert_eq!(diagnostic.render_compact(), "error: could not load root file");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test diagnostic::tests -- --nocapture
```

Expected: compile failure naming missing diagnostic types.

- [ ] **Step 3: Implement `src/diagnostic.rs`**

```rust
use crate::source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    span: Option<Span>,
    message: String,
}

impl Diagnostic {
    pub fn new(severity: Severity, span: Option<Span>, message: impl Into<String>) -> Self {
        Self { severity, span, message: message.into() }
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

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn span(&self) -> Option<Span> {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn render_compact(&self) -> String {
        match self.span {
            Some(span) => format!(
                "{}[file={} {}..{}]: {}",
                self.severity.label(),
                span.file_id().raw(),
                span.start(),
                span.end(),
                self.message
            ),
            None => format!("{}: {}", self.severity.label(), self.message),
        }
    }
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.severity == Severity::Error)
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test diagnostic::tests -- --nocapture
```

Expected: all `diagnostic::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add src/diagnostic.rs
git commit -m "feat: add diagnostic data model -Codex Automated"
```

**Acceptance Criteria:**

- Diagnostics carry severity, optional span, and message.
- Diagnostics omit a span for source-free failures.
- `has_errors` reports whether any diagnostic is an error.
- Rendering is deterministic and does not need source text.
- No compiler phase prints diagnostics directly in this task.

---

### Task 4: Define Token, Trivia, And Keyword Types

**Files:**
- Modify: `src/lexer/mod.rs`
- Modify: `src/lexer/kind.rs`
- Modify: `src/lexer/token.rs`
- Modify: `src/lexer/trivia.rs`

**Description:** Define the compact lexer artifact types. Tokens and trivia store spans, not copied source text.

- [ ] **Step 1: Write tests in `src/lexer/kind.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_keywords() {
        assert_eq!(Keyword::from_text("use"), Some(Keyword::Use));
        assert_eq!(Keyword::from_text("class"), Some(Keyword::Class));
        assert_eq!(Keyword::from_text("identifier"), None);
    }

    #[test]
    fn punctuation_debug_is_stable() {
        assert_eq!(format!("{:?}", Punct::Arrow), "Arrow");
        assert_eq!(format!("{:?}", Punct::OpenBrace), "OpenBrace");
    }
}
```

- [ ] **Step 2: Write tests in `src/lexer/token.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};

    #[test]
    fn token_stores_kind_and_span() {
        let span = Span::new(FileId::new(2), 10, 15);
        let token = Token::new(TokenKind::Identifier, span);

        assert_eq!(token.kind(), TokenKind::Identifier);
        assert_eq!(token.span(), span);
    }
}
```

- [ ] **Step 3: Write tests in `src/lexer/trivia.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};

    #[test]
    fn trivia_stores_kind_and_span() {
        let span = Span::new(FileId::new(2), 0, 4);
        let trivia = Trivia::new(TriviaKind::Whitespace, span);

        assert_eq!(trivia.kind(), TriviaKind::Whitespace);
        assert_eq!(trivia.span(), span);
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run:

```bash
cargo test lexer:: -- --nocapture
```

Expected: compile failure naming missing token, trivia, keyword, or punctuation types.

- [ ] **Step 5: Implement `src/lexer/kind.rs`**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    As,
    Assert,
    Asm,
    Class,
    Constructor,
    Data,
    Drain,
    Else,
    Error,
    Fn,
    For,
    From,
    Host,
    Image,
    Implements,
    Interface,
    Layout,
    Let,
    Loop,
    Match,
    Module,
    Mut,
    Own,
    Phase,
    Pub,
    Read,
    Reduce,
    Repeat,
    Return,
    Same,
    Scan,
    Target,
    Test,
    Trap,
    Try,
    Unique,
    Until,
    Use,
    Value,
    With,
}

impl Keyword {
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "as" => Some(Self::As),
            "assert" => Some(Self::Assert),
            "asm" => Some(Self::Asm),
            "class" => Some(Self::Class),
            "constructor" => Some(Self::Constructor),
            "data" => Some(Self::Data),
            "drain" => Some(Self::Drain),
            "else" => Some(Self::Else),
            "error" => Some(Self::Error),
            "fn" => Some(Self::Fn),
            "for" => Some(Self::For),
            "from" => Some(Self::From),
            "host" => Some(Self::Host),
            "image" => Some(Self::Image),
            "implements" => Some(Self::Implements),
            "interface" => Some(Self::Interface),
            "layout" => Some(Self::Layout),
            "let" => Some(Self::Let),
            "loop" => Some(Self::Loop),
            "match" => Some(Self::Match),
            "module" => Some(Self::Module),
            "mut" => Some(Self::Mut),
            "own" => Some(Self::Own),
            "phase" => Some(Self::Phase),
            "pub" => Some(Self::Pub),
            "read" => Some(Self::Read),
            "reduce" => Some(Self::Reduce),
            "repeat" => Some(Self::Repeat),
            "return" => Some(Self::Return),
            "same" => Some(Self::Same),
            "scan" => Some(Self::Scan),
            "target" => Some(Self::Target),
            "test" => Some(Self::Test),
            "trap" => Some(Self::Trap),
            "try" => Some(Self::Try),
            "unique" => Some(Self::Unique),
            "until" => Some(Self::Until),
            "use" => Some(Self::Use),
            "value" => Some(Self::Value),
            "with" => Some(Self::With),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier,
    Keyword(Keyword),
    IntLiteral,
    StringLiteral,
    Punct(Punct),
    Unknown,
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    DocComment,
}
```

- [ ] **Step 6: Implement `src/lexer/token.rs`**

```rust
use crate::source::Span;

use super::kind::TokenKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    kind: TokenKind,
    span: Span,
}

impl Token {
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub const fn kind(self) -> TokenKind {
        self.kind
    }

    pub const fn span(self) -> Span {
        self.span
    }
}
```

- [ ] **Step 7: Implement `src/lexer/trivia.rs`**

```rust
use crate::source::Span;

use super::kind::TriviaKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Trivia {
    kind: TriviaKind,
    span: Span,
}

impl Trivia {
    pub const fn new(kind: TriviaKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub const fn kind(self) -> TriviaKind {
        self.kind
    }

    pub const fn span(self) -> Span {
        self.span
    }
}
```

- [ ] **Step 8: Implement `src/lexer/mod.rs` exports**

```rust
pub mod batch;
pub mod kind;
pub mod lex;
pub mod token;
pub mod trivia;

pub use batch::lex_files_parallel;
pub use kind::{Keyword, Punct, TokenKind, TriviaKind};
pub use lex::{lex_file, LexedFile};
pub use token::Token;
pub use trivia::Trivia;
```

- [ ] **Step 9: Run tests**

Run:

```bash
cargo test lexer:: -- --nocapture
```

Expected: token, trivia, and kind tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/lexer
git commit -m "feat: define lexer artifact types -Codex Automated"
```

**Acceptance Criteria:**

- Tokens and trivia store `Span` only.
- Keyword recognition is exact and lowercase.
- `TokenKind::Eof` exists for parser boundaries.
- Public lexer types are re-exported through `wrela::lexer`.

---

### Task 5: Implement Single-File Lexing For Identifiers, Keywords, And Punctuation

**Files:**
- Modify: `src/lexer/lex.rs`

**Description:** Implement the first working `lex_file` pass with identifiers, keywords, punctuation, whitespace/newline trivia, unknown characters, EOF, and diagnostics. Comment trivia is handled in Task 6.

- [ ] **Step 1: Write tests in `src/lexer/lex.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{Keyword, Punct, TokenKind};
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn file(text: &str) -> SourceFile {
        SourceFile::new(FileId::new(0), PathBuf::from("test.wrela"), text.to_string())
    }

    #[test]
    fn lexes_keywords_identifiers_and_punctuation() {
        let source = file("use { RingBufferTests } from tests.ring_buffer");
        let lexed = lex_file(&source);
        let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword(Keyword::Use),
                TokenKind::Punct(Punct::OpenBrace),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::CloseBrace),
                TokenKind::Keyword(Keyword::From),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::Dot),
                TokenKind::Identifier,
                TokenKind::Eof,
            ]
        );
        assert!(lexed.diagnostics().is_empty());
    }

    #[test]
    fn reports_unknown_characters_and_continues() {
        let source = file("let @ name");
        let lexed = lex_file(&source);
        let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

        assert!(kinds.contains(&TokenKind::Unknown));
        assert_eq!(lexed.diagnostics().len(), 1);
        assert_eq!(lexed.diagnostics()[0].message(), "unknown character");
    }

    #[test]
    fn non_ascii_unknown_spans_cover_the_full_codepoint() {
        let source = file("let café");
        let lexed = lex_file(&source);
        let unknown = lexed
            .tokens()
            .iter()
            .find(|token| token.kind() == TokenKind::Unknown)
            .unwrap();
        let span = unknown.span();

        assert_eq!(&source.text()[span.start() as usize..span.end() as usize], "é");
        assert_eq!(lexed.diagnostics().len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: compile failure naming missing `LexedFile` or `lex_file`.

- [ ] **Step 3: Implement `LexedFile` and punctuation scanning in `src/lexer/lex.rs`**

Use this structure:

```rust
use crate::diagnostic::Diagnostic;
use crate::source::{FileId, SourceFile, Span};

use super::kind::{Keyword, Punct, TokenKind, TriviaKind};
use super::token::Token;
use super::trivia::Trivia;

#[derive(Debug)]
pub struct LexedFile {
    file_id: FileId,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
}

impl LexedFile {
    pub fn new(file_id: FileId, tokens: Vec<Token>, trivia: Vec<Trivia>, diagnostics: Vec<Diagnostic>) -> Self {
        Self { file_id, tokens, trivia, diagnostics }
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn trivia(&self) -> &[Trivia] {
        &self.trivia
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
```

Implement `lex_file(source: &SourceFile) -> LexedFile` as a byte scanner:

```rust
pub fn lex_file(source: &SourceFile) -> LexedFile {
    let mut lexer = Lexer::new(source);
    lexer.run()
}
```

The private `Lexer` should keep:

```rust
struct Lexer<'a> {
    source: &'a SourceFile,
    bytes: &'a [u8],
    cursor: usize,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
}
```

Identifier rules:

```rust
fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}
```

Punctuation rules:

```rust
"->" => Punct::Arrow
"=>" => Punct::FatArrow
"==" => Punct::EqEq
"!=" => Punct::BangEq
"<=" => Punct::LessEq
">=" => Punct::GreaterEq
"&&" => Punct::AmpAmp
"||" => Punct::PipePipe
"{" => Punct::OpenBrace
"}" => Punct::CloseBrace
"[" => Punct::OpenBracket
"]" => Punct::CloseBracket
"(" => Punct::OpenParen
")" => Punct::CloseParen
"," => Punct::Comma
"." => Punct::Dot
":" => Punct::Colon
";" => Punct::Semicolon
"+" => Punct::Plus
"-" => Punct::Minus
"*" => Punct::Star
"/" => Punct::Slash
"%" => Punct::Percent
"=" => Punct::Eq
"!" => Punct::Bang
"<" => Punct::Less
">" => Punct::Greater
"&" => Punct::Amp
"|" => Punct::Pipe
"^" => Punct::Caret
"~" => Punct::Tilde
```

Unknown-character rules:

- ASCII characters that are not identifiers, punctuation, literal starts, or trivia starts emit one `TokenKind::Unknown` with a one-byte span and `Diagnostic::error(span, "unknown character")`.
- Non-ASCII UTF-8 characters that are otherwise unsupported emit one `TokenKind::Unknown` spanning the full UTF-8 codepoint and one `unknown character` diagnostic.
- Do not create spans that start or end in the middle of a UTF-8 codepoint.

Use this helper whenever advancing an unsupported non-ASCII character:

```rust
fn next_char_len(text: &str, cursor: usize) -> usize {
    text[cursor..].chars().next().unwrap().len_utf8()
}
```

Whitespace rules:

- Contiguous `b' ' | b'\t'` and other non-newline ASCII whitespace bytes become one `TriviaKind::Whitespace`.
- `b'\n'` becomes one `TriviaKind::Newline`.
- `b'\r\n'` becomes one `TriviaKind::Newline`.
- Bare `b'\r'` becomes one `TriviaKind::Newline`.
- Whitespace and newline trivia are not semantic tokens.

Always append `TokenKind::Eof` at an empty span at the end of the file.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: all `lexer::lex::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add src/lexer/lex.rs
git commit -m "feat: lex identifiers keywords and punctuation -Codex Automated"
```

**Acceptance Criteria:**

- `lex_file` requires only `&SourceFile`.
- Identifiers and keywords are distinguished.
- Unknown characters are diagnostics, not panics.
- Unknown non-ASCII characters produce one token per UTF-8 codepoint.
- Token and trivia spans are valid UTF-8 character boundaries.
- EOF token is always emitted.
- Whitespace and newlines are preserved as trivia.

---

### Task 6: Preserve Comments And Doc Comments As Trivia

**Files:**
- Modify: `src/lexer/lex.rs`

**Description:** Extend the lexer from whitespace/newline trivia to comment and doc-comment trivia while keeping semantic tokens clean.

- [ ] **Step 1: Add trivia tests in `src/lexer/lex.rs`**

Add these tests to the existing `#[cfg(test)] mod tests` in `src/lexer/lex.rs`, next to the tests from Task 5.

```rust
#[test]
fn preserves_whitespace_newlines_and_comments_as_trivia() {
    let source = file("use  // hello\nfrom");
    let lexed = lex_file(&source);
    let trivia_kinds: Vec<TriviaKind> = lexed.trivia().iter().map(|trivia| trivia.kind()).collect();
    let token_kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert_eq!(
        token_kinds,
        vec![
            TokenKind::Keyword(Keyword::Use),
            TokenKind::Keyword(Keyword::From),
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        trivia_kinds,
        vec![
            TriviaKind::Whitespace,
            TriviaKind::LineComment,
            TriviaKind::Newline,
        ]
    );
}

#[test]
fn preserves_doc_comments_as_distinct_trivia() {
    let source = file("/// docs\nclass Thing {}");
    let lexed = lex_file(&source);
    let trivia_kinds: Vec<TriviaKind> = lexed.trivia().iter().map(|trivia| trivia.kind()).collect();

    assert_eq!(trivia_kinds[0], TriviaKind::DocComment);
}

#[test]
fn nested_block_comments_are_one_trivia_item() {
    let source = file("/* outer /* inner */ done */class Thing {}");
    let lexed = lex_file(&source);

    assert_eq!(lexed.trivia()[0].kind(), TriviaKind::BlockComment);
    assert!(lexed.diagnostics().is_empty());
}

#[test]
fn comment_edge_cases_have_stable_kinds() {
    let source = file("//// not docs\n/**** also not docs */\n//! docs");
    let lexed = lex_file(&source);
    let trivia_kinds: Vec<TriviaKind> = lexed.trivia().iter().map(|trivia| trivia.kind()).collect();

    assert_eq!(
        trivia_kinds,
        vec![
            TriviaKind::LineComment,
            TriviaKind::Newline,
            TriviaKind::BlockComment,
            TriviaKind::Newline,
            TriviaKind::DocComment,
        ]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: comment trivia tests fail because comment trivia is not emitted yet.

- [ ] **Step 3: Implement comment trivia scanning**

Keep the whitespace and newline branches from Task 5. Add comment branches before slash punctuation:

- `//` becomes `TriviaKind::LineComment`.
- `///` becomes `TriviaKind::DocComment` unless the byte after the `///` prefix is `/`.
- `//!` becomes `TriviaKind::DocComment`.
- `////` and longer slash runs become `TriviaKind::LineComment`.
- `/* ... */` becomes `TriviaKind::BlockComment`.
- `/** ... */` becomes `TriviaKind::DocComment` unless the byte after the `/**` prefix is `*`.
- `/*! ... */` becomes `TriviaKind::DocComment`.
- `/****/` and longer star-leading block comments become `TriviaKind::BlockComment`.
- block comments are nestable.

For unterminated block comments, emit the trivia item to EOF and add:

```rust
Diagnostic::error(span, "unterminated block comment")
```

Do not add whitespace or comments to the token stream.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: all lexer tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lexer/lex.rs
git commit -m "feat: preserve lexer trivia -Codex Automated"
```

**Acceptance Criteria:**

- Comment and doc-comment trivia are emitted without entering the semantic token stream.
- Whitespace and newline trivia behavior from Task 5 still passes.
- Doc comments have `TriviaKind::DocComment`.
- Nested block comments work.
- Unterminated block comments produce recoverable diagnostics.

---

### Task 7: Lex String And Integer Literals With Recovery

**Files:**
- Modify: `src/lexer/lex.rs`

**Description:** Add string and integer literal scanning, including recoverable diagnostics for malformed literals.

- [ ] **Step 1: Add literal tests in `src/lexer/lex.rs`**

```rust
#[test]
fn lexes_string_and_integer_literals() {
    let source = file("let answer = 42\nlet path = \"disk\"");
    let lexed = lex_file(&source);
    let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert!(kinds.contains(&TokenKind::IntLiteral));
    assert!(kinds.contains(&TokenKind::StringLiteral));
    assert!(lexed.diagnostics().is_empty());
}

#[test]
fn reports_unterminated_string_and_continues() {
    let source = file("let name = \"unterminated\nclass Next {}");
    let lexed = lex_file(&source);

    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::StringLiteral));
    assert_eq!(lexed.diagnostics()[0].message(), "unterminated string literal");
    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::Keyword(Keyword::Class)));
}

#[test]
fn reports_invalid_escape() {
    let source = file("\"bad\\q\"");
    let lexed = lex_file(&source);

    assert_eq!(lexed.diagnostics()[0].message(), "invalid string escape");
}

#[test]
fn reports_trailing_numeric_underscore() {
    let source = file("123_");
    let lexed = lex_file(&source);

    assert_eq!(lexed.diagnostics()[0].message(), "numeric literal cannot end with underscore");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: literal tests fail because literals are not handled yet.

- [ ] **Step 3: Implement integer scanning**

Rules:

- Decimal: `[0-9][0-9_]*`
- Hex: `0x[0-9a-fA-F_]+`
- A trailing underscore is a diagnostic.
- `0x` without hex digits is a diagnostic with message `hex literal requires digits`.
- Do not parse numeric values in the lexer. Emit `TokenKind::IntLiteral`.

- [ ] **Step 4: Implement string scanning**

Rules:

- Strings start with `"` and produce `TokenKind::StringLiteral`.
- Valid escapes: `\\`, `\"`, `\n`, `\r`, `\t`, `\0`.
- Invalid escapes emit `Diagnostic::error(span, "invalid string escape")` and continue.
- A newline before a closing quote emits `Diagnostic::error(span, "unterminated string literal")`, ends the string token before the newline, and lets normal scanning continue.
- EOF before a closing quote emits the same unterminated diagnostic.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test lexer::lex::tests -- --nocapture
```

Expected: all lexer tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/lexer/lex.rs
git commit -m "feat: lex literals with recovery -Codex Automated"
```

**Acceptance Criteria:**

- Integer and string literals produce semantic tokens.
- Numeric text remains in source text and is not copied into token values.
- Malformed literals produce diagnostics and scanning continues.
- Newline after an unterminated string can still be trivia for following tokens.

---

### Task 8: Implement Parallel Batch Lexing

**Files:**
- Modify: `src/lexer/batch.rs`

**Description:** Add `lex_files_parallel`, using source files as the unit of parallelism and returning deterministic file order.

- [ ] **Step 1: Write tests in `src/lexer/batch.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TokenKind;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn source(id: u32, text: &str) -> SourceFile {
        SourceFile::new(FileId::new(id), PathBuf::from(format!("{id}.wrela")), text.to_string())
    }

    #[test]
    fn lexes_files_in_deterministic_file_order() {
        let files = vec![
            source(2, "class C {}"),
            source(0, "class A {}"),
            source(1, "class B {}"),
        ];

        let lexed = lex_files_parallel(&files);
        let ids: Vec<u32> = lexed.iter().map(|file| file.file_id().raw()).collect();

        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn lexes_each_file_independently() {
        let files = vec![source(0, "class A {}"), source(1, "@")];
        let lexed = lex_files_parallel(&files);

        assert!(lexed[0].tokens().iter().any(|token| token.kind() == TokenKind::Keyword(crate::lexer::Keyword::Class)));
        assert_eq!(lexed[1].diagnostics().len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test lexer::batch::tests -- --nocapture
```

Expected: compile failure naming missing `lex_files_parallel`.

- [ ] **Step 3: Implement `src/lexer/batch.rs`**

```rust
use std::thread;

use crate::source::SourceFile;

use super::lex::{lex_file, LexedFile};

pub fn lex_files_parallel(files: &[SourceFile]) -> Vec<LexedFile> {
    if files.is_empty() {
        return Vec::new();
    }

    let worker_count = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(files.len());
    let chunk_size = files.len().div_ceil(worker_count);

    let mut results = thread::scope(|scope| {
        let mut handles = Vec::new();

        for chunk in files.chunks(chunk_size) {
            handles.push(scope.spawn(move || chunk.iter().map(lex_file).collect::<Vec<_>>()));
        }

        let mut results = Vec::with_capacity(files.len());
        for handle in handles {
            match handle.join() {
                Ok(mut chunk) => results.append(&mut chunk),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }

        results
    });

    results.sort_by_key(|file| file.file_id());
    results
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test lexer::batch::tests -- --nocapture
```

Expected: all batch lexer tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lexer/batch.rs
git commit -m "feat: lex files in parallel batches -Codex Automated"
```

**Acceptance Criteria:**

- Batch lexing uses `std::thread::scope`.
- Results are sorted by `FileId`.
- Empty input returns an empty vector.
- The lexer still has no build mode parameter.

---

### Task 9: Implement Import Summary Parsing

**Files:**
- Modify: `src/syntax/imports.rs`
- Modify: `src/syntax/mod.rs`

**Description:** Parse only enough syntax from a `LexedFile` to find explicit imports for root-driven discovery.

- [ ] **Step 1: Write tests in `src/syntax/imports.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn summary(text: &str) -> ImportSummary {
        let source = SourceFile::new(FileId::new(0), PathBuf::from("root.wrela"), text.to_string());
        let lexed = lex_file(&source);
        parse_import_summary(&lexed, &source)
    }

    #[test]
    fn extracts_dotted_import_paths() {
        let summary = summary("use { Console } from app.console\nuse { Storage } from app.storage");
        let modules: Vec<String> = summary.imports().iter().map(|import| import.module().as_dotted()).collect();

        assert_eq!(modules, vec!["app.console", "app.storage"]);
        assert!(summary.diagnostics().is_empty());
    }

    #[test]
    fn rejects_import_with_missing_module() {
        let summary = summary("use { Console } from");

        assert_eq!(summary.diagnostics()[0].message(), "expected module path after from");
    }

    #[test]
    fn rejects_use_without_from() {
        let summary = summary("use { Console }");

        assert_eq!(summary.diagnostics()[0].message(), "expected from in use import");
    }

    #[test]
    fn recovers_when_second_use_appears_before_from() {
        let summary = summary("use { Broken } use { Console } from app.console");
        let modules: Vec<String> = summary.imports().iter().map(|import| import.module().as_dotted()).collect();

        assert_eq!(summary.diagnostics()[0].message(), "expected from in use import");
        assert_eq!(modules, vec!["app.console"]);
    }

    #[test]
    fn rejects_invalid_module_start_after_from() {
        let summary = summary("use { Console } from 123");

        assert_eq!(summary.diagnostics()[0].message(), "expected module path after from");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test syntax::imports::tests -- --nocapture
```

Expected: compile failure naming missing import summary types.

- [ ] **Step 3: Implement `src/syntax/imports.rs` data types**

```rust
use crate::diagnostic::Diagnostic;
use crate::lexer::{Keyword, Punct, Token, TokenKind};
use crate::source::{SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePath {
    segments: Vec<String>,
}

impl ModulePath {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn as_dotted(&self) -> String {
        self.segments.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportEdge {
    module: ModulePath,
    span: Span,
}

impl ImportEdge {
    pub fn new(module: ModulePath, span: Span) -> Self {
        Self { module, span }
    }

    pub fn module(&self) -> &ModulePath {
        &self.module
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug)]
pub struct ImportSummary {
    imports: Vec<ImportEdge>,
    diagnostics: Vec<Diagnostic>,
}

impl ImportSummary {
    pub fn new(imports: Vec<ImportEdge>, diagnostics: Vec<Diagnostic>) -> Self {
        Self { imports, diagnostics }
    }

    pub fn imports(&self) -> &[ImportEdge] {
        &self.imports
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
```

- [ ] **Step 4: Implement `parse_import_summary`**

Parsing rules:

- Scan semantic tokens only.
- When `Keyword::Use` is found, scan forward looking for `Keyword::From`.
- If EOF appears before `from`, emit `expected from in use import` on the `use` token span.
- If another `Keyword::Use` appears before `from`, emit `expected from in use import` on the first `use` token span and continue scanning from the second `use`.
- After `from`, require `Identifier (Dot Identifier)*`.
- Record the module path span from the first identifier through the last identifier.
- If `from` is present and the next semantic token is EOF or anything other than an identifier, emit `expected module path after from` on the `from` token span.
- If a dotted path has `.` not followed by an identifier, emit `expected identifier after dot in module path`.
- After a malformed module path, skip to the next `Keyword::Use` or EOF.
- Continue scanning after recoverable import errors.

Use source text slicing for identifier segment text:

```rust
fn token_text<'a>(source: &'a SourceFile, token: Token) -> &'a str {
    let start = token.span().start() as usize;
    let end = token.span().end() as usize;
    debug_assert!(source.text().is_char_boundary(start));
    debug_assert!(source.text().is_char_boundary(end));
    &source.text()[start..end]
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test syntax::imports::tests -- --nocapture
```

Expected: all import summary tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/syntax
git commit -m "feat: parse import summaries -Codex Automated"
```

**Acceptance Criteria:**

- Import parsing consumes `LexedFile` and `SourceFile`.
- Full parsing is not introduced in this task.
- Import diagnostics are recoverable.
- Import module paths are dotted identifiers only.

---

### Task 10: Implement Root-Driven Source Discovery

**Files:**
- Modify: `src/discover.rs`

**Description:** Implement source graph discovery from a root image file, with no manifests and parallel lexing for each discovered frontier batch.

- [ ] **Step 1: Write tests in `src/discover.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_temp_dir(name: &str) -> TestDir {
        let mut dir = std::env::temp_dir();
        dir.push(format!("wrela-{name}-{}-{}", std::process::id(), unique_suffix()));
        fs::create_dir_all(&dir).unwrap();
        TestDir { path: fs::canonicalize(dir).unwrap() }
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn discovers_imported_files_from_root() {
        let dir = unique_temp_dir("discover");
        let root = dir.path().join("root.wrela");
        write(&root, "use { Console } from app.console\nimage Root {}");
        write(&dir.path().join("app/console.wrela"), "class Console {}");

        let result = discover_from_root(&root);
        let paths: Vec<String> = result
            .source_map()
            .files()
            .iter()
            .map(|file| file.path().strip_prefix(dir.path()).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();

        assert_eq!(paths, vec!["root.wrela", "app/console.wrela"]);
        assert!(result.diagnostics().is_empty());
    }

    #[test]
    fn reports_missing_imported_file() {
        let dir = unique_temp_dir("missing");
        let root = dir.path().join("root.wrela");
        write(&root, "use { Missing } from app.missing\nimage Root {}");

        let result = discover_from_root(&root);

        assert!(result.diagnostics().iter().any(|diagnostic| diagnostic.message() == "could not load imported file"));
    }

    #[test]
    fn reports_missing_root_without_invented_file_id() {
        let dir = unique_temp_dir("missing-root");
        let result = discover_from_root(dir.path().join("missing.wrela"));

        assert_eq!(result.source_map().files().len(), 0);
        assert_eq!(result.diagnostics()[0].message(), "could not load root file");
        assert!(result.diagnostics()[0].span().is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test discover::tests -- --nocapture
```

Expected: compile failure naming missing `discover_from_root` or `DiscoverResult`.

- [ ] **Step 3: Implement `DiscoverResult`**

```rust
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::lexer::{lex_files_parallel, LexedFile};
use crate::source::{FileId, SourceFile, SourceMap, Span};
use crate::syntax::imports::{parse_import_summary, ImportSummary, ModulePath};

#[derive(Debug)]
pub struct DiscoverResult {
    source_map: SourceMap,
    lexed_files: Vec<LexedFile>,
    import_summaries: Vec<ImportSummary>,
    diagnostics: Vec<Diagnostic>,
}

impl DiscoverResult {
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn lexed_files(&self) -> &[LexedFile] {
        &self.lexed_files
    }

    pub fn import_summaries(&self) -> &[ImportSummary] {
        &self.import_summaries
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
```

- [ ] **Step 4: Implement root-driven discovery**

Rules:

- Root file gets `FileId::new(0)`.
- Canonicalize the root path before loading it. If canonicalization or loading fails, return an empty `SourceMap`, no lexed files, no import summaries, and one unspanned diagnostic with message `could not load root file`.
- Source root is the canonical root file's `parent().unwrap_or(Path::new("."))`.
- `ModulePath { segments: ["app", "console"] }` resolves to `<source-root>/app/console.wrela`.
- Canonicalize each resolved import path before loading it. If canonicalization or loading fails, emit `Diagnostic::error(import_span, "could not load imported file")`.
- Use a `BTreeMap<PathBuf, FileId>` keyed by canonical path to de-duplicate paths.
- Use `VecDeque<PathBuf>` as the frontier.
- Within each frontier batch, load candidate paths in ascending canonical `PathBuf` order so `FileId` assignment is stable.
- Load all files in a frontier batch, then call `lex_files_parallel` on that batch.
- Parse import summaries after lexing the batch.
- Resolve and canonicalize unseen imports into sorted candidate records keyed by canonical or resolved path before extending the next frontier.
- `DiscoverResult::diagnostics()` contains a merged view of root-load, file-load, lexer, and import-summary diagnostics. `LexedFile` and `ImportSummary` still retain their own diagnostics.
- Diagnostic order is deterministic:
  - root-load diagnostic first, when present;
  - otherwise, for each `FileId` in ascending order, append that file's lexer diagnostics in source order, then import-summary diagnostics in source order, then import-load diagnostics from that file sorted by importing span and resolved target path.

Path resolution helper:

```rust
fn resolve_module(source_root: &Path, module: &ModulePath) -> PathBuf {
    let mut path = source_root.to_path_buf();
    for segment in module.segments() {
        path.push(segment);
    }
    path.set_extension("wrela");
    path
}
```

For root load failure, create an empty `SourceMap`, no lexed files, no import summaries, and one diagnostic:

```rust
Diagnostic::unspanned_error("could not load root file")
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test discover::tests -- --nocapture
```

Expected: all discovery tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/discover.rs
git commit -m "feat: discover sources from root imports -Codex Automated"
```

**Acceptance Criteria:**

- No manifest files are read or supported.
- Reachability starts from a root image path.
- Imports fan out through frontier batches.
- Newly discovered batches are lexed with `lex_files_parallel`.
- Duplicate imports load once after canonical-path de-duplication.
- Root-load failure diagnostics are unspanned.
- `DiscoverResult::diagnostics()` merges discovery, lexer, and import-summary diagnostics in the specified deterministic order.

---

### Task 11: Implement Lexer CLI Commands

**Files:**
- Modify: `src/command.rs`

**Description:** Add the first useful command-center commands for lexing and token dumping.

- [ ] **Step 1: Write tests in `src/command.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_lexer_commands() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(vec!["wrela".to_string(), "help".to_string()], &mut out, &mut err);

        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("wrela lex <root.wrela>"));
        assert!(out.contains("wrela dump tokens <file.wrela>"));
        assert!(err.is_empty());
    }

    #[test]
    fn unknown_command_exits_two() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(vec!["wrela".to_string(), "wat".to_string()], &mut out, &mut err);

        assert_eq!(code, 2);
        assert!(String::from_utf8(err).unwrap().contains("unknown command: wat"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test command::tests -- --nocapture
```

Expected: help test fails because lexing commands are not listed yet.

- [ ] **Step 3: Implement command dispatch**

Supported commands:

```text
wrela help
wrela version
wrela dump tokens <file.wrela>
wrela lex <root.wrela>
```

Behavior:

- `help` prints command usage and returns 0.
- `version` prints `wrela 0.1.0` and returns 0.
- `dump tokens` loads one source file, lexes it with `lex_file`, prints tokens and trivia, prints diagnostics, returns 1 if any diagnostic is an error.
- `lex` calls `discover_from_root`, prints per-file counts in `FileId` order, prints diagnostics, returns 1 if any diagnostic is an error.
- unknown commands return 2.
- malformed command arguments return 2.

Output shape for `lex`:

```text
file 0 root.wrela tokens=8 trivia=4 diagnostics=0
file 1 app/console.wrela tokens=5 trivia=1 diagnostics=0
```

Output shape for `dump tokens`:

```text
token Keyword(Use) 0..3
trivia Whitespace 3..4
token Identifier 4..11
token Eof 11..11
```

Use `Diagnostic::render_compact()` for diagnostics.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test command::tests -- --nocapture
```

Expected: command unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/command.rs
git commit -m "feat: add lexer cli commands -Codex Automated"
```

**Acceptance Criteria:**

- CLI uses no external parser crate.
- CLI command behavior is testable through `run_with_io`.
- Compiler phases still return diagnostics as data.
- CLI handles printing and exit codes.

---

### Task 12: Add Lexer Fixtures And Integration Tests

**Files:**
- Create: `tests/lexer.rs`
- Create: `fixtures/lexer/basic.wrela`
- Create: `fixtures/lexer/comments.wrela`
- Create: `fixtures/lexer/errors.wrela`
- Create: `fixtures/lexer/imports/root.wrela`
- Create: `fixtures/lexer/imports/app/console.wrela`
- Create: `fixtures/lexer/imports/app/storage.wrela`

**Description:** Add integration coverage that exercises the lexer and root-driven discovery through stable fixture files.

- [ ] **Step 1: Create fixture files**

`fixtures/lexer/basic.wrela`:

```wrela
use { Console } from app.console

class RingBufferTests {
    test "wraps" {
        let capacity = 64
        return None
    }
}
```

`fixtures/lexer/comments.wrela`:

```wrela
/// suite docs
class Commented {
    // line comment
    fn run(read self) -> None {
        /* nested /* inner */ comment */
        return None
    }
}
```

`fixtures/lexer/errors.wrela`:

```wrela
class Broken {
    test "bad string" {
        let name = "unterminated
        let bad = @
    }
}
```

`fixtures/lexer/imports/root.wrela`:

```wrela
use { Console } from app.console
use { Storage } from app.storage

image Root {}
```

`fixtures/lexer/imports/app/console.wrela`:

```wrela
class Console {}
```

`fixtures/lexer/imports/app/storage.wrela`:

```wrela
class Storage {}
```

- [ ] **Step 2: Write `tests/lexer.rs`**

```rust
use std::path::PathBuf;

use wrela::diagnostic::has_errors;
use wrela::discover::discover_from_root;
use wrela::lexer::{lex_file, TokenKind, TriviaKind};
use wrela::source::{FileId, SourceFile};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/lexer").join(path)
}

#[test]
fn basic_fixture_lexes_without_errors() {
    let path = fixture("basic.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);

    assert!(!has_errors(lexed.diagnostics()));
    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::StringLiteral));
}

#[test]
fn comments_fixture_preserves_doc_and_block_comment_trivia() {
    let path = fixture("comments.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);
    let trivia: Vec<TriviaKind> = lexed.trivia().iter().map(|item| item.kind()).collect();

    assert!(trivia.contains(&TriviaKind::DocComment));
    assert!(trivia.contains(&TriviaKind::BlockComment));
}

#[test]
fn errors_fixture_reports_recoverable_lexer_errors() {
    let path = fixture("errors.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);

    assert!(has_errors(lexed.diagnostics()));
    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::Keyword(wrela::lexer::Keyword::Let)));
}

#[test]
fn root_discovery_reaches_imported_files() {
    let result = discover_from_root(fixture("imports/root.wrela"));
    let files: Vec<String> = result
        .source_map()
        .files()
        .iter()
        .map(|file| file.path().file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert_eq!(files, vec!["root.wrela", "console.wrela", "storage.wrela"]);
    assert!(!has_errors(result.diagnostics()));
}
```

- [ ] **Step 3: Run integration tests**

Run:

```bash
cargo test --test lexer -- --nocapture
```

Expected: all integration tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/lexer.rs fixtures/lexer
git commit -m "test: add lexer integration fixtures -Codex Automated"
```

**Acceptance Criteria:**

- Fixture tests cover ordinary lexing, trivia preservation, recovery diagnostics, and root-driven import discovery.
- Integration tests use only the public crate API.
- Fixture files are valid Wrela-shaped source except the intentional recovery fixture.

---

### Task 13: Final Quality Gate

**Files:**
- Modify only files required by failures found in this task.

**Description:** Run the full local quality gate and remove any unfinished code markers from production paths.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --check
```

Expected: pass. If it fails, run `cargo fmt`, inspect the diff, and include formatting changes in the final commit.

- [ ] **Step 2: Treat compiler warnings as errors**

Run:

```bash
RUSTFLAGS="-D warnings" cargo check --all-targets
```

Expected: all targets compile with zero warnings.

- [ ] **Step 3: Run Clippy with warnings denied**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: Clippy reports no warnings.

- [ ] **Step 4: Run all tests**

Run:

```bash
cargo test -- --nocapture
```

Expected: all unit and integration tests pass.

- [ ] **Step 5: Check production source for unfinished markers**

Run:

```bash
rg -n '\b(todo!|unimplemented!)\s*\(|\bunsafe\b' src --glob '*.rs'
```

Expected: no matches. Treat any match in production source as a failure, including comments and string literals, because the initial compiler code should not need these markers or terms.

- [ ] **Step 6: Confirm zero external dependencies**

Run:

```bash
cargo metadata --no-deps --format-version 1
```

Expected: output includes package `wrela` and does not download or list third-party dependency packages.

- [ ] **Step 7: Run CLI smoke tests**

Run:

```bash
cargo run -- help
cargo run -- version
cargo run -- dump tokens fixtures/lexer/basic.wrela
cargo run -- lex fixtures/lexer/imports/root.wrela
```

Expected:

- `help` lists `wrela lex <root.wrela>` and `wrela dump tokens <file.wrela>`.
- `version` prints `wrela 0.1.0`.
- `dump tokens` prints token and trivia lines.
- `lex` prints three reachable files with token/trivia/diagnostic counts.

- [ ] **Step 8: Commit final fixes**

If this task changed files:

```bash
git add Cargo.toml src tests fixtures
git commit -m "chore: finish lexer quality gate -Codex Automated"
```

If this task changed no files, do not create an empty commit.

**Acceptance Criteria:**

- `cargo fmt --check` passes.
- `RUSTFLAGS="-D warnings" cargo check --all-targets` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test -- --nocapture` passes.
- `rg -n '\b(todo!|unimplemented!)\s*\(|\bunsafe\b' src --glob '*.rs'` has no matches.
- CLI smoke tests behave exactly as specified.
- `git status --short` is clean after any final commit.

---

## Self-Review Checklist For Implementers

- The lexer has no dev/release mode parameter.
- The implementation does not read or generate a manifest file.
- `discover_from_root` starts from a root image file path.
- Imports are the only source reachability edges.
- Lexing one file needs only `&SourceFile`.
- Batch lexing sorts results by `FileId`.
- Tokens and trivia store spans into source text.
- Token and trivia spans never split UTF-8 codepoints.
- Diagnostics are returned as data until the CLI prints them.
- Source-free failures use unspanned diagnostics instead of invented file IDs.
- `DiscoverResult::diagnostics()` is the deterministic merged diagnostic view.
- Tests cover trivia preservation and error recovery.
- The project has no external dependencies.
