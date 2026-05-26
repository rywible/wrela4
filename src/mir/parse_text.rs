use std::collections::BTreeMap;

use super::effect::parse_effect_set;
use super::type_text::{parse_mir_type, parse_quoted_or_token};
use super::{
    BlockData, CapabilityPath, EffectSet, MirModule, MirType, OperationData, OperationId,
    OperationKind, OwnershipMode, PlaceData, RegionData, ThetaKind, TrapProvenance, ValueData,
    ValueId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextParseError {
    message: String,
}

impl TextParseError {
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn parse_module(text: &str) -> Result<MirModule, TextParseError> {
    let mut lines = text.lines();
    match lines.next() {
        Some("wmir v1") => {}
        _ => return Err(TextParseError::new("missing wmir v1 header")),
    }

    let mut module = MirModule::new();
    let mut value_types = BTreeMap::new();
    let mut place_types = BTreeMap::new();
    let mut place_names = BTreeMap::new();
    let mut current_region = None;
    let mut current_block = None;
    let mut max_value_id = None::<u32>;

    for line in lines {
        if let Some(rest) = line.strip_prefix("value ") {
            let (value_id, ty) = parse_value_type_line(rest, line)?;
            value_types.insert(value_id.raw(), ty);
            max_value_id = Some(max_value_id.map_or(value_id.raw(), |max| max.max(value_id.raw())));
        } else if let Some(rest) = line.strip_prefix("place ") {
            let (place_id, name, ty) = parse_place_line(rest, line)?;
            place_names.insert(place_id, name);
            place_types.insert(place_id, ty);
        } else if let Some(rest) = line.strip_prefix("region ") {
            let region = parse_region_line(rest, line)?;
            current_region = Some(module.push_region(region));
            current_block = None;
        } else if let Some(rest) = line.strip_prefix("  block ") {
            let region = current_region
                .ok_or_else(|| TextParseError::new("block line appears before any region line"))?;
            let name = rest
                .split_once(' ')
                .map(|(_, name)| name.to_string())
                .ok_or_else(|| TextParseError::new(format!("bad block line: {line}")))?;
            current_block = Some(module.push_block(region, BlockData::new(name)));
        } else if let Some(rest) = line.strip_prefix("    arg ") {
            let block = current_block.ok_or_else(|| TextParseError::new("arg before block"))?;
            let (value_id, ty) = parse_value_type_line(rest, line)?;
            value_types.insert(value_id.raw(), ty);
            max_value_id = Some(max_value_id.map_or(value_id.raw(), |max| max.max(value_id.raw())));
            module.push_block_argument(block, value_id);
        } else if let Some(rest) = line.strip_prefix("    op ") {
            let block = current_block.ok_or_else(|| TextParseError::new("op before block"))?;
            let (effects, op_rest, results_text) = split_effects_and_results(rest)?;
            let kind = parse_operation_kind(op_rest)?;
            let (operands_text, _) = split_operands_and_results(op_rest);
            let operands = parse_value_ids(operands_text)?;
            let results = parse_values(results_text)?;
            for value in operands.iter().chain(results.iter()) {
                max_value_id = Some(max_value_id.map_or(value.raw(), |max| max.max(value.raw())));
            }
            module.push_operation(block, OperationData::new(kind, operands, results, effects));
        } else if let Some(rest) = line.strip_prefix("      fact ") {
            let block = current_block.ok_or_else(|| TextParseError::new("fact before block"))?;
            apply_fact_line(&mut module, block, rest)?;
        } else if !line.trim().is_empty() {
            return Err(TextParseError::new(format!("unexpected line: {line}")));
        }
    }

    let max_id = [max_value_id, value_types.keys().max().copied()]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);

    for id in 0..=max_id {
        let ty = value_types.get(&id).cloned().unwrap_or(MirType::Unknown);
        module.push_value(ValueData::new(ty));
    }

    if let Some(max_place) = place_types.keys().max().copied() {
        for id in 0..=max_place {
            let name = place_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("p{id}"));
            let ty = place_types.get(&id).cloned().unwrap_or(MirType::Unknown);
            module.push_place(PlaceData::new(name, ty));
        }
    }

    Ok(module)
}

