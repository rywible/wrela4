use std::collections::{BTreeMap, BTreeSet};

use super::{BlockId, MirModule, OperationId, OperationKind, RegionKind, ValueId};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifyResult {
    messages: Vec<String>,
}

impl VerifyResult {
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

pub fn verify_module(module: &MirModule) -> VerifyResult {
    let mut result = VerifyResult::default();
    let block_count = module.blocks().len() as u32;
    let operation_count = module.operations().len() as u32;
    let value_count = module.values().len() as u32;
    let mut owned_blocks = BTreeSet::new();
    let mut value_producers = BTreeMap::<ValueId, OperationId>::new();

    for (region_index, region) in module.regions().iter().enumerate() {
        if region.blocks().is_empty() {
            result.push(format!("region r{region_index} has no blocks"));
        }
        for block in region.blocks() {
            if block.raw() >= block_count {
                result.push(format!(
                    "region r{region_index} owns unknown block b{}",
                    block.raw()
                ));
                continue;
            }
            if !owned_blocks.insert(*block) {
                result.push(format!(
                    "block b{} is owned by multiple regions",
                    block.raw()
                ));
            }
        }
    }

    for (block_index, block) in module.blocks().iter().enumerate() {
        let block_id = BlockId::new(block_index as u32);
        if !owned_blocks.contains(&block_id) {
            result.push(format!("block b{block_index} is not owned by any region"));
        }

        let mut seen_return = false;
        let mut local_defs = block.arguments().iter().copied().collect::<BTreeSet<_>>();

        for op_id in block.operations() {
            if op_id.raw() >= operation_count {
                result.push(format!(
                    "block b{block_index} contains unknown operation o{}",
                    op_id.raw()
                ));
                continue;
            }
            let op = module.operation(*op_id);
            if seen_return {
                result.push(format!("operation after return in block b{block_index}"));
            }
            for operand in op.operands() {
                verify_value(*operand, value_count, &mut result);
                if operand.raw() < value_count && !local_defs.contains(operand) {
                    result.push(format!(
                        "operand v{} is used before definition in block b{block_index}",
                        operand.raw()
                    ));
                }
            }
            for output in op.results() {
                verify_value(*output, value_count, &mut result);
                if value_producers.insert(*output, *op_id).is_some() {
                    result.push(format!("value v{} has multiple producers", output.raw()));
                }
                local_defs.insert(*output);
            }
            if op.facts().has_state_edge() && op.effects().bits() == 0 {
                result.push(format!(
                    "operation o{} has state-edge facts but empty effect set",
                    op_id.raw()
                ));
            }
            if matches!(op.kind(), OperationKind::Return) {
                seen_return = true;
            }
        }
    }

    for (region_index, region) in module.regions().iter().enumerate() {
        if matches!(region.kind(), RegionKind::Lambda { .. }) {
            for block in region.blocks() {
                if block.raw() < block_count {
                    let data = module.block(*block);
                    let ends_in_return = data
                        .operations()
                        .last()
                        .map(|op_id| {
                            op_id.raw() < operation_count
                                && matches!(module.operation(*op_id).kind(), OperationKind::Return)
                        })
                        .unwrap_or(false);
                    if !ends_in_return {
                        result.push(format!(
                            "lambda region r{region_index} block b{} does not end in return",
                            block.raw()
                        ));
                    }
                }
            }
        }
    }

    verify_dataplane(module, &mut result);

    result
}

fn verify_dataplane(module: &MirModule, result: &mut VerifyResult) {
    let provenance = mask_provenance_by_value(module);
    let mut row_tokens = BTreeMap::<ValueId, (String, u64)>::new();

    for (op_index, op) in module.operations().iter().enumerate() {
        match op.kind() {
            OperationKind::MaskAnd | OperationKind::MaskOr if op.operands().len() == 2 => {
                let left = provenance.get(&op.operands()[0]);
                let right = provenance.get(&op.operands()[1]);
                match (left, right) {
                    (Some(left), Some(right))
                        if left.table() != right.table() || left.rows() != right.rows() =>
                    {
                        result.push(format!(
                            "operation o{op_index} combines masks from different domains"
                        ));
                    }
                    _ => {}
                }
            }
            OperationKind::ReduceRows {
                table, mask, rows, ..
            } => {
                if let Some(mask_prov) = mask_domain_for_name(&provenance, mask) {
                    if mask_prov.table() != table || mask_prov.rows() != *rows {
                        result.push(format!(
                            "operation o{op_index} reduce_rows table/mask rows mismatch"
                        ));
                    }
                }
            }
            OperationKind::RowToken { table, rows, .. } => {
                if let Some(result_value) = op.results().first() {
                    row_tokens.insert(*result_value, (table.clone(), *rows));
                }
            }
            _ => {}
        }
    }

    let used_values = module
        .operations()
        .iter()
        .flat_map(|op| op.operands().iter().copied())
        .collect::<BTreeSet<_>>();

    for (value, (table, rows)) in row_tokens {
        if !used_values.contains(&value) {
            continue;
        }
        let mut allowed = false;
        for op in module.operations() {
            if matches!(op.kind(), OperationKind::ReduceRows { .. })
                && op.operands().contains(&value)
            {
                allowed = true;
                break;
            }
        }
        if !allowed {
            result.push(format!(
                "row token v{} for table `{table}` rows={rows} escapes its reduce loop",
                value.raw()
            ));
        }
        for op in module.operations() {
            if matches!(op.kind(), OperationKind::Return) && op.operands().contains(&value) {
                result.push(format!(
                    "row token v{} for table `{table}` cannot be returned",
                    value.raw()
                ));
            }
        }
    }
}

fn mask_provenance_by_value(
    module: &MirModule,
) -> BTreeMap<ValueId, super::dataplane::MaskProvenance> {
    use super::dataplane::{MaskProvenance, MaskValueKind};
    let mut map = BTreeMap::new();
    for op in module.operations() {
        match op.kind() {
            OperationKind::MaskRead { name, table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(
                        *result,
                        MaskProvenance::new(name.clone(), table.clone(), *rows),
                    );
                }
            }
            OperationKind::MaskAllTrue { table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(
                        *result,
                        MaskProvenance::constant(table.clone(), *rows, MaskValueKind::AllTrue),
                    );
                }
            }
            OperationKind::MaskAllFalse { table, rows } => {
                if let Some(result) = op.results().first() {
                    map.insert(
                        *result,
                        MaskProvenance::constant(table.clone(), *rows, MaskValueKind::AllFalse),
                    );
                }
            }
            _ => {}
        }
    }
    map
}

