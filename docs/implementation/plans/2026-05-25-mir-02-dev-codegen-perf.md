# MIR 02 Dev Codegen And Perf Tooling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the dev-mode path from verified W-MIR to AArch64 assembly, plus first-class compiler and generated-code performance tooling.

**Architecture:** MIR 02 consumes MIR 01's verified W-MIR and lowers it to a target-near LIR, performs single-pass register allocation, emits deterministic AArch64 assembly, and adds `wrela perf compile` / `wrela perf code`. The generated-code perf path is intentionally lightweight: initial benchmarks are deterministic leaf-function style programs with checksum validation, designed to show directionally significant changes without long statistical runs.

**Tech Stack:** Rust 2024, standard library only, W-MIR from MIR 01, handwritten LIR, handwritten AArch64 assembly printer, `std::time::Instant`, optional local execution through the host toolchain only when the host is AArch64 macOS and `cc` is available.

---

## Locked Decisions

- MIR 02 does not add release optimizer pass controls.
- Dev codegen prioritizes correctness and fast compilation over generated-code quality.
- Dev codegen must map W-MIR block arguments to AArch64 ABI argument registers before any `ReadValue` use can be considered initialized.
- Dev register allocation fails with a diagnostic if it cannot assign a distinct physical register. It must never clamp multiple virtual registers onto the same physical register.
- MIR 02 `wrela perf code` benchmarks execute only zero-parameter generated functions. Parameterized generated-code benchmarks require a separate approved harness plan.
- LIR drops Wrela-level abstractions but retains target-level metadata needed for sound scheduling and register allocation.
- `wrela perf compile` must work without executing generated code.
- `wrela perf code` must check output equivalence before reporting runtime.
- Perf commands are not part of `./scripts/quality-gate.sh` by default.
- If the host cannot execute generated AArch64 code, `wrela perf code` exits `2` with a clear message instead of pretending to measure.
- No external crate dependencies are introduced.

## Planned File Structure

```text
src/mir/
  perf.rs
  lir.rs
  lower.rs
  aarch64.rs
  lir_verify.rs
  regalloc.rs
  emit.rs
src/command.rs
tests/
  mir_codegen.rs
  mir_perf.rs
fixtures/
  perf/
    scalar_const.wrela
scripts/
  perf-smoke.sh
  perf-compare.sh
```

## Public API Shape

```rust
pub fn mir::lower_to_lir(module: &mir::MirModule) -> mir::LirModule;
pub fn mir::verify_lir(module: &mir::LirModule) -> mir::LirVerifyResult;
pub fn mir::regalloc::allocate_registers(lir: &mir::LirModule) -> Result<mir::AllocatedProgram, mir::RegallocError>;
pub fn mir::emit::emit_aarch64(program: &mir::AllocatedProgram) -> String;
pub fn mir::emit::assembly_symbol(function_symbol: &str) -> String;

pub struct mir::perf::CompilePerfReport;
pub struct mir::perf::CodePerfReport;
```

## Parallel Work Map

- Task 1 must run first because it creates the full shared LIR surface used by every subsequent task.
- Task 2 must run second because it adds LIR verification and locks ABI metadata invariants.
- Task 3 depends on Tasks 1-2 and owns lowering.
- Task 4 depends on Tasks 1-3 and owns regalloc/emission.
- No two MIR 02 subagents may edit `src/mir/mod.rs` or `tests/mir_codegen.rs` at the same time; those shared files are reserved for integration commits.
- Task 5 depends on Task 4.
- Task 6 depends on Task 5.
- Task 7 can run after Task 5 and in parallel with Task 6 if it stays in scripts/tests.
- Task 8 runs last.

---

### Task 1: LIR Core And Target Metadata

**Files:**
- Create: `src/mir/lir.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/lir.rs`

**Description:** Define the target-near LIR model with scalar virtual registers, basic blocks, instructions, and target-level metadata.

- [ ] **Step 1: Write failing LIR tests**

Create `src/mir/lir.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lir_assigns_dense_virtual_registers_and_instructions() {
        let mut module = LirModule::new();
        let function = module.push_function("root.F.add".to_string());
        let block = module.push_block(function, "entry".to_string());
        let lhs = module.push_vreg(RegisterClass::Gp64);
        let rhs = module.push_vreg(RegisterClass::Gp64);
        let out = module.push_vreg(RegisterClass::Gp64);
        let inst = module.push_inst(
            block,
            LirInst::new(
                LirOpcode::Add,
                vec![LirOperand::Reg(lhs), LirOperand::Reg(rhs)],
                vec![out],
                TargetEffects::pure(),
            ),
        );

        assert_eq!(lhs.raw(), 0);
        assert_eq!(rhs.raw(), 1);
        assert_eq!(out.raw(), 2);
        assert_eq!(inst.raw(), 0);
        assert_eq!(module.block(block).instructions(), &[inst]);
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test mir::lir::tests::lir_assigns_dense_virtual_registers_and_instructions
```

Expected: fail because LIR is not implemented.

- [ ] **Step 3: Implement LIR types**

Create `src/mir/lir.rs`:

