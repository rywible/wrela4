# Supported Wrela subset (toolchain today)

What the **current** command center accepts. This is not the full language spec
— see [`2026-05-22-wrela-language-and-test-design.md`](2026-05-22-wrela-language-and-test-design.md).

Last updated: lexer + import-summary phase (2026-05-22 plan complete).

## Files

- Extension: `.wrela`
- Encoding: UTF-8
- Entry: explicit root image path (no manifest)

## Imports (parsed for discovery)

```wrela
use { Name, Other } from module.segment
```

- `from` target: dotted identifiers only (`app.console`).
- Maps to `<source-root>/app/console.wrela` (dots → path separators).
- Invalid: `/` in path, `..`, absolute paths, `.wrela` in segments.
- Recoverable errors: missing `from`, missing module, trailing dot, duplicate `use` before `from`.

The import binder list `{ Name, Other }` is **not** semantically validated yet — only the module path is extracted.

## Lexer: tokens

- **Keywords** — reserved words recognized by the lexer (e.g. `use`, `from`, `class`, `interface`, `match`, `return`, …). See `src/lexer/kind.rs` for the full set.
- **Identifiers** — ASCII start + continue; non-ASCII identifiers use full codepoint spans.
- **Literals** — decimal and `0x` hex integers (with `_` separators); double-quoted strings with escapes `\n`, `\r`, `\t`, `\\`, `\"`, `\0`.
- **Punctuation** — operators and delimiters (`+`, `-`, `->`, `=>`, `{`, `}`, `[`, `]`, `(`, `)`, `.`, `,`, `;`, `:`, `/`, etc.).
- **Unknown** — single-byte unrecognized ASCII → `Unknown` token + diagnostic; scanning continues.
- **EOF** — always emitted once at end.

## Lexer: trivia (not tokens)

| Form | Trivia kind |
|------|-------------|
| whitespace | `Whitespace` |
| newline | `Newline` |
| `//` | `LineComment` |
| `///` | `DocComment` |
| `////` | `LineComment` (extra slash) |
| `//!` | `DocComment` |
| `/* */` | `BlockComment` or `DocComment` (see lexer rules) |
| `/** */` | `DocComment` |
| nested `/* */` | supported |

Unterminated block comment → error diagnostic; trivia runs to EOF.

## Not supported yet

- Full parser / AST
- Name resolution beyond import paths
- Type checking, ownership, effects
- `wrela check`, `wrela build`, `wrela test` commands
- Classes, methods, tables, layouts (keywords may lex; grammar not parsed)

## Fixtures

| Path | Exercises |
|------|-----------|
| `fixtures/lexer/basic.wrela` | ordinary tokens |
| `fixtures/lexer/comments.wrela` | trivia preservation |
| `fixtures/lexer/errors.wrela` | recovery diagnostics |
| `fixtures/lexer/imports/` | root-driven multi-file discovery |

Run:

```bash
cargo run -- dump tokens fixtures/lexer/basic.wrela
cargo run -- lex fixtures/lexer/imports/root.wrela
```
