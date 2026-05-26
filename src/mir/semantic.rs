use crate::check::cst::CstView;
use crate::check::summary::AccessMode;
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::effect::Effect;
use super::facts::{OperationFacts, OwnershipMode};
use super::{EffectSet, OperationData, OperationKind, ValueId};

pub fn ownership_for_access(access: AccessMode) -> OwnershipMode {
    match access {
        AccessMode::Read => OwnershipMode::Read,
        AccessMode::Mut => OwnershipMode::Mut,
        AccessMode::Own => OwnershipMode::Own,
    }
}

pub fn infer_mir_effects(view: &CstView<'_>, node: SyntaxNodeId) -> EffectSet {
    match view.node_kind(node) {
        SyntaxKind::CallExpr
        | SyntaxKind::LoopStmt
        | SyntaxKind::TryExpr
        | SyntaxKind::ReduceExpr
        | SyntaxKind::ScanExpr => EffectSet::single(Effect::Mutate),
        _ => {
            let mut effects = EffectSet::empty();
            for child in view.child_nodes(node) {
                effects = effects.union(infer_mir_effects(view, child));
            }
            effects
        }
    }
}

pub fn make_operation(
    kind: OperationKind,
    operands: Vec<ValueId>,
    results: Vec<ValueId>,
    effects: EffectSet,
    ownership: Vec<OwnershipMode>,
) -> OperationData {
    let mut facts = OperationFacts::empty();
    facts.set_ownership(ownership);
    OperationData::new(kind, operands, results, effects).with_facts(facts)
}

pub fn make_operation_with_facts(
    kind: OperationKind,
    operands: Vec<ValueId>,
    results: Vec<ValueId>,
    effects: EffectSet,
    ownership: Vec<OwnershipMode>,
    configure: impl FnOnce(&mut OperationFacts),
) -> OperationData {
    let mut facts = OperationFacts::empty();
    facts.set_ownership(ownership);
    configure(&mut facts);
    OperationData::new(kind, operands, results, effects).with_facts(facts)
}

pub fn dataplane_reduce_facts() -> OperationFacts {
    let mut facts = OperationFacts::empty();
    facts.set_state_edge(true);
    facts
}