```rust
macro_rules! lir_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self { Self(raw) }
            pub const fn raw(self) -> u32 { self.0 }
        }
    };
}

lir_id!(LirFunctionId);
lir_id!(LirBlockId);
lir_id!(LirInstId);
lir_id!(VirtualReg);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterClass {
    Gp64,
    Gp32,
    Vec128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LirOperand {
    Reg(VirtualReg),
    ImmI64(i64),
    Label(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LirOpcode {
    Mov,
    Add,
    Ret,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOrder {
    Relaxed,
    Acquire,
    Release,
    Sequential,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SideEffectClass {
    Pure,
    MemoryRead,
    MemoryWrite,
    Volatile,
    Synchronization,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetEffects {
    may_trap: bool,
    volatile: bool,
    memory_order: Option<MemoryOrder>,
    side_effect: SideEffectClass,
}

impl TargetEffects {
    pub const fn pure() -> Self {
        Self {
            may_trap: false,
            volatile: false,
            memory_order: None,
            side_effect: SideEffectClass::Pure,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirInst {
    opcode: LirOpcode,
    operands: Vec<LirOperand>,
    results: Vec<VirtualReg>,
    effects: TargetEffects,
}

impl LirInst {
    pub fn new(
        opcode: LirOpcode,
        operands: Vec<LirOperand>,
        results: Vec<VirtualReg>,
        effects: TargetEffects,
    ) -> Self {
        Self { opcode, operands, results, effects }
    }

    pub fn opcode(&self) -> LirOpcode { self.opcode }
    pub fn operands(&self) -> &[LirOperand] { &self.operands }
    pub fn results(&self) -> &[VirtualReg] { &self.results }
    pub fn effects(&self) -> TargetEffects { self.effects }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirBlock {
    name: String,
    instructions: Vec<LirInstId>,
}

impl LirBlock {
    pub fn instructions(&self) -> &[LirInstId] { &self.instructions }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirFunction {
    symbol: String,
    blocks: Vec<LirBlockId>,
}

impl LirFunction {
    pub fn symbol(&self) -> &str { &self.symbol }
    pub fn blocks(&self) -> &[LirBlockId] { &self.blocks }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LirModule {
    functions: Vec<LirFunction>,
    blocks: Vec<LirBlock>,
    instructions: Vec<LirInst>,
    registers: Vec<RegisterClass>,
}

impl LirModule {
    pub fn new() -> Self { Self::default() }

    pub fn push_function(&mut self, symbol: String) -> LirFunctionId {
        let id = LirFunctionId::new(self.functions.len() as u32);
        self.functions.push(LirFunction { symbol, blocks: Vec::new() });
        id
    }

    pub fn push_block(&mut self, function: LirFunctionId, name: String) -> LirBlockId {
        let id = LirBlockId::new(self.blocks.len() as u32);
        self.blocks.push(LirBlock { name, instructions: Vec::new() });
        self.functions[function.raw() as usize].blocks.push(id);
        id
    }

    pub fn push_vreg(&mut self, class: RegisterClass) -> VirtualReg {
        let id = VirtualReg::new(self.registers.len() as u32);
        self.registers.push(class);
        id
    }

    pub fn push_inst(&mut self, block: LirBlockId, inst: LirInst) -> LirInstId {
        let id = LirInstId::new(self.instructions.len() as u32);
        self.instructions.push(inst);
        self.blocks[block.raw() as usize].instructions.push(id);
        id
    }

    pub fn block(&self, id: LirBlockId) -> &LirBlock { &self.blocks[id.raw() as usize] }
    pub fn inst(&self, id: LirInstId) -> &LirInst { &self.instructions[id.raw() as usize] }
    pub fn functions(&self) -> &[LirFunction] { &self.functions }
    pub fn blocks(&self) -> &[LirBlock] { &self.blocks }
    pub fn instructions(&self) -> &[LirInst] { &self.instructions }
    pub fn registers(&self) -> &[RegisterClass] { &self.registers }
}
```

- [ ] **Step 4: Export LIR module**

Modify `src/mir/mod.rs`:

```rust
pub mod lir;

pub use lir::{
    LirBlockId, LirFunctionId, LirInst, LirInstId, LirModule, LirOpcode, LirOperand,
    MemoryOrder, RegisterClass, SideEffectClass, TargetEffects, VirtualReg,
};
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test mir::lir::tests::lir_assigns_dense_virtual_registers_and_instructions
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/mod.rs src/mir/lir.rs
git commit -m "feat: add LIR core model -Codex Automated"
```

**Acceptance criteria:**
- LIR stores functions, blocks, instructions, and virtual registers in deterministic dense order.
- Target metadata represents traps, volatile operations, memory order, and side-effect class.

---

### Task 2: LIR Verifier And ABI Metadata

**Files:**
- Create: `src/mir/lir_verify.rs`
- Modify: `src/mir/lir.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/lir_verify.rs`

**Description:** Lock the LIR invariants before lowering: every function parameter is a virtual register, every instruction operand/result references a known virtual register, return instructions are terminators, and unsupported ABI shapes are rejected before assembly emission.

- [ ] **Step 1: Write failing LIR verifier tests**

Create `src/mir/lir_verify.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::verify_lir;
    use crate::mir::{
        LirInst, LirModule, LirOpcode, LirOperand, RegisterClass, TargetEffects, VirtualReg,
    };

    #[test]
    fn verifier_rejects_unknown_virtual_registers() {
        let mut module = LirModule::new();
        let function = module.push_function("root.F.run".to_string());
        let block = module.push_block(function, "entry".to_string());
        module.push_inst(
            block,
            LirInst::new(
                LirOpcode::Ret,
                vec![LirOperand::Reg(VirtualReg::new(99))],
                Vec::new(),
                TargetEffects::pure(),
            ),
        );

        let result = verify_lir(&module);
        assert!(!result.ok());
        assert!(result.messages()[0].contains("unknown vreg v99"));
    }

    #[test]
    fn verifier_accepts_argument_return() {
        let mut module = LirModule::new();
        let function = module.push_function("root.F.id".to_string());
        let block = module.push_block(function, "entry".to_string());
        let arg = module.push_vreg(RegisterClass::Gp64);
        module.push_function_param(function, arg);
        module.push_inst(
            block,
            LirInst::new(
                LirOpcode::Ret,
                vec![LirOperand::Reg(arg)],
                Vec::new(),
                TargetEffects::pure(),
            ),
        );

        assert!(verify_lir(&module).ok());
    }
}
```

- [ ] **Step 2: Add parameter metadata to LIR**

Modify `LirFunction` in `src/mir/lir.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirFunction {
    symbol: String,
    params: Vec<VirtualReg>,
    blocks: Vec<LirBlockId>,
}

impl LirFunction {
    pub fn symbol(&self) -> &str { &self.symbol }
    pub fn params(&self) -> &[VirtualReg] { &self.params }
    pub fn blocks(&self) -> &[LirBlockId] { &self.blocks }
}
```

