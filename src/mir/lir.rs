macro_rules! lir_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }
            pub const fn raw(self) -> u32 {
                self.0
            }
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
    LoadU32,
    LoadMaskByte,
    TestMaskBit,
    AddU64,
    BranchIfZero,
    BranchIfLessThan,
    Branch,
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
        Self {
            opcode,
            operands,
            results,
            effects,
        }
    }

    pub fn opcode(&self) -> LirOpcode {
        self.opcode
    }

    pub fn operands(&self) -> &[LirOperand] {
        &self.operands
    }

    pub fn results(&self) -> &[VirtualReg] {
        &self.results
    }

    pub fn effects(&self) -> TargetEffects {
        self.effects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirBlock {
    name: String,
    instructions: Vec<LirInstId>,
}

impl LirBlock {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instructions(&self) -> &[LirInstId] {
        &self.instructions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirFunction {
    symbol: String,
    params: Vec<VirtualReg>,
    blocks: Vec<LirBlockId>,
}

impl LirFunction {
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn params(&self) -> &[VirtualReg] {
        &self.params
    }

    pub fn blocks(&self) -> &[LirBlockId] {
        &self.blocks
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LirModule {
    functions: Vec<LirFunction>,
    blocks: Vec<LirBlock>,
    instructions: Vec<LirInst>,
    registers: Vec<RegisterClass>,
}

impl LirModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_function(&mut self, symbol: String) -> LirFunctionId {
        let id = LirFunctionId::new(self.functions.len() as u32);
        self.functions.push(LirFunction {
            symbol,
            params: Vec::new(),
            blocks: Vec::new(),
        });
        id
    }

    pub fn push_function_param(&mut self, function: LirFunctionId, reg: VirtualReg) {
        self.functions[function.raw() as usize].params.push(reg);
    }

    pub fn push_block(&mut self, function: LirFunctionId, name: String) -> LirBlockId {
        let id = LirBlockId::new(self.blocks.len() as u32);
        self.blocks.push(LirBlock {
            name,
            instructions: Vec::new(),
        });
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

    pub fn block(&self, id: LirBlockId) -> &LirBlock {
        &self.blocks[id.raw() as usize]
    }

    pub fn inst(&self, id: LirInstId) -> &LirInst {
        &self.instructions[id.raw() as usize]
    }

    pub fn functions(&self) -> &[LirFunction] {
        &self.functions
    }

    pub fn blocks(&self) -> &[LirBlock] {
        &self.blocks
    }

    pub fn instructions(&self) -> &[LirInst] {
        &self.instructions
    }

    pub fn registers(&self) -> &[RegisterClass] {
        &self.registers
    }
}

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
