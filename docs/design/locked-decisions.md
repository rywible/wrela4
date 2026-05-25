# Locked decisions

Rules agents must not violate without an explicit new decision doc in
`docs/design/`. When a plan adds locked decisions, update this file.

## Toolchain and repository

| Decision | Detail |
|----------|--------|
| Package name | `wrela` (library + binary) |
| Rust edition | 2024, `rust-version = "1.85"` |
| External dependencies | **None** by default; new crates require an ADR |
| CI | **None** — local `./scripts/quality-gate.sh` is the verifier |
| Production markers | No `unsafe`, `todo!()`, `unimplemented!()` in `src/` |

## Project shape

| Decision | Detail |
|----------|--------|
| Command center | CLI and compiler nucleus in one Rust project |
| CLI parser | Handwritten (no clap etc.) |
| Diagnostics | Returned as data; only `command.rs` prints |
| Phase artifacts | Immutable; phases do not mutate prior outputs |
| Parallelism | Across files/batches, not ad-hoc shared mutable compiler state |

## Source model

| Decision | Detail |
|----------|--------|
| Manifests | **None** — root image file path is the entry point |
| Reachability | Only via `use { ... } from <module.path>` imports |
| Source root | Parent directory of the canonical root image file |
| Module paths | Dotted identifiers (`app.console` → `app/console.wrela` under root) |
| Invalid import paths | Absolute paths, `..`, `/` separators, file extensions in segments |
| Source encoding | UTF-8; load failures are diagnostics |
| Path de-dupe | Canonicalize before deduplicating loaded files |

## Lexer (implemented)

| Decision | Detail |
|----------|--------|
| Mode | No dev/release lex mode |
| Parallelism | `lex_files_parallel` across files; single-file lex is sequential |
| Spans | Byte offsets into owned source text; no copied token/trivia text |
| UTF-8 | Spans must align to char boundaries |
| Comments | Trivia only; doc attachment not done in lexer |
| Block comments | Nestable; unterminated → recoverable diagnostic |
| Unspanned failures | Root load failure must not invent a `FileId` |

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

## CLI (implemented)

| Command | Behavior |
|---------|----------|
| `help`, `version` | Exit 0 |
| `dump tokens <file>` | Lex one file; exit 1 on errors |
| `lex <root.wrela>` | Discover + lex graph; exit 1 on errors |
| `parse <root.wrela>` | Discover + parse graph; exit 1 on errors |
| Unknown command | Exit 2 |
| Malformed arity | Exit 2 (no surplus arguments) |

## Review workflow

| Decision | Detail |
|----------|--------|
| Plan execution | Feature branch from `main` in the main repo checkout (not a worktree) |
| Gate | Phase A (self) → user feedback → Phase C (cleanup + merge) |
| Feedback policy | Fix **all** suggestions at **all** severities; no deferral unless explicit disagreement is documented |
| Interim review files | Ephemeral; delete before merge (see `plan-review-cleanup.sh`) |

## References

- [`0001-rust-command-center-and-zero-dependency-nucleus.md`](0001-rust-command-center-and-zero-dependency-nucleus.md)
- [`2026-05-22-wrela-language-and-test-design.md`](2026-05-22-wrela-language-and-test-design.md)
- [`../implementation/plans/2026-05-22-lexer-and-initial-rust-setup.md`](../implementation/plans/2026-05-22-lexer-and-initial-rust-setup.md)
