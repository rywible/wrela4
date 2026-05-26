use super::{Effect, EffectSet, RewriteFact, dataplane::MaskProvenance};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityFact {
    SameTable { table: String },
    SameMaskDomain { table: String, rows: u64 },
    SameRowCount { rows: u64 },
    EffectAbsent { effect: Effect },
}

impl AuthorityFact {
    pub fn as_rewrite_fact(&self) -> RewriteFact {
        match self {
            AuthorityFact::SameTable { table } => RewriteFact::Authority {
                name: "same-table",
                detail: table.clone(),
            },
            AuthorityFact::SameMaskDomain { table, rows } => RewriteFact::Authority {
                name: "same-mask-domain",
                detail: format!("{table}:{rows}"),
            },
            AuthorityFact::SameRowCount { rows } => RewriteFact::Authority {
                name: "same-row-count",
                detail: rows.to_string(),
            },
            AuthorityFact::EffectAbsent { effect } => RewriteFact::EffectAbsent {
                effect: effect.name(),
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthorityQuery;

impl AuthorityQuery {
    pub fn new() -> Self {
        Self
    }

    pub fn same_mask_domain(
        &self,
        left: &MaskProvenance,
        right: &MaskProvenance,
    ) -> Option<AuthorityFact> {
        if left.table() == right.table() && left.rows() == right.rows() {
            Some(AuthorityFact::SameMaskDomain {
                table: left.table().to_string(),
                rows: left.rows(),
            })
        } else {
            None
        }
    }

    pub fn effect_absent(&self, effects: EffectSet, effect: Effect) -> Option<AuthorityFact> {
        if effects.contains(effect) {
            None
        } else {
            Some(AuthorityFact::EffectAbsent { effect })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedAuthorityFact {
    name: &'static str,
    detail: String,
}

impl NamedAuthorityFact {
    pub fn new(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            detail: detail.into(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn as_rewrite_fact(&self) -> RewriteFact {
        RewriteFact::Authority {
            name: self.name,
            detail: self.detail.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopAuthorityFacts {
    table: String,
    rows: u64,
    mask: String,
    mutation_places: Vec<String>,
    effects: EffectSet,
    row_token_escapes: bool,
    state_token: String,
    trap_order_index: u32,
}

impl LoopAuthorityFacts {
    pub fn new(table: impl Into<String>, rows: u64) -> Self {
        Self {
            table: table.into(),
            rows,
            mask: String::new(),
            mutation_places: Vec::new(),
            effects: EffectSet::empty(),
            row_token_escapes: false,
            state_token: String::new(),
            trap_order_index: 0,
        }
    }

    pub fn with_mask(mut self, mask: impl Into<String>) -> Self {
        self.mask = mask.into();
        self
    }

    pub fn with_mutation_place(mut self, place: impl Into<String>) -> Self {
        self.mutation_places.push(place.into());
        self.mutation_places.sort();
        self.mutation_places.dedup();
        self
    }

    pub fn with_effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub fn with_row_token_escapes(mut self, escapes: bool) -> Self {
        self.row_token_escapes = escapes;
        self
    }

    pub fn with_state_token(mut self, token: impl Into<String>) -> Self {
        self.state_token = token.into();
        self
    }

    pub fn with_trap_order_index(mut self, index: u32) -> Self {
        self.trap_order_index = index;
        self
    }
}

fn loop_effects_allow_table_fusion(effects: EffectSet) -> bool {
    effects.is_empty() || effects == EffectSet::single(Effect::Mutate)
}

fn loop_effects_fact_label(effects: EffectSet) -> &'static str {
    if effects.is_empty() {
        "effect-absent"
    } else {
        "mutate-only"
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthorityEngine;

impl AuthorityEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn table_fusion_facts(
        &self,
        first: &LoopAuthorityFacts,
        second: &LoopAuthorityFacts,
    ) -> Option<Vec<NamedAuthorityFact>> {
        if first.table != second.table || first.rows != second.rows {
            return None;
        }
        if first.mask != second.mask {
            return None;
        }
        if first
            .mutation_places
            .iter()
            .any(|place| second.mutation_places.contains(place))
        {
            return None;
        }
        if !loop_effects_allow_table_fusion(first.effects)
            || !loop_effects_allow_table_fusion(second.effects)
        {
            return None;
        }
        if first.row_token_escapes || second.row_token_escapes {
            return None;
        }
        if first.state_token != second.state_token {
            return None;
        }
        if first.trap_order_index > second.trap_order_index {
            return None;
        }
        Some(vec![
            NamedAuthorityFact::new("same-table", first.table.clone()),
            NamedAuthorityFact::new("same-row-count", first.rows.to_string()),
            NamedAuthorityFact::new("compatible-masks", first.mask.clone()),
            NamedAuthorityFact::new(
                "disjoint-mutation-places",
                format!("{:?}|{:?}", first.mutation_places, second.mutation_places),
            ),
            NamedAuthorityFact::new(loop_effects_fact_label(first.effects), "all"),
            NamedAuthorityFact::new("row-token-non-escaping", first.table.clone()),
            NamedAuthorityFact::new("state-edges-preserved", first.state_token.clone()),
            NamedAuthorityFact::new(
                "trap-order-preserved",
                format!("{}..{}", first.trap_order_index, second.trap_order_index),
            ),
            NamedAuthorityFact::new("independent-ordering-domain", first.table.clone()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthorityFact, AuthorityQuery};
    use crate::mir::dataplane::MaskProvenance;

    #[test]
    fn authority_query_proves_same_mask_domain() {
        let query = AuthorityQuery::new();
        let left = MaskProvenance::new("valid".to_string(), "packets".to_string(), 256);
        let right = MaskProvenance::new("large".to_string(), "packets".to_string(), 256);

        assert_eq!(
            query.same_mask_domain(&left, &right),
            Some(AuthorityFact::SameMaskDomain {
                table: "packets".to_string(),
                rows: 256,
            })
        );
    }

    #[test]
    fn authority_query_refuses_mismatched_mask_domain() {
        let query = AuthorityQuery::new();
        let left = MaskProvenance::new("valid".to_string(), "packets".to_string(), 256);
        let right = MaskProvenance::new("small_valid".to_string(), "small".to_string(), 128);

        assert_eq!(query.same_mask_domain(&left, &right), None);
    }
}
