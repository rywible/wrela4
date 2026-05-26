#[test]
fn authority_engine_proves_table_fusion_facts() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total")
        .with_effects(wrela::mir::EffectSet::empty())
        .with_row_token_escapes(false)
        .with_state_token("table:packets")
        .with_trap_order_index(10);
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:flagged")
        .with_effects(wrela::mir::EffectSet::empty())
        .with_row_token_escapes(false)
        .with_state_token("table:packets")
        .with_trap_order_index(11);

    let facts = engine.table_fusion_facts(&first, &second).unwrap();

    assert!(facts.iter().any(|fact| fact.name() == "same-table"));
    assert!(facts.iter().any(|fact| fact.name() == "same-row-count"));
    assert!(facts.iter().any(|fact| fact.name() == "compatible-masks"));
    assert!(
        facts
            .iter()
            .any(|fact| fact.name() == "disjoint-mutation-places")
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.name() == "state-edges-preserved")
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.name() == "trap-order-preserved")
    );
}

#[test]
fn authority_engine_rejects_escaping_row_token() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256).with_row_token_escapes(true);
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256);

    assert!(engine.table_fusion_facts(&first, &second).is_none());
}

#[test]
fn authority_engine_rejects_overlapping_mutation_places() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total");
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total");

    assert!(engine.table_fusion_facts(&first, &second).is_none());
}
