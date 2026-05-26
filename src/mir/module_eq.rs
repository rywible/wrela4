use super::{BlockData, MirModule, OperationData, OperationFacts, RegionData};

pub fn modules_semantically_equal(left: &MirModule, right: &MirModule) -> bool {
    if left.regions().len() != right.regions().len()
        || left.blocks().len() != right.blocks().len()
        || left.operations().len() != right.operations().len()
        || left.values().len() != right.values().len()
        || left.places().len() != right.places().len()
    {
        return false;
    }

    for (index, region) in left.regions().iter().enumerate() {
        if !regions_equal(region, &right.regions()[index]) {
            return false;
        }
    }

    for (index, block) in left.blocks().iter().enumerate() {
        if !blocks_equal(block, &right.blocks()[index]) {
            return false;
        }
    }

    for (index, value) in left.values().iter().enumerate() {
        if value.ty() != right.values()[index].ty() {
            return false;
        }
    }

    for (index, place) in left.places().iter().enumerate() {
        let other = &right.places()[index];
        if place.name() != other.name() || place.ty() != other.ty() {
            return false;
        }
    }

    for (index, op) in left.operations().iter().enumerate() {
        if !operations_equal(op, &right.operations()[index]) {
            return false;
        }
    }

    true
}

fn regions_equal(left: &RegionData, right: &RegionData) -> bool {
    left.kind() == right.kind() && left.blocks() == right.blocks()
}

fn blocks_equal(left: &BlockData, right: &BlockData) -> bool {
    left.name() == right.name()
        && left.arguments() == right.arguments()
        && left.operations() == right.operations()
}

fn operations_equal(left: &OperationData, right: &OperationData) -> bool {
    left.kind() == right.kind()
        && left.operands() == right.operands()
        && left.results() == right.results()
        && left.effects() == right.effects()
        && facts_equal(left.facts(), right.facts())
}

fn facts_equal(left: &OperationFacts, right: &OperationFacts) -> bool {
    left.ownership() == right.ownership()
        && left.capabilities() == right.capabilities()
        && left.trap() == right.trap()
        && left.has_state_edge() == right.has_state_edge()
        && left.has_capacity_token() == right.has_capacity_token()
        && left.fused_with() == right.fused_with()
        && left.fused_into() == right.fused_into()
}

#[cfg(test)]
mod tests {
    use super::modules_semantically_equal;
    use crate::mir::{
        CapabilityPath, Effect, EffectSet, MirModule, MirType, OperationData, OperationFacts,
        OperationKind, OwnershipMode, ScalarType, TrapProvenance, ValueData,
    };

    #[test]
    fn semantic_equality_compares_effects_and_facts() {
        let mut left = MirModule::empty_for_test();
        let mut right = MirModule::empty_for_test();
        let value = left.push_value(ValueData::new(MirType::Scalar(ScalarType::U64)));
        let mut facts = OperationFacts::empty();
        facts.set_ownership(vec![OwnershipMode::Read, OwnershipMode::Mut]);
        facts.push_capability(CapabilityPath::new(
            "Console".to_string(),
            vec!["write".to_string()],
        ));
        facts.set_trap(Some(TrapProvenance::new("overflow".to_string())));
        facts.set_state_edge(true);
        facts.set_capacity_token(true);
        let block = crate::mir::BlockId::new(0);
        left.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![value],
                Vec::new(),
                EffectSet::single(Effect::Trap),
            )
            .with_facts(facts.clone()),
        );

        let value = right.push_value(ValueData::new(MirType::Scalar(ScalarType::U64)));
        right.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                vec![value],
                Vec::new(),
                EffectSet::empty(),
            ),
        );

        assert!(!modules_semantically_equal(&left, &right));

        let text = crate::mir::text::render_module(&left);
        let parsed = crate::mir::parse_text::parse_module(&text).expect("parse");
        assert!(modules_semantically_equal(&left, &parsed));
    }
}