Update `push_function` and add `push_function_param`:

```rust
pub fn push_function(&mut self, symbol: String) -> LirFunctionId {
    let id = LirFunctionId::new(self.functions.len() as u32);
    self.functions.push(LirFunction { symbol, params: Vec::new(), blocks: Vec::new() });
    id
}

pub fn push_function_param(&mut self, function: LirFunctionId, reg: VirtualReg) {
    self.functions[function.raw() as usize].params.push(reg);
}
```

- [ ] **Step 3: Implement verifier**

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LirVerifyResult {
    messages: Vec<String>,
}

impl LirVerifyResult {
    pub fn ok(&self) -> bool { self.messages.is_empty() }
    pub fn messages(&self) -> &[String] { &self.messages }
    fn push(&mut self, message: impl Into<String>) { self.messages.push(message.into()); }
}

pub fn verify_lir(module: &LirModule) -> LirVerifyResult {
    let mut result = LirVerifyResult::default();
    let reg_count = module.registers().len() as u32;

    for function in module.functions() {
        if function.params().len() > 8 {
            result.push(format!("function {} has more than 8 ABI parameters", function.symbol()));
        }
        for param in function.params() {
            if param.raw() >= reg_count {
                result.push(format!("unknown parameter vreg v{}", param.raw()));
            }
        }
    }

    for block in module.blocks() {
        let insts = block.instructions();
        if insts.is_empty() {
            result.push(format!("block {} has no terminator", block.name()));
            continue;
        }
        for (index, inst_id) in insts.iter().enumerate() {
            let inst = module.instruction(*inst_id);
            if inst.opcode() == LirOpcode::Ret && index + 1 != insts.len() {
                result.push(format!("block {} has instructions after ret", block.name()));
            }
        }
        let last = module.instruction(*insts.last().expect("checked non-empty"));
        if last.opcode() != LirOpcode::Ret {
            result.push(format!("block {} does not end with ret", block.name()));
        }
    }

    for inst in module.instructions() {
        for operand in inst.operands() {
            if let LirOperand::Reg(reg) = operand {
                if reg.raw() >= reg_count {
                    result.push(format!("unknown vreg v{}", reg.raw()));
                }
            }
        }
        for reg in inst.results() {
            if reg.raw() >= reg_count {
                result.push(format!("unknown result vreg v{}", reg.raw()));
            }
        }
    }

    result
}
```

- [ ] **Step 4: Export verifier**

```rust
pub mod lir_verify;
pub use lir_verify::{LirVerifyResult, verify_lir};
```

- [ ] **Step 5: Run focused tests**

```bash
cargo test mir::lir_verify::tests
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/lir.rs src/mir/lir_verify.rs src/mir/mod.rs
git commit -m "feat: verify LIR invariants -Codex Automated"
```

**Acceptance criteria:**
- LIR functions carry ABI parameter virtual registers.
- LIR verifier rejects unknown virtual registers.
- LIR verifier rejects functions with more than 8 ABI register parameters.
- LIR verifier rejects empty blocks, blocks without a final `Ret`, and instructions after `Ret`.

---

### Task 3: Direct W-MIR To LIR Lowering

**Files:**
- Create: `src/mir/lower.rs`
- Modify: `src/mir/mod.rs`
- Test: `tests/mir_codegen.rs`

**Description:** Lower MIR 01 operations to simple LIR instructions. This task handles the current checked subset: literals, read values, binary add, let, and return.

- [ ] **Step 1: Write failing lowering test**

Create `tests/mir_codegen.rs`:

```rust
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn lower_data_flow_fixture_to_lir() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());

    let lir = wrela::mir::lower_to_lir(mir.module().unwrap());

    assert_eq!(lir.functions().len(), 1);
    assert!(lir.instructions().iter().any(|inst| inst.opcode() == wrela::mir::LirOpcode::Add));
    assert!(lir.instructions().iter().any(|inst| inst.opcode() == wrela::mir::LirOpcode::Ret));
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test mir_codegen lower_data_flow_fixture_to_lir
```

Expected: fail because `lower_to_lir` does not exist.

- [ ] **Step 3: Implement lowering**

Create `src/mir/lower.rs`:

```rust
use std::collections::BTreeMap;

use super::{
    LirInst, LirModule, LirOpcode, LirOperand, MirModule, OperationKind, RegisterClass,
    TargetEffects, ValueId, VirtualReg,
};

pub fn lower_to_lir(module: &MirModule) -> LirModule {
    let mut lowerer = Lowerer {
        lir: LirModule::new(),
        values: BTreeMap::new(),
    };
    lowerer.lower_module(module);
    lowerer.lir
}

struct Lowerer {
    lir: LirModule,
    values: BTreeMap<ValueId, VirtualReg>,
}

impl Lowerer {
    fn lower_module(&mut self, module: &MirModule) {
        for region in module.regions() {
            let symbol = match region.kind() {
                super::RegionKind::Lambda { symbol } => symbol.clone(),
                _ => continue,
            };
            self.values.clear();
            assert_eq!(
                region.blocks().len(),
                1,
                "MIR 02 lower_to_lir supports single-block lambda regions; Gamma/Theta lowering belongs in a later plan"
            );
            let function = self.lir.push_function(symbol);
            for block_id in region.blocks() {
                let block_data = module.block(*block_id);
                let lir_block = self.lir.push_block(function, block_data.name().to_string());
                for arg in block_data.arguments() {
                    let reg = self.define_value(*arg);
                    self.lir.push_function_param(function, reg);
                }
                for op_id in block_data.operations() {
                    self.lower_operation(module, lir_block, *op_id);
                }
            }
        }
    }

