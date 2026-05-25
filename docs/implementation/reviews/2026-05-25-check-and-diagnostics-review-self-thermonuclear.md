# Thermo-nuclear self review verdict

- **Plan:** docs/implementation/plans/2026-05-25-check-and-diagnostics.md
- **Branch:** feat/check-and-diagnostics
- **Range:** 5c4cc6c..HEAD (post-feedback)
- **Round:** 2

## Verdict: APPROVED

## Structural regressions

Fixed P0 duplicate-module panic: `ModuleId` now uses `modules.len()` at push time; `ResolvedGraph` retains `path_to_module` for O(1) lookup.

## Missed simplification / code-judo opportunities

- Body/ownership/effect contexts borrow `&[ModuleInput]` (no triple clone).
- `resolve_type_ref` uses a single `resolve_visible_any` lookup with `ItemId` type interning.
- Invalid JSON suggestions are omitted entirely rather than rendered with empty `edits`.

## Spaghetti / branching concerns

None introduced. Prefix expressions now emit `W-CHECK-UNSUPPORTED` like other unsupported forms.

## Boundary / abstraction / type-contract issues

- Import scanner diagnostics in `syntax/imports.rs` now use structured `W-PARSE-*` codes.
- `TypeTable::types_compatible` allows integer literal/subtyping between `I64`/`U32`/`U64` in V1.
- `semanticReport` is serialized in JSON alongside diagnostics.

## File-size / decomposition

`summary.rs` remains ~930 lines; acceptable for this delivery. No file exceeds 1k lines.

## Legibility and maintainability

- Human renderer underline uses UTF-8 character columns, not byte columns.
- Unknown-name diagnostics flush in source span order.
- `nearest_name` tie-break on equal distance prefers lexicographically larger candidate (selects `Console` over `Consola` for `"Consol"` even when names are sorted).

## Required fixes before handoff

All user feedback items (P0–P3 and ranked findings 1–10) addressed in round 2.

## Explicit disagreements

**Maintenance smells deferred:** `CstView::child_nodes` Vec allocation per call and splitting `summary.rs` are not changed in this round — both are performance/organization refactors without correctness impact. `module_path_from_file` whole-file spans for inferred paths remain; that branch only fires when `module` decl is absent.

## User feedback addressed

| Item | Fix |
|------|-----|
| P0 duplicate-module panic | `ModuleId` from `modules.len()` + regression test |
| P1 item type interning | `item_type_ids` map in signature checker |
| P1 uncommitted branch | Committed on feat/check-and-diagnostics |
| P2 unknown CLI flags | Reject `--*` unknown flags with exit 2 |
| P2 trailing format flags | Reject `--json`/`--human` after root path |
| P3 suggest tie-break | Lexicographic larger wins on equal distance |
| Integer literal vs U32 | `types_compatible` for integer builtins |
| PrefixExpr silent pass | `W-CHECK-UNSUPPORTED` + test |
| Import parse codes | Structured codes in `imports.rs` |
| UTF-8 underlines | Char-based column/width in human renderer |
| JSON invalid suggestions | Skip suggestion entirely |
| Unknown-name flush order | Sort by `(file_id, start)` |
| semanticReport in JSON | Added to `wrela.check.v1` payload |
| Double type lookup | Single resolve path with interning |
