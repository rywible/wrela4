use std::collections::BTreeMap;

use super::{
    LirInst, LirModule, LirOpcode, LirOperand, MirModule, OperationKind, RegisterClass,
    TargetEffects, ValueId, VirtualReg,
};

pub fn lower_to_lir(module: &MirModule) -> LirModule {
    if let Some(lir) = super::dataplane_codegen::try_lower_dataplane(module) {
        return lir;
    }
    let mut lowerer = Lowerer {
        lir: LirModule::new(),
        values: BTreeMap::new(),
        param_reads: 0,
    };
    lowerer.lower_module(module);
    lowerer.lir
}

struct Lowerer {
    lir: LirModule,
    values: BTreeMap<ValueId, VirtualReg>,
    param_reads: usize,
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
                self.param_reads = 0;
                for op_id in block_data.operations() {
                    self.lower_operation(module, lir_block, block_data, *op_id);
                }
            }
        }
    }

    fn lower_operation(
        &mut self,
        module: &MirModule,
        block: super::LirBlockId,
        block_data: &super::BlockData,
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
                let Some(result) = op.results().first().copied() else {
                    return;
                };
                let Some(arg) = block_data.arguments().get(self.param_reads) else {
                    return;
                };
                self.param_reads += 1;
                let arg_reg = self.use_value(*arg);
                let result_reg = self.define_value(result);
                if result_reg != arg_reg {
                    self.lir.push_inst(
                        block,
                        LirInst::new(
                            LirOpcode::Mov,
                            vec![LirOperand::Reg(arg_reg)],
                            vec![result_reg],
                            TargetEffects::pure(),
                        ),
                    );
                }
            }
            OperationKind::Binary(operator) => {
                if op.operands().len() != 2 {
                    return;
                }
                let opcode = match operator.as_str() {
                    "+" => LirOpcode::Add,
                    _ => return,
                };
                let left = self.use_value(op.operands()[0]);
                let right = self.use_value(op.operands()[1]);
                let out = self.define_value(op.results()[0]);
                self.lir.push_inst(
                    block,
                    LirInst::new(
                        opcode,
                        vec![LirOperand::Reg(left), LirOperand::Reg(right)],
                        vec![out],
                        TargetEffects::pure(),
                    ),
                );
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
            OperationKind::TableRows { .. }
            | OperationKind::MaskRead { .. }
            | OperationKind::MaskAllTrue { .. }
            | OperationKind::MaskAllFalse { .. }
            | OperationKind::MaskNot
            | OperationKind::MaskAnd
            | OperationKind::MaskOr
            | OperationKind::RowToken { .. }
            | OperationKind::ReduceRows { .. } => {}
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
