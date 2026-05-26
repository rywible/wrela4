# Supported Wrela Subset

## Commands

- `wrela dump tokens <root.wrela>`
- `wrela lex <root.wrela>`
- `wrela parse <root.wrela>`
- `wrela check <root.wrela>`
- `wrela dump mir <root.wrela>`
- `wrela dump asm <root.wrela>`
- `wrela perf compile --mode dev --repeat 1 --json <root.wrela>`
- `wrela perf code --mode dev --repeat 1 --json <bench-root.wrela>`

## MIR Body Forms

`wrela dump mir <root.wrela>` builds W-MIR only after `check` succeeds. The first MIR builder covers the checker-supported body subset: `let`, `return`, literals, names, parenthesized expressions, and binary expressions.

Checked expressions outside this MIR 01 subset must produce a MIR diagnostic and no MIR module. They must not be silently lowered to `None`.
