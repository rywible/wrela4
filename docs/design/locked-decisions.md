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

## CLI (implemented)

| Command | Behavior |
|---------|----------|
| `help`, `version` | Exit 0 |
| `dump tokens <file>` | Lex one file; exit 1 on errors |
| `lex <root.wrela>` | Discover + lex graph; exit 1 on errors |
| Unknown command | Exit 2 |
| Malformed arity | Exit 2 (no surplus arguments) |

## Review workflow

| Decision | Detail |
|----------|--------|
| Plan execution | Isolated git worktree per plan |
| Gate | Phase A (self) → Phase B (Claude + Codex) → Phase C (cleanup + merge) |
| Interim review files | Ephemeral; delete before merge (see `plan-review-cleanup.sh`) |

## References

- [`0001-rust-command-center-and-zero-dependency-nucleus.md`](0001-rust-command-center-and-zero-dependency-nucleus.md)
- [`2026-05-22-wrela-language-and-test-design.md`](2026-05-22-wrela-language-and-test-design.md)
- [`../implementation/plans/2026-05-22-lexer-and-initial-rust-setup.md`](../implementation/plans/2026-05-22-lexer-and-initial-rust-setup.md)
