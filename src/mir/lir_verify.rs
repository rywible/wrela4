use super::lir::{LirModule, LirOpcode, LirOperand};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LirVerifyResult {
    messages: Vec<String>,
}

impl LirVerifyResult {
    pub fn ok(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    fn push(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }
}

pub fn verify_lir(module: &LirModule) -> LirVerifyResult {
    let mut result = LirVerifyResult::default();
    let reg_count = module.registers().len() as u32;

    for function in module.functions() {
        if function.params().len() > 8 {
            result.push(format!(
                "function {} has more than 8 ABI parameters",
                function.symbol()
            ));
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
            let inst = module.inst(*inst_id);
            if inst.opcode() == LirOpcode::Ret && index + 1 != insts.len() {
                result.push(format!("block {} has instructions after ret", block.name()));
            }
        }
        let last = module.inst(*insts.last().expect("checked non-empty"));
        if !matches!(
            last.opcode(),
            LirOpcode::Ret
                | LirOpcode::Branch
                | LirOpcode::BranchIfZero
                | LirOpcode::BranchIfLessThan
        ) {
            result.push(format!(
                "block {} does not end with a terminator",
                block.name()
            ));
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
