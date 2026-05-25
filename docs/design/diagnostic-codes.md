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