fn parse_place_line(rest: &str, line: &str) -> Result<(u32, String, MirType), TextParseError> {
    let Some((id_text, tail)) = rest.split_once(' ') else {
        return Err(TextParseError::new(format!("bad place line: {line}")));
    };
    let raw = id_text
        .strip_prefix('p')
        .ok_or_else(|| TextParseError::new(format!("bad place id in `{line}`")))?
        .parse::<u32>()
        .map_err(|_| TextParseError::new(format!("bad place id in `{line}`")))?;
    let name = parse_named_attr(tail, "name=")?;
    let ty = parse_mir_type(&parse_named_attr(tail, "type=")?).map_err(TextParseError::new)?;
    Ok((raw, name, ty))
}

fn parse_value_type_line(rest: &str, line: &str) -> Result<(ValueId, MirType), TextParseError> {
    let Some((id_text, ty_text)) = rest.split_once(" type=") else {
        return Err(TextParseError::new(format!("bad value line: {line}")));
    };
    let raw = id_text
        .strip_prefix('v')
        .ok_or_else(|| TextParseError::new(format!("bad value id in `{line}`")))?
        .parse::<u32>()
        .map_err(|_| TextParseError::new(format!("bad value id in `{line}`")))?;
    let ty = parse_mir_type(ty_text).map_err(TextParseError::new)?;
    Ok((ValueId::new(raw), ty))
}

fn split_effects_and_results(rest: &str) -> Result<(EffectSet, &str, &str), TextParseError> {
    let (left, results_text) = if let Some(index) = rest.find(" -> ") {
        (&rest[..index], &rest[index + 4..])
    } else {
        (rest, "")
    };
    let (op_rest, effects) = if let Some(index) = left.rfind(" effects=") {
        let effects_text = &left[index + " effects=".len()..];
        (
            &left[..index],
            parse_effect_set(effects_text).map_err(TextParseError::new)?,
        )
    } else {
        (left, EffectSet::empty())
    };
    Ok((effects, op_rest, results_text))
}

fn parse_region_line(rest: &str, line: &str) -> Result<RegionData, TextParseError> {
    let kind_rest = rest
        .split_once(' ')
        .map(|(_, tail)| tail)
        .ok_or_else(|| TextParseError::new(format!("bad region line: {line}")))?;
    if let Some(symbol) = kind_rest.strip_prefix("lambda ") {
        return Ok(RegionData::lambda(symbol.to_string()));
    }
    if let Some(symbol) = kind_rest.strip_prefix("omega ") {
        return Ok(RegionData::omega(symbol.to_string()));
    }
    if let Some(label) = kind_rest.strip_prefix("gamma ") {
        return Ok(RegionData::gamma(label.to_string()));
    }
    if let Some(label) = kind_rest.strip_prefix("delta ") {
        return Ok(RegionData::delta(label.to_string()));
    }
    if let Some(kind) = kind_rest.strip_prefix("theta ") {
        let theta = parse_theta_kind(kind.trim())?;
        return Ok(RegionData::theta(theta));
    }
    Err(TextParseError::new(format!(
        "unsupported region line: {line}"
    )))
}

fn parse_theta_kind(text: &str) -> Result<ThetaKind, TextParseError> {
    match text {
        "repeat" => Ok(ThetaKind::Repeat),
        "for" => Ok(ThetaKind::For),
        "drain" => Ok(ThetaKind::Drain),
        "reduce" => Ok(ThetaKind::Reduce),
        "scan" => Ok(ThetaKind::Scan),
        "loop" => Ok(ThetaKind::Loop),
        _ => Err(TextParseError::new(format!(
            "unsupported theta kind `{text}`"
        ))),
    }
}

fn apply_fact_line(
    module: &mut MirModule,
    block: super::BlockId,
    rest: &str,
) -> Result<(), TextParseError> {
    let op_id = *module
        .block(block)
        .operations()
        .last()
        .ok_or_else(|| TextParseError::new("fact without preceding op"))?;
    let facts = module.operation_mut(op_id).facts_mut();
    if let Some(target) = rest.strip_prefix("fused_with=") {
        return apply_fused_with(module, block, parse_operation_ref(target)?);
    }
    if let Some(target) = rest.strip_prefix("fused_into=") {
        return apply_fused_into(module, block, parse_operation_ref(target)?);
    }
    if let Some(modes) = rest.strip_prefix("ownership=") {
        let parsed = modes
            .split(',')
            .map(OwnershipMode::parse_name)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| TextParseError::new(format!("bad ownership fact `{rest}`")))?;
        facts.set_ownership(parsed);
        return Ok(());
    }
    if let Some(payload) = rest.strip_prefix("capability=") {
        let capability = parse_capability_fact(payload)?;
        facts.push_capability(capability);
        return Ok(());
    }
    if let Some(reason) = rest.strip_prefix("trap=") {
        facts.set_trap(Some(TrapProvenance::new(parse_quoted_or_token(reason))));
        return Ok(());
    }
    if rest == "state_edge" {
        facts.set_state_edge(true);
        return Ok(());
    }
    if rest == "capacity_token" {
        facts.set_capacity_token(true);
        return Ok(());
    }
    Err(TextParseError::new(format!("unsupported fact `{rest}`")))
}