    fn lower_operation(
        &mut self,
        module: &MirModule,
        block: super::LirBlockId,
        op_id: super::OperationId,
    ) {
        let op = module.operation(op_id);
        match op.kind() {
            OperationKind::Literal(text) => {
                if let Some(result) = op.results().first().copied() {
                    let reg = self.define_value(result);
                    let imm = text.parse::<i64>().unwrap_or(0);
                    self.lir.push_inst(
                        block,
                        LirInst::new(
                            LirOpcode::Mov,
                            vec![LirOperand::ImmI64(imm)],
                            vec![reg],
                            TargetEffects::pure(),
                        ),
                    );
                }
            }
            OperationKind::ReadValue(_) => {
                if let Some(result) = op.results().first().copied() {
                    self.define_value(result);
                }
            }
            OperationKind::Binary(_) => {
                if op.operands().len() == 2 {
                    let left = self.use_value(op.operands()[0]);
                    let right = self.use_value(op.operands()[1]);
                    let out = self.define_value(op.results()[0]);
                    self.lir.push_inst(
                        block,
                        LirInst::new(
                            LirOpcode::Add,
                            vec![LirOperand::Reg(left), LirOperand::Reg(right)],
                            vec![out],
                            TargetEffects::pure(),
                        ),
                    );
                }
            }
            OperationKind::Let(_) => {}
            OperationKind::Return => {
                let operands = op
                    .operands()
                    .iter()
                    .map(|value| LirOperand::Reg(self.use_value(*value)))
                    .collect::<Vec<_>>();
                self.lir.push_inst(
                    block,
                    LirInst::new(LirOpcode::Ret, operands, Vec::new(), TargetEffects::pure()),
                );
            }
        }
    }

    fn define_value(&mut self, value: ValueId) -> VirtualReg {
        let reg = self.lir.push_vreg(RegisterClass::Gp64);
        self.values.insert(value, reg);
        reg
    }

    fn use_value(&mut self, value: ValueId) -> VirtualReg {
        if let Some(reg) = self.values.get(&value).copied() {
            return reg;
        }
        self.define_value(value)
    }
}
```

- [ ] **Step 4: Export lowering**

Modify `src/mir/mod.rs`:

```rust
pub mod lower;

pub use lower::lower_to_lir;
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_codegen lower_data_flow_fixture_to_lir
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/mod.rs src/mir/lower.rs tests/mir_codegen.rs
git commit -m "feat: lower MIR to LIR -Codex Automated"
```

**Acceptance criteria:**
- Current checked expression subset lowers to LIR.
- LIR output is deterministic for the same W-MIR input.

---

### Task 4: Single-Pass Register Allocation And AArch64 Assembly Emission

**Files:**
- Create: `src/mir/regalloc.rs`
- Create: `src/mir/emit.rs`
- Modify: `src/mir/mod.rs`
- Test: `tests/mir_codegen.rs`

**Description:** Map LIR virtual registers to a fixed development register pool and emit deterministic AArch64 assembly text.

- [ ] **Step 1: Add failing assembly test**

Append to `tests/mir_codegen.rs`:

```rust
#[test]
fn emit_aarch64_for_data_flow_fixture() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());
    let mir = wrela::mir::build_mir(&check);
    let lir = wrela::mir::lower_to_lir(mir.module().unwrap());
    let allocated = wrela::mir::regalloc::allocate_registers(&lir).unwrap();
    let asm = wrela::mir::emit::emit_aarch64(&allocated);

    assert!(asm.contains(".text"));
    assert!(asm.contains("_app_flow_Calculator_add:"));
    assert!(asm.contains("add "));
    assert!(asm.contains("ret"));
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test mir_codegen emit_aarch64_for_data_flow_fixture
```

Expected: fail because regalloc and emitter do not exist.

- [ ] **Step 3: Implement single-pass register allocation**

Create `src/mir/regalloc.rs`:

```rust
use std::collections::BTreeMap;

use super::{LirModule, RegisterClass, VirtualReg};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocatedProgram {
    lir: LirModule,
    registers: BTreeMap<VirtualReg, PhysicalReg>,
}

