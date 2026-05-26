use std::fmt::Write;

use super::effect::render_effect_set;
use super::type_text::{render_mir_type, render_quoted_if_needed};
use super::{MirModule, OperationKind, RegionKind};

pub fn render_module(module: &MirModule) -> String {
    let mut out = String::new();
    out.push_str("wmir v1\n");
    for (index, value) in module.values().iter().enumerate() {
        let _ = writeln!(out, "value v{index} type={}", render_mir_type(value.ty()));
    }
    for (index, place) in module.places().iter().enumerate() {
        let _ = writeln!(
            out,
            "place p{index} name={} type={}",
            render_quoted_if_needed(place.name()),
            render_mir_type(place.ty())
        );
    }
    for (index, region) in module.regions().iter().enumerate() {
        let kind = match region.kind() {
            RegionKind::Lambda { symbol } => format!("lambda {symbol}"),
            RegionKind::Omega { symbol } => format!("omega {symbol}"),
            RegionKind::Gamma { label } => format!("gamma {label}"),
            RegionKind::Theta(kind) => format!("theta {}", theta_kind_text(kind)),
            RegionKind::Delta { label } => format!("delta {label}"),
        };
        let _ = writeln!(out, "region r{index} {kind}");
        for block in region.blocks() {
            let block_data = module.block(*block);
            let _ = writeln!(out, "  block b{} {}", block.raw(), block_data.name());
            for arg in block_data.arguments() {
                let ty = module.value(*arg).ty();
                let _ = writeln!(out, "    arg v{} type={}", arg.raw(), render_mir_type(ty));
            }
            for op in block_data.operations() {
                let op_data = module.operation(*op);
                let operands = op_data
                    .operands()
                    .iter()
                    .map(|value| format!("v{}", value.raw()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let results = op_data
                    .results()
                    .iter()
                    .map(|value| format!("v{}", value.raw()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let payload = operation_payload(op_data.kind());
                let effects = render_effect_set(op_data.effects());
                if results.is_empty() {
                    let _ = writeln!(
                        out,
                        "    op o{} {payload} {operands} effects={effects}",
                        op.raw()
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "    op o{} {payload} {operands} effects={effects} -> {results}",
                        op.raw()
                    );
                }
                render_operation_facts(&mut out, op_data.facts());
            }
        }
    }
    out
}

fn render_operation_facts(out: &mut String, facts: &super::OperationFacts) {
    if !facts.ownership().is_empty() {
        let modes = facts
            .ownership()
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(out, "      fact ownership={modes}");
    }
    for capability in facts.capabilities() {
        let path = capability.path().join(".");
        let _ = writeln!(
            out,
            "      fact capability={}:{}",
            render_quoted_if_needed(capability.class_name()),
            render_quoted_if_needed(&path)
        );
    }
    if let Some(trap) = facts.trap() {
        let _ = writeln!(
            out,
            "      fact trap={}",
            render_quoted_if_needed(trap.reason())
        );
    }
    if facts.has_state_edge() {
        let _ = writeln!(out, "      fact state_edge");
    }
    if facts.has_capacity_token() {
        let _ = writeln!(out, "      fact capacity_token");
    }
    if let Some(target) = facts.fused_with() {
        let _ = writeln!(out, "      fact fused_with=o{}", target.raw());
    }
    if let Some(target) = facts.fused_into() {
        let _ = writeln!(out, "      fact fused_into=o{}", target.raw());
    }
}

fn operation_payload(kind: &OperationKind) -> String {
    match kind {
        OperationKind::Literal(text) => format!("literal {}", render_quoted_if_needed(text)),
        OperationKind::ReadValue(name) => format!("read_value {}", render_quoted_if_needed(name)),
        OperationKind::Binary(op) => format!("binary {}", render_quoted_if_needed(op)),
        OperationKind::Let(name) => format!("let {}", render_quoted_if_needed(name)),
        OperationKind::Return => "return".to_string(),
        OperationKind::TableRows { table, rows } => format!("table_rows {table} rows={rows}"),
        OperationKind::MaskRead { name, table, rows } => {
            format!("mask_read {name} table={table} rows={rows}")
        }
        OperationKind::MaskAllTrue { table, rows } => {
            format!("mask_all_true table={table} rows={rows}")
        }
        OperationKind::MaskAllFalse { table, rows } => {
            format!("mask_all_false table={table} rows={rows}")
        }
        OperationKind::MaskNot => "mask_not".to_string(),
        OperationKind::MaskAnd => "mask_and".to_string(),
        OperationKind::MaskOr => "mask_or".to_string(),
        OperationKind::RowToken { table, rows } => format!("row_token table={table} rows={rows}"),
        OperationKind::ReduceRows {
            table,
            mask,
            field,
            rows,
        } => format!("reduce_rows table={table} mask={mask} field={field} rows={rows}"),
    }
}

fn theta_kind_text(kind: &super::ThetaKind) -> &'static str {
    match kind {
        super::ThetaKind::Repeat => "repeat",
        super::ThetaKind::For => "for",
        super::ThetaKind::Drain => "drain",
        super::ThetaKind::Reduce => "reduce",
        super::ThetaKind::Scan => "scan",
        super::ThetaKind::Loop => "loop",
    }
}

#[cfg(test)]
mod tests {
    use crate::mir::{
        BlockData, Effect, EffectSet, MirModule, MirType, OperationData, OperationFacts,
        OperationKind, OwnershipMode, RegionData, ScalarType, TrapProvenance, ValueData,
    };

    #[test]
    fn text_dump_includes_effects_and_facts() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let value = module.push_value(ValueData::new(MirType::Scalar(ScalarType::U64)));
        let mut facts = OperationFacts::empty();
        facts.set_ownership(vec![OwnershipMode::Read]);
        facts.set_trap(Some(TrapProvenance::new("overflow".to_string())));
        facts.set_state_edge(true);
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![value],
                Vec::new(),
                EffectSet::single(Effect::Trap),
            )
            .with_facts(facts),
        );

        let text = super::render_module(&module);
        assert!(text.contains("value v0 type=scalar:U64"));
        assert!(text.contains("effects=Trap"));
        assert!(text.contains("fact ownership=read"));
        assert!(text.contains("fact trap=overflow"));
        assert!(text.contains("fact state_edge"));
    }
}