fn parse_capability_fact(payload: &str) -> Result<CapabilityPath, TextParseError> {
    let Some((class, path)) = payload.split_once(':') else {
        return Err(TextParseError::new(format!(
            "bad capability fact `{payload}`"
        )));
    };
    let segments = path
        .split('.')
        .map(parse_quoted_or_token)
        .collect::<Vec<_>>();
    Ok(CapabilityPath::new(parse_quoted_or_token(class), segments))
}

fn parse_operation_ref(text: &str) -> Result<OperationId, TextParseError> {
    let trimmed = text.trim();
    let Some(raw) = trimmed.strip_prefix('o') else {
        return Err(TextParseError::new(format!(
            "expected operation reference like o0, got `{trimmed}`"
        )));
    };
    let index = raw
        .parse::<u32>()
        .map_err(|_| TextParseError::new(format!("invalid operation id `{trimmed}`")))?;
    Ok(OperationId::new(index))
}

fn apply_fused_with(
    module: &mut MirModule,
    block: super::BlockId,
    target: OperationId,
) -> Result<(), TextParseError> {
    let last = *module
        .block(block)
        .operations()
        .last()
        .ok_or_else(|| TextParseError::new("fused_with fact without preceding op"))?;
    module
        .operation_mut(last)
        .facts_mut()
        .mark_fused_with(target);
    Ok(())
}

fn apply_fused_into(
    module: &mut MirModule,
    block: super::BlockId,
    target: OperationId,
) -> Result<(), TextParseError> {
    let last = *module
        .block(block)
        .operations()
        .last()
        .ok_or_else(|| TextParseError::new("fused_into fact without preceding op"))?;
    module
        .operation_mut(last)
        .facts_mut()
        .mark_fused_into(target);
    Ok(())
}

fn parse_operation_kind(rest: &str) -> Result<OperationKind, TextParseError> {
    let mut words = rest.split_whitespace();
    let _op_tag = words
        .next()
        .ok_or_else(|| TextParseError::new(format!("missing operation tag in `{rest}`")))?;
    let tag = words
        .next()
        .ok_or_else(|| TextParseError::new(format!("missing operation kind in `{rest}`")))?;
    let tail = words.collect::<Vec<_>>().join(" ");
    match tag {
        "return" => Ok(OperationKind::Return),
        "literal" => Ok(OperationKind::Literal(parse_quoted_or_token(first_word(
            &tail,
        )))),
        "read_value" => Ok(OperationKind::ReadValue(parse_quoted_or_token(first_word(
            &tail,
        )))),
        "binary" => Ok(OperationKind::Binary(parse_quoted_or_token(first_word(
            &tail,
        )))),
        "let" => Ok(OperationKind::Let(parse_quoted_or_token(first_word(&tail)))),
        "table_rows" => {
            let table = first_word(&tail);
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::TableRows {
                table: table.to_string(),
                rows,
            })
        }
        "mask_read" => {
            let name = parse_quoted_or_token(first_word(&tail));
            let table = parse_named_attr(&tail, "table=")?;
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::MaskRead { name, table, rows })
        }
        "mask_all_true" => {
            let table = parse_named_attr(&tail, "table=")?;
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::MaskAllTrue { table, rows })
        }
        "mask_all_false" => {
            let table = parse_named_attr(&tail, "table=")?;
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::MaskAllFalse { table, rows })
        }
        "mask_not" => Ok(OperationKind::MaskNot),
        "mask_and" => Ok(OperationKind::MaskAnd),
        "mask_or" => Ok(OperationKind::MaskOr),
        "row_token" => {
            let table = parse_named_attr(&tail, "table=")?;
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::RowToken { table, rows })
        }
        "reduce_rows" => {
            let table = parse_named_attr(&tail, "table=")?;
            let mask = parse_named_attr(&tail, "mask=")?;
            let field = parse_named_attr(&tail, "field=")?;
            let rows = parse_rows_attr(&tail)?;
            Ok(OperationKind::ReduceRows {
                table,
                mask,
                field,
                rows,
            })
        }
        _ => Err(TextParseError::new(format!("unsupported op kind `{tag}`"))),
    }
}