fn mask_domain_for_name<'a>(
    provenance: &'a BTreeMap<ValueId, super::dataplane::MaskProvenance>,
    name: &str,
) -> Option<&'a super::dataplane::MaskProvenance> {
    provenance.values().find(|prov| prov.name() == name)
}

fn verify_value(value: ValueId, value_count: u32, result: &mut VerifyResult) {
    if value.raw() >= value_count {
        result.push(format!("unknown value v{}", value.raw()));
    }
}

#[cfg(test)]
mod tests {
    use super::verify_module;
    use crate::mir::{
        BlockData, EffectSet, MirModule, MirType, OperationData, OperationKind, RegionData,
        ScalarType, ValueData, ValueId,
    };

    #[test]
    fn verifier_accepts_minimal_returning_lambda() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let none = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Literal("None".to_string()),
                Vec::new(),
                vec![none],
                EffectSet::empty(),
            ),
        );
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![none],
                Vec::new(),
                EffectSet::empty(),
            ),
        );

        assert!(verify_module(&module).ok());
    }

    #[test]
    fn verifier_rejects_missing_operand_definition() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![ValueId::new(99)],
                Vec::new(),
                EffectSet::empty(),
            ),
        );

        let result = verify_module(&module);
        assert!(!result.ok());
        assert!(result.messages()[0].contains("unknown value v99"));
    }
}
