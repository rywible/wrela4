# Compiler Pipeline

| Command | Pipeline |
|---|---|
| `wrela dump mir <root.wrela>` | discover -> lex -> parse -> check -> W-MIR build -> verify -> text dump |
| `wrela dump asm <root.wrela>` | check -> W-MIR -> LIR -> regalloc -> AArch64 assembly |
| `wrela perf compile <root.wrela>` | compile pipeline telemetry |
| `wrela perf code <bench-root.wrela>` | generated-code smoke telemetry with checksum |

## Phase Responsibilities

| Phase | Input | Output | Responsibility |
|---|---|---|---|
| `mir` | `CheckResult` | `MirBuildResult` | Regioned Effect SSA build, verify, deterministic text |