fn parse_named_attr(text: &str, key: &str) -> Result<String, TextParseError> {
    text.split_whitespace()
        .find_map(|word| word.strip_prefix(key))
        .map(parse_quoted_or_token)
        .ok_or_else(|| TextParseError::new(format!("missing attribute `{key}` in `{text}`")))
}

fn parse_rows_attr(text: &str) -> Result<u64, TextParseError> {
    parse_named_attr(text, "rows=")?
        .parse::<u64>()
        .map_err(|_| TextParseError::new(format!("invalid rows attribute in `{text}`")))
}

fn split_operands_and_results(rest: &str) -> (&str, &str) {
    rest.split_once(" -> ").unwrap_or((rest, ""))
}

fn parse_value_ids(text: &str) -> Result<Vec<ValueId>, TextParseError> {
    let mut values = Vec::new();
    for word in text.split_whitespace() {
        if let Some(parsed) = parse_value_token(word)? {
            values.push(parsed);
        }
    }
    Ok(values)
}

fn parse_values(text: &str) -> Result<Vec<ValueId>, TextParseError> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    text.split_whitespace()
        .map(|word| {
            parse_value_token(word)?
                .ok_or_else(|| TextParseError::new(format!("expected value result `{word}`")))
        })
        .collect()
}

fn parse_value_token(word: &str) -> Result<Option<ValueId>, TextParseError> {
    let Some(digits) = word.strip_prefix('v') else {
        return Ok(None);
    };
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(None);
    }
    digits
        .parse::<u32>()
        .map(ValueId::new)
        .map(Some)
        .map_err(|_| TextParseError::new(format!("bad value id {word}")))
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use crate::mir::{
        BlockData, Effect, EffectSet, MirModule, MirType, OperationData, OperationFacts,
        OperationKind, OwnershipMode, RegionData, ScalarType, TrapProvenance, ValueData,
    };

    #[test]
    fn parser_round_trips_rendered_module() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.F.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let none = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![none],
                Vec::new(),
                EffectSet::empty(),
            ),
        );

        let text = crate::mir::text::render_module(&module);
        let parsed = super::parse_module(&text).expect("round-trip parse succeeds");
        assert_eq!(crate::mir::text::render_module(&parsed), text);
        assert!(crate::mir::module_eq::modules_semantically_equal(
            &module, &parsed
        ));
    }

    #[test]
    fn block_before_region_returns_error() {
        let text = "wmir v1\n  block b0 entry\n";
        let err = super::parse_module(text).expect_err("block before region must fail");
        assert!(err.message().contains("before any region"));
    }

    #[test]
    fn parser_accepts_gamma_theta_delta_regions() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::gamma("g".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let value = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![value],
                Vec::new(),
                EffectSet::empty(),
            ),
        );
        let text = crate::mir::text::render_module(&module);
        let parsed = super::parse_module(&text).expect("gamma region round-trips");
        assert!(crate::mir::module_eq::modules_semantically_equal(
            &module, &parsed
        ));
    }

    #[test]
    fn parser_round_trips_effects_and_facts() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("test".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let value = module.push_value(ValueData::new(MirType::Scalar(ScalarType::U64)));
        let mut facts = OperationFacts::empty();
        facts.set_ownership(vec![OwnershipMode::Read, OwnershipMode::Mut]);
        facts.set_trap(Some(TrapProvenance::new("overflow".to_string())));
        facts.set_state_edge(true);
        facts.set_capacity_token(true);
        module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![value],
                Vec::new(),
                EffectSet::single(Effect::Trap).union(EffectSet::single(Effect::Io)),
            )
            .with_facts(facts),
        );

        let text = crate::mir::text::render_module(&module);
        let parsed = super::parse_module(&text).expect("parse");
        assert_eq!(crate::mir::text::render_module(&parsed), text);
        assert!(crate::mir::module_eq::modules_semantically_equal(
            &module, &parsed
        ));
    }
}
