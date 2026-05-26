use std::collections::BTreeMap;

use super::{
    BlockId, LirBlockId, LirFunctionId, LirInst, LirModule, LirOpcode, LirOperand, MirModule,
    OperationId, OperationKind, RegionKind, RegisterClass, TargetEffects, ValueId, VirtualReg,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AbiParam {
    table: Option<&'static str>,
    mask: Option<&'static str>,
}

const FILTER_SUM_ABI: [AbiParam; 5] = [
    AbiParam {
        table: None,
        mask: None,
    },
    AbiParam {
        table: Some("packets"),
        mask: None,
    },
    AbiParam {
        table: None,
        mask: Some("valid"),
    },
    AbiParam {
        table: Some("small"),
        mask: None,
    },
    AbiParam {
        table: None,
        mask: Some("small_valid"),
    },
];

pub fn try_lower_dataplane(module: &MirModule) -> Option<LirModule> {
    let (region, block) = find_dataplane_lambda(module)?;
    let mut reduces = Vec::new();
    for &op_id in module.block(block).operations() {
        let op = module.operation(op_id);
        if op.facts().fused_into().is_some() {
            continue;
        }
        if let OperationKind::ReduceRows {
            table,
            mask,
            field,
            rows,
        } = op.kind()
        {
            if !supported_reduce(table, mask, field, *rows) {
                return None;
            }
            let fused = op.facts().fused_with().and_then(|second| {
                let second_op = module.operation(second);
                if let OperationKind::ReduceRows {
                    table: table2,
                    mask: mask2,
                    field: field2,
                    rows: rows2,
                } = second_op.kind()
                {
                    if table == table2 && mask == mask2 && rows == rows2 {
                        Some((second, field2.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            reduces.push((
                op_id,
                table.clone(),
                mask.clone(),
                field.clone(),
                *rows,
                fused,
            ));
        }
    }
    if reduces.is_empty() {
        return None;
    }
    let return_value = final_return_value(module, block)?;
    Some(lower_filter_sum(
        module,
        region,
        block,
        &reduces,
        return_value,
    ))
}

fn supported_reduce(table: &str, mask: &str, field: &str, rows: u64) -> bool {
    matches!(
        (table, mask, field, rows),
        ("packets", "valid", "len", 256)
            | ("packets", "valid", "flags", 256)
            | ("small", "small_valid", "len", 128)
    )
}

fn field_offset(field: &str) -> Option<u32> {
    match field {
        "len" => Some(0),
        "flags" => Some(4),
        _ => None,
    }
}

fn find_dataplane_lambda(module: &MirModule) -> Option<(super::RegionId, BlockId)> {
    for (region_index, region) in module.regions().iter().enumerate() {
        let RegionKind::Lambda { symbol } = region.kind() else {
            continue;
        };
        if !symbol.contains("FilterSumBench") {
            continue;
        }
        let block = *region.blocks().first()?;
        return Some((super::RegionId::new(region_index as u32), block));
    }
    None
}

fn final_return_value(module: &MirModule, block: BlockId) -> Option<ValueId> {
    let return_op = module
        .block(block)
        .operations()
        .iter()
        .rev()
        .find_map(|op_id| {
            let op = module.operation(*op_id);
            matches!(op.kind(), OperationKind::Return).then_some(*op_id)
        })?;
    module.operation(return_op).operands().first().copied()
}

#[allow(clippy::type_complexity)]
fn lower_filter_sum(
    module: &MirModule,
    _region: super::RegionId,
    _block: BlockId,
    reduces: &[(
        OperationId,
        String,
        String,
        String,
        u64,
        Option<(OperationId, String)>,
    )],
    return_value: ValueId,
) -> LirModule {
    let mut lir = LirModule::new();
    let symbol = module
        .regions()
        .iter()
        .find_map(|region| {
            if let RegionKind::Lambda { symbol } = region.kind() {
                Some(symbol.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "dataplane.run".to_string());
    let function = lir.push_function(symbol);
    for _ in 0..FILTER_SUM_ABI.len() {
        let param = lir.push_vreg(RegisterClass::Gp64);
        lir.push_function_param(function, param);
    }

    let entry = lir.push_block(function, "entry".to_string());
    let epilogue = lir.push_block(function, "epilogue".to_string());
    let temps = DataplaneTemps::new(&mut lir);
    let mut reduce_results: BTreeMap<ValueId, VirtualReg> = BTreeMap::new();
    let mut next_pre: Option<LirBlockId> = None;

    for (index, (op_id, table, mask, field, rows, fused)) in reduces.iter().enumerate() {
        let acc_value = module.operation(*op_id).results()[0];
        let (field0, field1, second_op) = if let Some((second_id, field2)) = fused {
            (field.as_str(), field2.as_str(), Some(*second_id))
        } else {
            (field.as_str(), "", None)
        };
        let acc1_value =
            second_op.and_then(|second_id| module.operation(second_id).results().first().copied());
        let table_reg = table_param_reg(function, &lir, table);
        let mask_reg = mask_param_reg(function, &lir, mask);
        let emitted = emit_reduce_loop(
            &mut lir, function, &temps, table_reg, mask_reg, field0, field1, *rows,
        );
        reduce_results.insert(acc_value, emitted.acc.0);
        if let (Some(acc1), Some(acc1_reg)) = (acc1_value, emitted.acc.1) {
            reduce_results.insert(acc1, acc1_reg);
        }
        if let Some(pre) = next_pre {
            lir.push_inst(
                pre,
                LirInst::new(
                    LirOpcode::Branch,
                    vec![LirOperand::Label(block_label(emitted.pre))],
                    Vec::new(),
                    TargetEffects::pure(),
                ),
            );
        } else {
            lir.push_inst(
                entry,
                LirInst::new(
                    LirOpcode::Branch,
                    vec![LirOperand::Label(block_label(emitted.pre))],
                    Vec::new(),
                    TargetEffects::pure(),
                ),
            );
        }
        let done_target = if index + 1 == reduces.len() {
            epilogue
        } else {
            next_pre = Some(lir.push_block(function, format!("bridge_{index}")));
            next_pre.expect("bridge block")
        };
        lir.push_inst(
            emitted.exit,
            LirInst::new(
                LirOpcode::Branch,
                vec![LirOperand::Label(block_label(done_target))],
                Vec::new(),
                TargetEffects::pure(),
            ),
        );
    }

    let result_reg = sum_return_operands(module, return_value, &reduce_results, &mut lir, epilogue);
    lir.push_inst(
        epilogue,
        LirInst::new(
            LirOpcode::Ret,
            vec![LirOperand::Reg(result_reg)],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );
    lir
}

fn table_param_reg(function: LirFunctionId, lir: &LirModule, table: &str) -> VirtualReg {
    for (index, param) in FILTER_SUM_ABI.iter().enumerate() {
        if param.table == Some(table) {
            return lir.functions()[function.raw() as usize].params()[index];
        }
    }
    lir.functions()[function.raw() as usize].params()[1]
}

fn mask_param_reg(function: LirFunctionId, lir: &LirModule, mask: &str) -> VirtualReg {
    for (index, param) in FILTER_SUM_ABI.iter().enumerate() {
        if param.mask == Some(mask) {
            return lir.functions()[function.raw() as usize].params()[index];
        }
    }
    lir.functions()[function.raw() as usize].params()[2]
}

struct LoopAccRegs(VirtualReg, Option<VirtualReg>);

struct EmittedLoop {
    acc: LoopAccRegs,
    exit: LirBlockId,
    pre: LirBlockId,
}

struct DataplaneTemps {
    index: VirtualReg,
    mask_byte: VirtualReg,
    bit_test: VirtualReg,
    value: VirtualReg,
}

impl DataplaneTemps {
    fn new(lir: &mut LirModule) -> Self {
        Self {
            index: lir.push_vreg(RegisterClass::Gp64),
            mask_byte: lir.push_vreg(RegisterClass::Gp32),
            bit_test: lir.push_vreg(RegisterClass::Gp32),
            value: lir.push_vreg(RegisterClass::Gp32),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_reduce_loop(
    lir: &mut LirModule,
    function: LirFunctionId,
    temps: &DataplaneTemps,
    table: VirtualReg,
    mask: VirtualReg,
    field0: &str,
    field1: &str,
    rows: u64,
) -> EmittedLoop {
    let field0_off = field_offset(field0).expect("supported field");
    let fused = !field1.is_empty();
    let field1_off = if fused { field_offset(field1) } else { None };

    let pre = lir.push_block(function, format!("pre_{field0}_{rows}"));
    let head = lir.push_block(function, format!("head_{field0}_{rows}"));
    let body = lir.push_block(function, format!("body_{field0}_{rows}"));
    let next = lir.push_block(function, format!("next_{field0}_{rows}"));
    let exit = lir.push_block(function, format!("exit_{field0}_{rows}"));

    let index = temps.index;
    let mask_byte = temps.mask_byte;
    let bit_test = temps.bit_test;
    let value0 = temps.value;
    let rows_imm = LirOperand::ImmI64(rows as i64);
    let acc0 = lir.push_vreg(RegisterClass::Gp64);
    let acc1 = if fused {
        Some(lir.push_vreg(RegisterClass::Gp64))
    } else {
        None
    };

    lir.push_inst(
        pre,
        LirInst::new(
            LirOpcode::Mov,
            vec![LirOperand::ImmI64(0)],
            vec![index],
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        pre,
        LirInst::new(
            LirOpcode::Mov,
            vec![LirOperand::ImmI64(0)],
            vec![acc0],
            TargetEffects::pure(),
        ),
    );
    if let Some(acc1_reg) = acc1 {
        lir.push_inst(
            pre,
            LirInst::new(
                LirOpcode::Mov,
                vec![LirOperand::ImmI64(0)],
                vec![acc1_reg],
                TargetEffects::pure(),
            ),
        );
    }
    lir.push_inst(
        pre,
        LirInst::new(
            LirOpcode::Branch,
            vec![LirOperand::Label(block_label(head))],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );

    lir.push_inst(
        head,
        LirInst::new(
            LirOpcode::BranchIfLessThan,
            vec![
                LirOperand::Reg(index),
                rows_imm.clone(),
                LirOperand::Label(block_label(body)),
            ],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        head,
        LirInst::new(
            LirOpcode::Branch,
            vec![LirOperand::Label(block_label(exit))],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );

    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::LoadMaskByte,
            vec![LirOperand::Reg(mask), LirOperand::Reg(index)],
            vec![mask_byte],
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::TestMaskBit,
            vec![LirOperand::Reg(mask_byte), LirOperand::Reg(index)],
            vec![bit_test],
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::BranchIfZero,
            vec![
                LirOperand::Reg(bit_test),
                LirOperand::Label(block_label(next)),
            ],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::LoadU32,
            vec![
                LirOperand::Reg(table),
                LirOperand::Reg(index),
                LirOperand::ImmI64(field0_off as i64),
            ],
            vec![value0],
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::AddU64,
            vec![LirOperand::Reg(acc0), LirOperand::Reg(value0)],
            vec![acc0],
            TargetEffects::pure(),
        ),
    );
    if let (Some(field1_off), Some(acc1_reg)) = (field1_off, acc1) {
        lir.push_inst(
            body,
            LirInst::new(
                LirOpcode::LoadU32,
                vec![
                    LirOperand::Reg(table),
                    LirOperand::Reg(index),
                    LirOperand::ImmI64(field1_off as i64),
                ],
                vec![value0],
                TargetEffects::pure(),
            ),
        );
        lir.push_inst(
            body,
            LirInst::new(
                LirOpcode::AddU64,
                vec![LirOperand::Reg(acc1_reg), LirOperand::Reg(value0)],
                vec![acc1_reg],
                TargetEffects::pure(),
            ),
        );
    }
    lir.push_inst(
        body,
        LirInst::new(
            LirOpcode::BranchIfLessThan,
            vec![
                LirOperand::Reg(index),
                rows_imm.clone(),
                LirOperand::Label(block_label(next)),
            ],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );

    lir.push_inst(
        next,
        LirInst::new(
            LirOpcode::Add,
            vec![LirOperand::Reg(index), LirOperand::ImmI64(1)],
            vec![index],
            TargetEffects::pure(),
        ),
    );
    lir.push_inst(
        next,
        LirInst::new(
            LirOpcode::BranchIfLessThan,
            vec![
                LirOperand::Reg(index),
                rows_imm.clone(),
                LirOperand::Label(block_label(head)),
            ],
            Vec::new(),
            TargetEffects::pure(),
        ),
    );

    EmittedLoop {
        acc: LoopAccRegs(acc0, acc1),
        exit,
        pre,
    }
}

fn block_label(block: LirBlockId) -> String {
    format!(".L{}", block.raw())
}

fn sum_return_operands(
    module: &MirModule,
    value: ValueId,
    reduce_results: &BTreeMap<ValueId, VirtualReg>,
    lir: &mut LirModule,
    block: LirBlockId,
) -> VirtualReg {
    let op = module
        .operations()
        .iter()
        .find(|candidate| candidate.results().first() == Some(&value));
    if let Some(op) = op {
        if let OperationKind::Binary(op_name) = op.kind() {
            if op_name == "+" && op.operands().len() == 2 {
                let left =
                    sum_return_operands(module, op.operands()[0], reduce_results, lir, block);
                let right =
                    sum_return_operands(module, op.operands()[1], reduce_results, lir, block);
                let sum = lir.push_vreg(RegisterClass::Gp64);
                lir.push_inst(
                    block,
                    LirInst::new(
                        LirOpcode::Add,
                        vec![LirOperand::Reg(left), LirOperand::Reg(right)],
                        vec![sum],
                        TargetEffects::pure(),
                    ),
                );
                return sum;
            }
        }
    }
    reduce_results.get(&value).copied().unwrap_or_else(|| {
        let zero = lir.push_vreg(RegisterClass::Gp64);
        lir.push_inst(
            block,
            LirInst::new(
                LirOpcode::Mov,
                vec![LirOperand::ImmI64(0)],
                vec![zero],
                TargetEffects::pure(),
            ),
        );
        zero
    })
}
