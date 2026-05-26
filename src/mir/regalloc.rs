use std::collections::BTreeMap;

use super::aarch64::PhysicalReg;
use super::{LirModule, RegisterClass, VirtualReg};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocatedProgram {
    lir: LirModule,
    registers: BTreeMap<VirtualReg, PhysicalReg>,
}

impl AllocatedProgram {
    pub fn lir(&self) -> &LirModule {
        &self.lir
    }

    pub fn physical(&self, reg: VirtualReg) -> Option<PhysicalReg> {
        self.registers.get(&reg).copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegallocError {
    message: String,
}

impl RegallocError {
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn allocate_registers(lir: &LirModule) -> Result<AllocatedProgram, RegallocError> {
    let mut registers = BTreeMap::new();

    for function in lir.functions() {
        if function.params().len() > 8 {
            return Err(RegallocError::new(format!(
                "function {} has more than 8 ABI register parameters",
                function.symbol()
            )));
        }
        let mut next_gp = 9u8;
        let mut next_vec = 0u8;
        for (index, param) in function.params().iter().enumerate() {
            registers.insert(*param, PhysicalReg::X(index as u8));
        }

        let mut function_vregs = Vec::new();
        for block in function.blocks() {
            for inst_id in lir.block(*block).instructions() {
                let inst = lir.inst(*inst_id);
                for operand in inst.operands() {
                    if let super::lir::LirOperand::Reg(vreg) = operand {
                        function_vregs.push(*vreg);
                    }
                }
                for result in inst.results() {
                    function_vregs.push(*result);
                }
            }
        }
        function_vregs.sort_unstable();
        function_vregs.dedup();

        for vreg in function_vregs {
            if registers.contains_key(&vreg) {
                continue;
            }
            let class = lir.registers().get(vreg.raw() as usize).ok_or_else(|| {
                RegallocError::new(format!("missing register class for v{}", vreg.raw()))
            })?;
            let physical = match class {
                RegisterClass::Gp64 => {
                    if next_gp > 21 {
                        return Err(RegallocError::new("dev regalloc exhausted gp registers"));
                    }
                    let reg = PhysicalReg::X(next_gp);
                    next_gp += 1;
                    reg
                }
                RegisterClass::Gp32 => {
                    if next_gp > 21 {
                        return Err(RegallocError::new("dev regalloc exhausted gp registers"));
                    }
                    let reg = PhysicalReg::W(next_gp);
                    next_gp += 1;
                    reg
                }
                RegisterClass::Vec128 => {
                    if next_vec > 15 {
                        return Err(RegallocError::new(
                            "dev regalloc exhausted vector registers",
                        ));
                    }
                    let reg = PhysicalReg::V(next_vec);
                    next_vec += 1;
                    reg
                }
            };
            registers.insert(vreg, physical);
        }
    }

    Ok(AllocatedProgram {
        lir: lir.clone(),
        registers,
    })
}