impl AllocatedProgram {
    pub fn lir(&self) -> &LirModule { &self.lir }
    pub fn physical(&self, reg: VirtualReg) -> Option<PhysicalReg> {
        self.registers.get(&reg).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalReg {
    X(u8),
    W(u8),
    V(u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegallocError {
    message: String,
}

impl RegallocError {
    pub fn message(&self) -> &str { &self.message }
    fn new(message: impl Into<String>) -> Self { Self { message: message.into() } }
}

pub fn allocate_registers(lir: &LirModule) -> Result<AllocatedProgram, RegallocError> {
    let mut registers = BTreeMap::new();
    let mut next_gp = 9u8;
    let mut next_vec = 0u8;

    for function in lir.functions() {
        if function.params().len() > 8 {
            return Err(RegallocError::new(format!(
                "function {} has more than 8 ABI register parameters",
                function.symbol()
            )));
        }
        for (index, param) in function.params().iter().enumerate() {
            registers.insert(*param, PhysicalReg::X(index as u8));
        }
    }

    for (index, class) in lir.registers().iter().enumerate() {
        let vreg = VirtualReg::new(index as u32);
        if registers.contains_key(&vreg) {
            continue;
        }
        let physical = match class {
            RegisterClass::Gp64 => {
                if next_gp > 15 {
                    return Err(RegallocError::new("dev regalloc exhausted gp registers"));
                }
                let reg = PhysicalReg::X(next_gp);
                next_gp += 1;
                reg
            }
            RegisterClass::Gp32 => {
                if next_gp > 15 {
                    return Err(RegallocError::new("dev regalloc exhausted gp registers"));
                }
                let reg = PhysicalReg::W(next_gp);
                next_gp += 1;
                reg
            }
            RegisterClass::Vec128 => {
                if next_vec > 15 {
                    return Err(RegallocError::new("dev regalloc exhausted vector registers"));
                }
                let reg = PhysicalReg::V(next_vec);
                next_vec += 1;
                reg
            }
        };
        registers.insert(vreg, physical);
    }

    Ok(AllocatedProgram {
        lir: lir.clone(),
        registers,
    })
}
```

- [ ] **Step 4: Implement AArch64 emitter**

Create `src/mir/emit.rs`:

```rust
use std::fmt::Write;

use super::lir::{LirOpcode, LirOperand};
use super::regalloc::{AllocatedProgram, PhysicalReg};

pub fn emit_aarch64(program: &AllocatedProgram) -> String {
    let mut out = String::new();
    out.push_str(".text\n");
    for function in program.lir().functions() {
        let symbol = assembly_symbol(function.symbol());
        let _ = writeln!(out, ".global {symbol}");
        let _ = writeln!(out, "{symbol}:");
        for block in function.blocks() {
            for inst_id in program.lir().block(*block).instructions() {
                let inst = program.lir().inst(*inst_id);
                match inst.opcode() {
                    LirOpcode::Mov => {
                        if let (Some(LirOperand::ImmI64(value)), Some(result)) =
                            (inst.operands().first(), inst.results().first())
                        {
                            let dst = reg_name(program.physical(*result));
                            let _ = writeln!(out, "  mov {dst}, #{value}");
                        }
                    }
                    LirOpcode::Add => {
                        if let [LirOperand::Reg(left), LirOperand::Reg(right)] = inst.operands() {
                            let dst = reg_name(program.physical(inst.results()[0]));
                            let lhs = reg_name(program.physical(*left));
                            let rhs = reg_name(program.physical(*right));
                            let _ = writeln!(out, "  add {dst}, {lhs}, {rhs}");
                        }
                    }
                    LirOpcode::Ret => {
                        if let Some(LirOperand::Reg(value)) = inst.operands().first() {
                            let src = reg_name(program.physical(*value));
                            if src != "x0" {
                                let _ = writeln!(out, "  mov x0, {src}");
                            }
                        }
                        out.push_str("  ret\n");
                    }
                }
            }
        }
    }
    out
}

pub fn assembly_symbol(function_symbol: &str) -> String {
    format!("_{}", function_symbol.replace('.', "_"))
}

fn reg_name(reg: Option<PhysicalReg>) -> String {
    match reg.unwrap_or(PhysicalReg::X(0)) {
        PhysicalReg::X(index) => format!("x{index}"),
        PhysicalReg::W(index) => format!("w{index}"),
        PhysicalReg::V(index) => format!("v{index}.16b"),
    }
}
```

Use `assembly_symbol(function.symbol())` everywhere an emitted function name is needed. Perf tooling must call the same function instead of parsing assembly text.

- [ ] **Step 5: Export regalloc and emit**

Modify `src/mir/mod.rs`:

```rust
pub mod emit;
pub mod regalloc;
```

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test --test mir_codegen emit_aarch64_for_data_flow_fixture
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/regalloc.rs src/mir/emit.rs tests/mir_codegen.rs
git commit -m "feat: emit dev AArch64 assembly -Codex Automated"
```

**Acceptance criteria:**
- LIR virtual registers map deterministically to physical registers.
- Assembly output contains stable symbols and instructions.
- Emitter preserves target-level metadata in data structures even if this first assembly path does not schedule.

---

### Task 5: Dev Codegen CLI Surface

**Files:**
- Modify: `src/command.rs`
- Test: `tests/mir_codegen.rs`
- Unit test: `src/command.rs`

**Description:** Add a dev codegen inspection command without introducing `wrela build` yet. The command is `wrela dump asm <root.wrela>`.

- [ ] **Step 1: Add failing CLI tests**

Append to `tests/mir_codegen.rs`:

```rust
#[test]
fn dump_asm_command_prints_aarch64() {
    let root = fixture("data_flow.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "dump".to_string(),
            "asm".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let asm = String::from_utf8(out).unwrap();
    assert!(asm.contains(".text"));
    assert!(asm.contains("ret"));
}
```

Add to `src/command.rs` tests:

```rust
#[test]
fn help_lists_asm_dump_command() {
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = run_with_io(vec!["wrela".to_string(), "help".to_string()], &mut out, &mut err);

    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("wrela dump asm <root.wrela>")
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test mir_codegen dump_asm_command_prints_aarch64
cargo test command::tests::help_lists_asm_dump_command
```

Expected: fail because CLI command is not wired.

- [ ] **Step 3: Add `dump asm` branch**

Modify help output in `src/command.rs` to include:

```rust
let _ = writeln!(out, "wrela dump asm <root.wrela>");
```

Add `Some("asm")` to the `dump` subcommand branch and implement:

```rust
fn dump_asm<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let check = crate::check::check_root(path);
    if !check.ok() {
        print_diagnostics(out, check.diagnostics());
        return 1;
    }
    let mir = crate::mir::build_mir(&check);
    if !mir.ok() {
        print_diagnostics(out, mir.diagnostics());
        return 1;
    }
    let lir = crate::mir::lower_to_lir(mir.module().expect("ok MIR has module"));
    let lir_check = crate::mir::verify_lir(&lir);
    if !lir_check.ok() {
        for message in lir_check.messages() {
            let _ = writeln!(err, "{message}");
        }
        return 1;
    }
    let allocated = match crate::mir::regalloc::allocate_registers(&lir) {
        Ok(allocated) => allocated,
        Err(error) => {
            let _ = writeln!(err, "{}", error.message());
            return 1;
        }
    };
    let asm = crate::mir::emit::emit_aarch64(&allocated);
    let _ = write!(out, "{asm}");
    0
}
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test --test mir_codegen dump_asm_command_prints_aarch64
cargo test command::tests::help_lists_asm_dump_command
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/command.rs tests/mir_codegen.rs
git commit -m "feat: add assembly dump command -Codex Automated"
```

**Acceptance criteria:**
- `wrela dump asm <root.wrela>` emits deterministic AArch64 assembly for valid checked source.
- Invalid source prints check diagnostics and exits `1`.
- No `wrela build` command is added in MIR 02.

---

### Task 6: Compiler Telemetry With `wrela perf compile`

**Files:**
- Create: `src/mir/perf.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/command.rs`
- Test: `tests/mir_perf.rs`

**Description:** Add lightweight phase timing and counter reporting for compile pipeline telemetry. The output has human and JSON forms.

- [ ] **Step 1: Write failing perf compile test**

Create `tests/mir_perf.rs`:

```rust
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn perf_compile_json_reports_phase_and_counters() {
    let root = fixture("data_flow.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "compile".to_string(),
            "--mode".to_string(),
            "dev".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            "--json".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"schema\":\"wrela.perf.compile.v1\""));
    assert!(json.contains("\"mode\":\"dev\""));
    assert!(json.contains("\"releasePipeline\":\"dev\""));
    assert!(json.contains("\"mirBuild\""));
    assert!(json.contains("\"mirCounters\""));
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test mir_perf perf_compile_json_reports_phase_and_counters
```

Expected: fail because perf command does not exist.

- [ ] **Step 3: Implement compile perf report**

Create `src/mir/perf.rs`:

```rust
use std::fmt::Write;
use std::time::{Duration, Instant};

use crate::check::check_root;

use super::{MirReport, build_mir, emit, lower_to_lir, regalloc, verify_lir};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerfMode {
    Dev,
    Release,
}

#[derive(Clone, Debug)]
pub struct CompilePerfReport {
    mode: PerfMode,
    repeats: usize,
    mir_build_nanos: Vec<u128>,
    lower_nanos: Vec<u128>,
    emit_nanos: Vec<u128>,
    mir_report: MirReport,
}

pub fn measure_compile(path: &str, mode: PerfMode, repeats: usize) -> Result<CompilePerfReport, String> {
    let repeats = repeats.max(1);
    let mut mir_build_nanos = Vec::new();
    let mut lower_nanos = Vec::new();
    let mut emit_nanos = Vec::new();
    let mut mir_report = MirReport::default();

    for _ in 0..repeats {
        let check = check_root(path);
        if !check.ok() {
            return Err("check failed before compile benchmark".to_string());
        }

        let start = Instant::now();
        let mir = build_mir(&check);
        mir_build_nanos.push(elapsed_nanos(start.elapsed()));
        if !mir.ok() {
            return Err("MIR build failed before compile benchmark".to_string());
        }
        mir_report = mir.report().clone();

        let start = Instant::now();
        let lir = lower_to_lir(mir.module().expect("ok MIR has module"));
        let lir_check = verify_lir(&lir);
        if !lir_check.ok() {
            return Err(format!("LIR verification failed: {:?}", lir_check.messages()));
        }
        lower_nanos.push(elapsed_nanos(start.elapsed()));

        let start = Instant::now();
        let allocated = regalloc::allocate_registers(&lir).map_err(|err| err.message().to_string())?;
        let _asm = emit::emit_aarch64(&allocated);
        emit_nanos.push(elapsed_nanos(start.elapsed()));
    }

    Ok(CompilePerfReport {
        mode,
        repeats,
        mir_build_nanos,
        lower_nanos,
        emit_nanos,
        mir_report,
    })
}

pub fn render_compile_json(report: &CompilePerfReport) -> String {
    format!(
        "{{\"schema\":\"wrela.perf.compile.v1\",\"mode\":\"{}\",\"releasePipeline\":\"{}\",\"repeats\":{},\"mirBuild\":{},\"lower\":{},\"emit\":{},\"mirCounters\":{{\"regions\":{},\"blocks\":{},\"operations\":{},\"values\":{},\"places\":{}}}}}\n",
        mode_name(report.mode),
        release_pipeline_name(report.mode),
        report.repeats,
        median(&report.mir_build_nanos),
        median(&report.lower_nanos),
        median(&report.emit_nanos),
        report.mir_report.regions(),
        report.mir_report.blocks(),
        report.mir_report.operations(),
        report.mir_report.values(),
        report.mir_report.places(),
    )
}

pub fn render_compile_human(report: &CompilePerfReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "wrela perf compile ({})", mode_name(report.mode));
    let _ = writeln!(out, "repeats: {}", report.repeats);
    let _ = writeln!(out, "mir.build median ns: {}", median(&report.mir_build_nanos));
    let _ = writeln!(out, "lower median ns: {}", median(&report.lower_nanos));
    let _ = writeln!(out, "emit median ns: {}", median(&report.emit_nanos));
    out
}

fn mode_name(mode: PerfMode) -> &'static str {
    match mode {
        PerfMode::Dev => "dev",
        PerfMode::Release => "release",
    }
}

fn release_pipeline_name(_mode: PerfMode) -> &'static str {
    "dev"
}

fn elapsed_nanos(duration: Duration) -> u128 {
    duration.as_nanos()
}

fn median(values: &[u128]) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}
```

- [ ] **Step 4: Export perf module**

Modify `src/mir/mod.rs`:

```rust
pub mod perf;
```

- [ ] **Step 5: Wire CLI `perf compile`**

In `src/command.rs`, add a top-level `Some("perf")` branch:

```rust
Some("perf") => run_perf_command(&collected, out, err),
```

Add parser:

```rust
fn run_perf_command<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    match args.get(2).map(String::as_str) {
        Some("compile") => run_perf_compile(args, out, err),
        _ => {
            let _ = writeln!(err, "malformed command");
            2
        }
    }
}
```

Implement `run_perf_compile` with explicit flags `--mode dev|release`, `--repeat N`, `--json`. Unknown flags exit `2`. Release mode is accepted in MIR 02 but internally uses the dev pipeline until MIR 03 changes it. Compile benchmark failures return `1` with the error message on stderr:

```rust
fn run_perf_compile<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let parsed = match parse_perf_compile_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let report = match crate::mir::perf::measure_compile(
        &parsed.path,
        parsed.mode,
        parsed.repeats,
    ) {
        Ok(report) => report,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    let rendered = if parsed.json {
        crate::mir::perf::render_compile_json(&report)
    } else {
        crate::mir::perf::render_compile_human(&report)
    };
    let _ = write!(out, "{rendered}");
    0
}
```

The release-mode dev-pipeline behavior must be documented in JSON as:

```json
"mode":"release",
"releasePipeline":"dev"
```

Do not add speculative comments in production code; this field is the explicit machine-readable contract for the MIR 02 behavior.

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test --test mir_perf perf_compile_json_reports_phase_and_counters
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/perf.rs src/command.rs tests/mir_perf.rs
git commit -m "feat: add compile perf telemetry -Codex Automated"
```

**Acceptance criteria:**
- `wrela perf compile` reports phase timing and MIR counters.
- JSON output is machine-readable and stable in field names.
- No perf command is part of the default quality gate.

---

### Task 7: Generated-Code Smoke Benchmark With `wrela perf code`

**Files:**
- Create: `fixtures/perf/scalar_const.wrela`
- Modify: `src/mir/perf.rs`
- Modify: `src/command.rs`
- Test: `tests/mir_perf.rs`

**Description:** Add a deterministic generated-code smoke benchmark command. On supported hosts, the command writes emitted AArch64 assembly plus a tiny C harness to a temporary directory, links it with `cc`, executes the generated function, checks the printed checksum, and reports runtime. On unsupported hosts, it exits `2` with a clear message.

- [ ] **Step 1: Create perf fixture**

Create `fixtures/perf/scalar_const.wrela`:

```wrela
module perf.scalar_const

pub class ConstBench {
    fn run(read self) -> U32 {
        return 3
    }
}
```

- [ ] **Step 2: Add failing perf code test**

Append to `tests/mir_perf.rs`:

```rust
#[test]
fn perf_code_json_reports_checksum_or_unsupported_host() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("perf")
        .join("scalar_const.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "code".to_string(),
            "--mode".to_string(),
            "dev".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            "--json".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert!(code == 0 || code == 2);
    if code == 0 {
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"schema\":\"wrela.perf.code.v1\""));
        assert!(json.contains("\"checksum\""));
        assert!(json.contains("\"runtime\""));
    } else {
        assert!(out.is_empty());
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("generated-code execution is not supported")
        );
    }
}
```

- [ ] **Step 3: Run test and verify failure**

Run:

```bash
cargo test --test mir_perf perf_code_json_reports_checksum_or_unsupported_host
```

Expected: fail because `perf code` is not wired.

- [ ] **Step 4: Implement code perf report**

In `src/mir/perf.rs`, add:

```rust
#[derive(Clone, Debug)]
pub struct CodePerfReport {
    mode: PerfMode,
    repeats: usize,
    runtime_nanos: Vec<u128>,
    checksum: u64,
    emitted_bytes: usize,
    lir_instructions: usize,
}

pub fn measure_code(path: &str, mode: PerfMode, repeats: usize) -> Result<CodePerfReport, String> {
    if !(std::env::consts::ARCH == "aarch64" && std::env::consts::OS == "macos") {
        return Err("generated-code execution is not supported on this host".to_string());
    }

    let check = check_root(path);
    if !check.ok() {
        return Err("check failed before generated-code benchmark".to_string());
    }
    let mir = build_mir(&check);
    if !mir.ok() {
        return Err("MIR build failed before generated-code benchmark".to_string());
    }
    let lir = lower_to_lir(mir.module().expect("ok MIR has module"));
    let lir_check = verify_lir(&lir);
    if !lir_check.ok() {
        return Err(format!("LIR verification failed: {:?}", lir_check.messages()));
    }
    let allocated = regalloc::allocate_registers(&lir).map_err(|err| err.message().to_string())?;
    let asm = emit::emit_aarch64(&allocated);
    let function = lir
        .functions()
        .first()
        .ok_or_else(|| "generated-code benchmark has no function".to_string())?;
    if !function.params().is_empty() {
        return Err("generated-code execution is not supported for parameterized benchmarks in MIR 02".to_string());
    }
    let symbol = emit::assembly_symbol(function.symbol());

    let temp_dir = std::env::temp_dir().join(format!(
        "wrela-perf-code-{}-{}",
        std::process::id(),
        unique_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
    let asm_path = temp_dir.join("bench.s");
    let harness_path = temp_dir.join("harness.c");
    let exe_path = temp_dir.join("bench");
    std::fs::write(&asm_path, asm.as_bytes()).map_err(|err| err.to_string())?;
    std::fs::write(&harness_path, harness_source(&symbol).as_bytes())
        .map_err(|err| err.to_string())?;

    let compile = std::process::Command::new("cc")
        .arg("-O2")
        .arg(&harness_path)
        .arg(&asm_path)
        .arg("-o")
        .arg(&exe_path)
        .output()
        .map_err(|err| format!("generated-code execution is not supported: {err}"))?;
    if !compile.status.success() {
        return Err("generated-code execution is not supported: cc failed".to_string());
    }

    let repeats = repeats.max(1);
    let mut runtime_nanos = Vec::new();
    let mut checksum = 0u64;
    for _ in 0..repeats {
        let start = Instant::now();
        let output = std::process::Command::new(&exe_path)
            .output()
            .map_err(|err| format!("generated-code execution is not supported: {err}"))?;
        if !output.status.success() {
            return Err("generated-code benchmark process failed".to_string());
        }
        checksum = parse_checksum(&output.stdout)?;
        runtime_nanos.push(elapsed_nanos(start.elapsed()));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(CodePerfReport {
        mode,
        repeats,
        runtime_nanos,
        checksum,
        emitted_bytes: asm.len(),
        lir_instructions: lir.instructions().len(),
    })
}

pub fn render_code_json(report: &CodePerfReport) -> String {
    format!(
        "{{\"schema\":\"wrela.perf.code.v1\",\"mode\":\"{}\",\"repeats\":{},\"runtime\":{},\"checksum\":{},\"emittedBytes\":{},\"lirInstructions\":{}}}\n",
        mode_name(report.mode),
        report.repeats,
        median(&report.runtime_nanos),
        report.checksum,
        report.emitted_bytes,
        report.lir_instructions,
    )
}

fn harness_source(symbol: &str) -> String {
    format!(
        "#include <stdint.h>\n#include <stdio.h>\nextern uint64_t {symbol}(void);\nint main(void) {{ uint64_t checksum = 0; for (uint64_t i = 0; i < 100000; i++) {{ checksum += {symbol}(); }} printf(\"%llu\\n\", (unsigned long long)checksum); return 0; }}\n"
    )
}

fn parse_checksum(bytes: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "benchmark checksum was not utf-8")?;
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("benchmark checksum was not an unsigned decimal integer".to_string());
    }
    trimmed
        .parse::<u64>()
        .map_err(|_| "benchmark checksum overflowed u64".to_string())
}

fn unique_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}
```

`parse_checksum` accepts only ASCII decimal output followed by optional whitespace. `unique_nanos` is used only for temporary directory uniqueness, not deterministic compiler output.

- [ ] **Step 5: Wire CLI `perf code`**

Extend `run_perf_command` in `src/command.rs`:

```rust
Some("code") => run_perf_code(args, out, err),
```

`run_perf_code` accepts the same flags as `perf compile`. On `measure_code` error, write the error string to stderr and exit `2` for unsupported execution, `1` for failed compilation.

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test --test mir_perf perf_code_json_reports_checksum_or_unsupported_host
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add fixtures/perf/scalar_const.wrela src/mir/perf.rs src/command.rs tests/mir_perf.rs
git commit -m "feat: add generated-code perf smoke command -Codex Automated"
```

**Acceptance criteria:**
- `wrela perf code` has stable JSON fields.
- On supported hosts, it compiles and executes emitted AArch64 assembly through a tiny harness.
- It checks generated-code checksum before runtime comparisons count.
- It exits clearly on unsupported hosts.
- It does not claim release optimizer A/B support.

---

### Task 8: Perf Scripts And Current-State Docs

**Files:**
- Create: `scripts/perf-smoke.sh`
- Create: `scripts/perf-compare.sh`
- Modify: `docs/design/compiler-pipeline.md`
- Modify: `docs/design/supported-wrela-subset.md`

**Description:** Add intentional perf scripts outside the default quality gate and document the MIR 02 command surface.

- [ ] **Step 1: Add perf smoke script**

Create `scripts/perf-smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo run -- perf compile --mode dev --repeat 1 --json fixtures/mir/data_flow.wrela
cargo run -- perf code --mode dev --repeat 1 --json fixtures/perf/scalar_const.wrela || true
```

Make executable:

```bash
chmod +x scripts/perf-smoke.sh
```

- [ ] **Step 2: Add perf compare script**

Create `scripts/perf-compare.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ROOT_FILE="${1:-fixtures/mir/data_flow.wrela}"
REPEAT="${2:-5}"

cargo run -- perf compile --mode dev --repeat "$REPEAT" --json "$ROOT_FILE"
```

Make executable:

```bash
chmod +x scripts/perf-compare.sh
```

- [ ] **Step 3: Update current-state docs**

Add commands to `docs/design/supported-wrela-subset.md`:

```markdown
cargo run -- dump asm fixtures/mir/data_flow.wrela
cargo run -- perf compile --mode dev --repeat 1 --json fixtures/mir/data_flow.wrela
cargo run -- perf code --mode dev --repeat 1 --json fixtures/perf/scalar_const.wrela
```

Add pipeline rows to `docs/design/compiler-pipeline.md`:

```markdown
| `wrela dump asm <root.wrela>` | check -> W-MIR -> LIR -> regalloc -> AArch64 assembly |
| `wrela perf compile <root.wrela>` | compile pipeline telemetry |
| `wrela perf code <bench-root.wrela>` | generated-code smoke telemetry with checksum |
```

- [ ] **Step 4: Run scripts**

Run:

```bash
./scripts/perf-smoke.sh
./scripts/perf-compare.sh fixtures/mir/data_flow.wrela 1
```

Expected:
- `perf compile` exits `0`.
- `perf code` exits `0` on supported AArch64 macOS hosts or `2` with an unsupported-host message elsewhere. The smoke script tolerates that unsupported host path.

- [ ] **Step 5: Commit**

```bash
git add scripts/perf-smoke.sh scripts/perf-compare.sh docs/design/compiler-pipeline.md docs/design/supported-wrela-subset.md
git commit -m "docs: document MIR dev codegen tooling -Codex Automated"
```

**Acceptance criteria:**
- Perf scripts are opt-in and do not modify `quality-gate.sh`.
- Docs list MIR 02 command surfaces.

---

### Task 9: MIR 02 Final Quality Gate And Phase A Handoff

**Files:**
- No production file edits unless verification reveals a bug.

**Description:** Prove MIR 02 before handoff.

- [ ] **Step 1: Run full local quality gate**

Run:

```bash
./scripts/quality-gate.sh
```

Expected: pass.

- [ ] **Step 2: Run MIR 02 smoke commands**

Run:

```bash
cargo run -- dump asm fixtures/mir/data_flow.wrela
cargo run -- perf compile --mode dev --repeat 1 --json fixtures/mir/data_flow.wrela
./scripts/perf-smoke.sh
```

Expected:
- `dump asm` prints AArch64 assembly and exits `0`.
- `perf compile` exits `0`.
- `perf-smoke.sh` exits `0`; it tolerates unsupported generated-code execution hosts.

- [ ] **Step 3: Run strict gate when ready for final merge**

Run when intended implementation changes are committed:

```bash
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

Expected: pass only when the working tree is clean.

- [ ] **Step 4: Complete Phase A self review**

Use the repository review workflow in `docs/implementation/reviews/README.md`. Fix every finding at every priority unless the verdict documents an explicit disagreement.

**Acceptance criteria:**
- `./scripts/quality-gate.sh` passes.
- MIR 02 smoke commands pass.
- Phase A verdict is APPROVED in the handoff message before user handoff.

## Self-Review Checklist

- [ ] LIR carries target-level side-effect metadata.
- [ ] Dev assembly output is deterministic.
- [ ] `wrela dump asm` does not add `wrela build`.
- [ ] `wrela perf compile` reports timings and counters.
- [ ] `wrela perf code` checks checksum or exits clearly on unsupported hosts.
- [ ] Perf scripts are opt-in.
- [ ] No external crates were added.
