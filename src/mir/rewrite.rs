use std::collections::{BTreeMap, BTreeSet};

use super::{
    AeGraph, BlockId, CERTIFICATE_VERSION, Enode, MirModule, OperationId, OperationKind, Pass,
    PassSet, RegionId, RewriteCertificate, RewriteEvent, RewriteEventLog, RewriteFact,
    RewriteOutcome, RewriteRule, StableHash, TargetProfile, ValueId, stable_hash_text,
};
use crate::mir::dataplane::{MaskProvenance, MaskValueKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseOptimizeReport {
    regions_visited: usize,
    pure_ops_lifted: usize,
    rewrites_applied: usize,
    fuel_used: usize,
    dead_ops_removed: usize,
    before_hash: StableHash,
    after_hash: StableHash,
    events: RewriteEventLog,
    details: Vec<String>,
}

impl ReleaseOptimizeReport {
    pub fn regions_visited(&self) -> usize {
        self.regions_visited
    }

    pub fn pure_ops_lifted(&self) -> usize {
        self.pure_ops_lifted
    }

    pub fn rewrites_applied(&self) -> usize {
        self.rewrites_applied
    }

    pub fn fuel_used(&self) -> usize {
        self.fuel_used
    }

    pub fn dead_ops_removed(&self) -> usize {
        self.dead_ops_removed
    }

    pub fn before_hash(&self) -> StableHash {
        self.before_hash
    }

    pub fn after_hash(&self) -> StableHash {
        self.after_hash
    }

    pub fn rewrite_events(&self) -> &[RewriteEvent] {
        self.events.events()
    }

    pub fn events_truncated(&self) -> bool {
        self.events.truncated()
    }

    pub fn details(&self) -> &[String] {
        &self.details
    }

    pub fn push_detail(&mut self, detail: impl Into<String>) {
        self.details.push(detail.into());
    }

    pub fn trident_failure(&self) -> Option<&'static str> {
        for detail in &self.details {
            if detail.contains("ledger-corrupt") {
                return Some("ledger-corrupt");
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct ReleaseOptimizeResult {
    module: MirModule,
    report: ReleaseOptimizeReport,
}

impl ReleaseOptimizeResult {
    pub fn module(&self) -> &MirModule {
        &self.module
    }

    pub fn report(&self) -> &ReleaseOptimizeReport {
        &self.report
    }
}

pub fn optimize_release(module: &MirModule, passes: &PassSet) -> ReleaseOptimizeResult {
    let before_text = crate::mir::text::render_module(module);
    let before_hash = stable_hash_text("wmir.module.v0", &before_text);
    let mut report = ReleaseOptimizeReport {
        regions_visited: 0,
        pure_ops_lifted: 0,
        rewrites_applied: 0,
        fuel_used: 0,
        dead_ops_removed: 0,
        before_hash,
        after_hash: before_hash,
        events: RewriteEventLog::new(),
        details: Vec::new(),
    };

    for region in module.regions() {
        report.regions_visited += 1;
        let mut graph = AeGraph::new();
        for block in region.blocks() {
            for op_id in module.block(*block).operations() {
                let op = module.operation(*op_id);
                if is_pure_liftable(op.kind()) && op.effects().bits() == 0 {
                    graph.intern(Enode::new(
                        op.kind().clone(),
                        op.operands().to_vec(),
                        *op_id,
                    ));
                }
            }
        }
        report.pure_ops_lifted += graph.enode_count();
    }

    let mut output = module.clone();
    let mut fuel = 128usize;

    if passes.enabled(Pass::ScalarPeephole) {
        apply_scalar_add_zero(&mut output, &mut report, &mut fuel);
    }
    if passes.enabled(Pass::MaskAlgebra) {
        apply_mask_algebra(&mut output, &mut report, &mut fuel);
    }
    if passes.enabled(Pass::BranchSelect) {
        apply_branch_select(&output, &mut report, &mut fuel);
    }
    if passes.enabled(Pass::TableLoopFusion) {
        apply_table_loop_fusion(&mut output, &mut report, &mut fuel);
    }
    if passes.enabled(Pass::DeadCode) {
        let removed = remove_unused_pure_operations(&mut output);
        report.dead_ops_removed += removed;
    }
    let after_text = crate::mir::text::render_module(&output);
    report.after_hash = stable_hash_text("wmir.module.v0", &after_text);

    ReleaseOptimizeResult {
        module: output,
        report,
    }
}

fn is_pure_liftable(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Literal(_)
            | OperationKind::ReadValue(_)
            | OperationKind::Binary(_)
            | OperationKind::Let(_)
            | OperationKind::TableRows { .. }
            | OperationKind::MaskRead { .. }
            | OperationKind::MaskAllTrue { .. }
            | OperationKind::MaskAllFalse { .. }
            | OperationKind::MaskNot
            | OperationKind::MaskAnd
            | OperationKind::MaskOr
            | OperationKind::RowToken { .. }
    )
}

fn apply_scalar_add_zero(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let zero_values = literal_zero_values(module);
    let overlay = build_pure_overlay(module);
    for enode in overlay.enodes() {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let op_id = enode.source();
        let Some(region_id) = containing_region(module, op_id) else {
            continue;
        };
        let op = module.operation(op_id).clone();
        let OperationKind::Binary(operator) = op.kind() else {
            continue;
        };
        if operator != "+" || op.operands().len() != 2 || op.results().len() != 1 {
            continue;
        }

        let left = op.operands()[0];
        let right = op.operands()[1];
        let replacement = if zero_values.contains_key(&right) {
            Some((left, right))
        } else if zero_values.contains_key(&left) {
            Some((right, left))
        } else {
            None
        };
        let Some((replacement_value, zero_value)) = replacement else {
            continue;
        };

        let before_hash =
            stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
        let mut candidate = module.clone();
        candidate.replace_value_uses(op.results()[0], replacement_value);
        let removed = remove_unused_pure_operations(&mut candidate);
        let verify = crate::mir::verify_module(&candidate);
        let after_hash = stable_hash_text(
            "wmir.module.v0",
            &crate::mir::text::render_module(&candidate),
        );
        let outcome = if verify.ok() {
            RewriteOutcome::PassedVerifier
        } else {
            let _ = crate::mir::preserve_failure_artifact(
                "rewrite-verifier-failure",
                "wmir.failed-rewrite.v1",
                &candidate,
            );
            RewriteOutcome::FailedVerifier
        };
        let mut cert_facts = vec![
            RewriteFact::PureOperation { op: op_id },
            RewriteFact::LiteralZero {
                value: format!("v{}", zero_value.raw()),
            },
            RewriteFact::EffectAbsent { effect: "all" },
        ];
        if verify.ok() {
            cert_facts.push(RewriteFact::VerifierPassed);
        }
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            region_id,
            vec![op_id],
            before_hash,
            after_hash,
            cert_facts,
            outcome,
        );
        if !cert.can_record_in_ledger() {
            continue;
        }
        if !report.events.push(cert) {
            continue;
        }
        if !verify.ok() {
            continue;
        }

        *module = candidate;
        report.rewrites_applied += 1;
        report.fuel_used += 1;
        report.dead_ops_removed += removed;
        *fuel -= 1;
    }
}

fn containing_region(module: &MirModule, op_id: OperationId) -> Option<RegionId> {
    for (region_index, region) in module.regions().iter().enumerate() {
        for block in region.blocks() {
            if module.block(*block).operations().contains(&op_id) {
                return Some(RegionId::new(region_index as u32));
            }
        }
    }
    None
}

fn literal_zero_values(module: &MirModule) -> BTreeMap<ValueId, OperationId> {
    let mut values = BTreeMap::new();
    for (index, op) in module.operations().iter().enumerate() {
        if matches!(op.kind(), OperationKind::Literal(text) if text == "0") {
            if let Some(value) = op.results().first().copied() {
                values.insert(value, OperationId::new(index as u32));
            }
        }
    }
    values
}

fn build_pure_overlay(module: &MirModule) -> AeGraph {
    let mut graph = AeGraph::new();
    for (index, op) in module.operations().iter().enumerate() {
        if op.effects().bits() == 0 && is_pure_liftable(op.kind()) {
            graph.intern(Enode::new(
                op.kind().clone(),
                op.operands().to_vec(),
                OperationId::new(index as u32),
            ));
        }
    }
    graph
}

fn block_reachable_used_values(module: &MirModule) -> BTreeSet<ValueId> {
    let mut used = BTreeSet::new();
    for block_index in 0..module.blocks().len() {
        let block = BlockId::new(block_index as u32);
        for op_id in module.block(block).operations() {
            let op = module.operation(*op_id);
            used.extend(op.operands().iter().copied());
        }
    }
    used
}

fn remove_unused_pure_operations(module: &mut MirModule) -> usize {
    let mut total_removed = 0usize;
    loop {
        let used = block_reachable_used_values(module);
        let mut removed = 0usize;

        for block_index in 0..module.blocks().len() {
            let block = BlockId::new(block_index as u32);
            let retained = module
                .block(block)
                .operations()
                .iter()
                .copied()
                .filter(|op_id| {
                    let op = module.operation(*op_id);
                    let removable = op.effects().bits() == 0
                        && !op.results().is_empty()
                        && op.results().iter().all(|result| !used.contains(result));
                    if removable {
                        removed += 1;
                    }
                    !removable
                })
                .collect::<Vec<_>>();
            module.set_block_operations(block, retained);
        }

        if removed == 0 {
            break;
        }
        total_removed += removed;
    }
    total_removed
}

fn mask_provenance_by_value(module: &MirModule) -> BTreeMap<ValueId, MaskProvenance> {
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
            OperationKind::MaskNot if op.operands().len() == 1 => {
                if let (Some(operand), Some(result)) = (op.operands().first(), op.results().first())
                {
                    if let Some(provenance) = map.get(operand) {
                        map.insert(*result, provenance.clone());
                    }
                }
            }
            _ => {}
        }
    }
    map
}

fn apply_mask_algebra(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let provenance = mask_provenance_by_value(module);
    let authority = crate::mir::AuthorityQuery::new();
    for op_index in 0..module.operations().len() {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let op_id = OperationId::new(op_index as u32);
        let op = module.operation(op_id).clone();
        let Some((rule, replacement, domain_fact)) = (match op.kind() {
            OperationKind::MaskAnd if op.operands().len() == 2 && op.results().len() == 1 => {
                mask_and_rewrite(&provenance, &authority, &op)
            }
            OperationKind::MaskOr if op.operands().len() == 2 && op.results().len() == 1 => {
                mask_or_rewrite(&provenance, &authority, &op)
            }
            OperationKind::MaskNot if op.operands().len() == 1 && op.results().len() == 1 => {
                mask_double_not_rewrite(module, &provenance, &authority, &op)
            }
            _ => None,
        }) else {
            continue;
        };
        let Some(region_id) = containing_region(module, op_id) else {
            continue;
        };

        let before_hash =
            stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
        let mut candidate = module.clone();
        candidate.replace_value_uses(op.results()[0], replacement);
        let removed = remove_unused_pure_operations(&mut candidate);
        let verify = crate::mir::verify_module(&candidate);
        let after_hash = stable_hash_text(
            "wmir.module.v0",
            &crate::mir::text::render_module(&candidate),
        );
        let outcome = if verify.ok() {
            RewriteOutcome::PassedVerifier
        } else {
            let _ = crate::mir::preserve_failure_artifact(
                "rewrite-verifier-failure",
                "wmir.failed-rewrite.v1",
                &candidate,
            );
            RewriteOutcome::FailedVerifier
        };
        let mut cert_facts = vec![
            domain_fact.as_rewrite_fact(),
            RewriteFact::PureOperation { op: op_id },
        ];
        if verify.ok() {
            cert_facts.push(RewriteFact::VerifierPassed);
        }
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            rule,
            Pass::MaskAlgebra,
            region_id,
            vec![op_id],
            before_hash,
            after_hash,
            cert_facts,
            outcome,
        );
        if !cert.can_record_in_ledger() {
            continue;
        }
        if !report.events.push(cert) {
            continue;
        }
        if !verify.ok() {
            continue;
        }
        *module = candidate;
        report.rewrites_applied += 1;
        report.dead_ops_removed += removed;
        report.fuel_used += 1;
        *fuel -= 1;
    }
}

fn mask_and_rewrite(
    provenance: &BTreeMap<ValueId, MaskProvenance>,
    authority: &crate::mir::AuthorityQuery,
    op: &super::OperationData,
) -> Option<(RewriteRule, ValueId, crate::mir::AuthorityFact)> {
    let left = op.operands()[0];
    let right = op.operands()[1];
    let left_prov = provenance.get(&left)?;
    let right_prov = provenance.get(&right)?;
    let domain_fact = authority.same_mask_domain(left_prov, right_prov)?;
    if left == right {
        return Some((RewriteRule::MaskAndSelf, left, domain_fact));
    }
    if right_prov.kind() == MaskValueKind::AllTrue {
        return Some((RewriteRule::MaskAndTrue, left, domain_fact));
    }
    if left_prov.kind() == MaskValueKind::AllTrue {
        return Some((RewriteRule::MaskAndTrue, right, domain_fact));
    }
    None
}

fn mask_or_rewrite(
    provenance: &BTreeMap<ValueId, MaskProvenance>,
    authority: &crate::mir::AuthorityQuery,
    op: &super::OperationData,
) -> Option<(RewriteRule, ValueId, crate::mir::AuthorityFact)> {
    let left = op.operands()[0];
    let right = op.operands()[1];
    let left_prov = provenance.get(&left)?;
    let right_prov = provenance.get(&right)?;
    let domain_fact = authority.same_mask_domain(left_prov, right_prov)?;
    if left == right {
        return Some((RewriteRule::MaskOrSelf, left, domain_fact));
    }
    if right_prov.kind() == MaskValueKind::AllFalse {
        return Some((RewriteRule::MaskOrFalse, left, domain_fact));
    }
    if left_prov.kind() == MaskValueKind::AllFalse {
        return Some((RewriteRule::MaskOrFalse, right, domain_fact));
    }
    None
}

fn mask_double_not_rewrite(
    module: &MirModule,
    provenance: &BTreeMap<ValueId, MaskProvenance>,
    authority: &crate::mir::AuthorityQuery,
    op: &super::OperationData,
) -> Option<(RewriteRule, ValueId, crate::mir::AuthorityFact)> {
    let inner = op.operands()[0];
    let inner_op = module
        .operations()
        .iter()
        .find(|candidate| candidate.results().first() == Some(&inner))?;
    if !matches!(inner_op.kind(), OperationKind::MaskNot) {
        return None;
    }
    let original = inner_op.operands()[0];
    let original_prov = provenance.get(&original)?;
    let inner_prov = provenance.get(&inner)?;
    let domain_fact = authority.same_mask_domain(original_prov, inner_prov)?;
    Some((RewriteRule::MaskDoubleNot, original, domain_fact))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReduceFacts {
    op: OperationId,
    region: RegionId,
    block: BlockId,
    table: String,
    mask: String,
    field: String,
    rows: u64,
    authority: crate::mir::LoopAuthorityFacts,
}

fn reduce_facts(module: &MirModule) -> Vec<ReduceFacts> {
    let mut facts = Vec::new();
    for (region_index, region) in module.regions().iter().enumerate() {
        let region_id = RegionId::new(region_index as u32);
        for block in region.blocks() {
            for op_id in module.block(*block).operations() {
                let op = module.operation(*op_id);
                if let OperationKind::ReduceRows {
                    table,
                    mask,
                    field,
                    rows,
                } = op.kind()
                {
                    facts.push(ReduceFacts {
                        op: *op_id,
                        region: region_id,
                        block: *block,
                        table: table.clone(),
                        mask: mask.clone(),
                        field: field.clone(),
                        rows: *rows,
                        authority: crate::mir::LoopAuthorityFacts::new(table.clone(), *rows)
                            .with_mask(mask.clone())
                            .with_mutation_place(format!("acc:{field}"))
                            .with_effects(op.effects())
                            .with_row_token_escapes(false)
                            .with_state_token(format!("table:{table}"))
                            .with_trap_order_index(op_id.raw()),
                    });
                }
            }
        }
    }
    facts
}

fn apply_table_loop_fusion(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let engine = crate::mir::AuthorityEngine::new();
    let ledger_dir = std::path::PathBuf::from("target")
        .join("wrela")
        .join("ledger");
    let ledger =
        crate::mir::OptimizationLedger::open(ledger_dir, TargetProfile::GenericAArch64).ok();
    if let Some(ref ledger) = ledger {
        if ledger.corrupt_lines() > 0 {
            let _ = crate::mir::preserve_failure_artifact(
                "ledger-corrupt",
                "wmir.ledger-corrupt.v1",
                module,
            );
            report.push_detail("table-loop-fusion:disabled:ledger-corrupt");
            return;
        }
    }
    loop {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let facts = reduce_facts(module);
        let mut applied = false;
        for pair in facts.windows(2) {
            let first = &pair[0];
            let second = &pair[1];
            let first_op = module.operation(first.op);
            let second_op = module.operation(second.op);
            if first.region != second.region || first.block != second.block {
                continue;
            }
            if first_op.facts().fused_into().is_some()
                || second_op.facts().fused_into().is_some()
                || first_op.facts().fused_with().is_some()
            {
                continue;
            }
            let Some(authority_facts) =
                engine.table_fusion_facts(&first.authority, &second.authority)
            else {
                continue;
            };
            let before_hash =
                stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
            let key = crate::mir::OptimizerKey::new(
                before_hash,
                TargetProfile::GenericAArch64,
                &PassSet::only(Pass::TableLoopFusion),
                env!("CARGO_PKG_VERSION"),
                CERTIFICATE_VERSION,
            );
            let decision = ledger
                .as_ref()
                .map(|ledger| crate::mir::CostDecision::from_ledger(ledger, &key))
                .unwrap_or(crate::mir::CostDecision::UseStaticEstimate);
            report.push_detail(format!("cost-decision:{decision:?}"));
            if decision == crate::mir::CostDecision::AvoidMeasuredLoser {
                continue;
            }
            let mut candidate = module.clone();
            candidate.mark_reduce_pair_fused(first.op, second.op);
            let region_id = first.region;
            let verify = crate::mir::verify_module(&candidate);
            let after_hash = stable_hash_text(
                "wmir.module.v0",
                &crate::mir::text::render_module(&candidate),
            );
            let outcome = if verify.ok() {
                RewriteOutcome::PassedVerifier
            } else {
                let _ = crate::mir::preserve_failure_artifact(
                    "table-fusion-verifier-failure",
                    "wmir.failed-table-fusion.v1",
                    &candidate,
                );
                RewriteOutcome::FailedVerifier
            };
            let mut cert_facts = authority_facts
                .iter()
                .map(|fact| fact.as_rewrite_fact())
                .collect::<Vec<_>>();
            if verify.ok() {
                cert_facts.push(RewriteFact::VerifierPassed);
            }
            let cert = RewriteCertificate::new(
                CERTIFICATE_VERSION,
                RewriteRule::TableLoopFusion,
                Pass::TableLoopFusion,
                region_id,
                vec![first.op, second.op],
                before_hash,
                after_hash,
                cert_facts,
                outcome,
            );
            if !cert.can_record_in_ledger() {
                continue;
            }
            if !report.events.push(cert) {
                continue;
            }
            if !verify.ok() {
                continue;
            }
            *module = candidate;
            report.rewrites_applied += 1;
            report.fuel_used += 1;
            *fuel -= 1;
            applied = true;
            break;
        }
        if !applied {
            break;
        }
    }
}

fn apply_branch_select(module: &MirModule, report: &mut ReleaseOptimizeReport, fuel: &mut usize) {
    let costs = TargetProfile::GenericAArch64.costs();
    report.push_detail(format!(
        "branch-select:profile={}",
        TargetProfile::GenericAArch64.name()
    ));
    let before_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
    for (index, region) in module.regions().iter().enumerate() {
        let in_loop = matches!(region.kind(), super::RegionKind::Theta(_));
        let pure_ops = count_pure_scalar_ops_in_region(module, region);
        // Branch predictability is not modeled in MIR yet; only loop context is known.
        let predictable_branch = false;
        if !costs.should_if_convert(in_loop, predictable_branch, pure_ops) {
            continue;
        }
        report.push_detail(format!(
            "branch-select:candidate:r{index}:pure_ops={pure_ops}"
        ));
        if *fuel == 0 || report.events_truncated() {
            continue;
        }
        let region_id = RegionId::new(index as u32);
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::BranchSelect,
            Pass::BranchSelect,
            region_id,
            Vec::new(),
            before_hash,
            before_hash,
            vec![RewriteFact::Authority {
                name: "branch-select-candidate",
                detail: format!("pure_ops={pure_ops}:in_loop={in_loop}"),
            }],
            RewriteOutcome::PassedVerifier,
        );
        if !cert.can_record_in_ledger() {
            continue;
        }
        if report.events.push(cert) {
            report.fuel_used += 1;
            *fuel -= 1;
        }
    }
}

fn count_pure_scalar_ops_in_region(module: &MirModule, region: &super::RegionData) -> u8 {
    let mut count = 0u8;
    for block in region.blocks() {
        for op_id in module.block(*block).operations() {
            let op = module.operation(*op_id);
            if op.effects().bits() != 0 {
                continue;
            }
            if matches!(
                op.kind(),
                OperationKind::Literal(_)
                    | OperationKind::ReadValue(_)
                    | OperationKind::Binary(_)
                    | OperationKind::Let(_)
            ) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{Pass, PassSet, optimize_release};

    #[test]
    fn mask_algebra_rewrites_and_true_for_same_table() {
        let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
        let passes = PassSet::only(Pass::MaskAlgebra);
        let optimized = optimize_release(&module, &passes);

        assert_eq!(optimized.report().rewrites_applied(), 1);
        assert_eq!(optimized.report().rewrite_events().len(), 1);
        let cert = optimized.report().rewrite_events()[0].certificate();
        assert_eq!(cert.rule().name(), "mask-and-true");
        assert!(
            cert.facts()
                .iter()
                .any(|fact| format!("{fact:?}").contains("same-mask-domain"))
        );
        assert!(!crate::mir::text::render_module(optimized.module()).contains("op mask_and"));
    }

    #[test]
    fn mask_algebra_refuses_different_row_counts() {
        let module =
            crate::mir::dataplane::testing::mask_and_mismatched_true_module("packets", 256, 128);
        let passes = PassSet::only(Pass::MaskAlgebra);
        let optimized = optimize_release(&module, &passes);

        assert_eq!(optimized.report().rewrites_applied(), 0);
        assert_eq!(optimized.report().rewrite_events().len(), 0);
    }

    #[test]
    fn dataplane_release_reports_mask_certificate_details() {
        let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
        let passes = PassSet::only(Pass::MaskAlgebra);
        let optimized = optimize_release(&module, &passes);

        assert_eq!(optimized.report().rewrites_applied(), 1);
        assert_eq!(optimized.report().rewrite_events().len(), 1);
        let cert = optimized.report().rewrite_events()[0].certificate();
        assert_eq!(cert.rule().name(), "mask-and-true");
        assert!(cert.facts().iter().any(|fact| {
            format!("{fact:?}").contains("same-mask-domain")
                && format!("{fact:?}").contains("packets:256")
        }));
        assert!(crate::mir::verify_module(optimized.module()).ok());
    }

    #[test]
    fn mask_algebra_rewrites_double_not_to_original_mask() {
        let module = crate::mir::dataplane::testing::mask_double_not_module("packets", 256);
        let passes = PassSet::only(Pass::MaskAlgebra);
        let optimized = optimize_release(&module, &passes);

        assert_eq!(optimized.report().rewrites_applied(), 1);
        let cert = optimized.report().rewrite_events()[0].certificate();
        assert_eq!(cert.rule().name(), "mask-double-not");
        let rendered = crate::mir::text::render_module(optimized.module());
        assert!(
            !rendered
                .lines()
                .any(|line| line.contains("    op ") && line.contains(" mask_not")),
            "{rendered}"
        );
    }

    #[test]
    fn dead_code_pass_is_gated_by_pass_set() {
        let mut module = crate::mir::MirModule::empty_for_test();
        let unused = module.push_value(crate::mir::ValueData::new(crate::mir::MirType::Scalar(
            crate::mir::ScalarType::U64,
        )));
        let used = module.push_value(crate::mir::ValueData::new(crate::mir::MirType::Scalar(
            crate::mir::ScalarType::U64,
        )));
        module.push_test_operation(crate::mir::OperationData::new(
            crate::mir::OperationKind::Literal("99".to_string()),
            Vec::new(),
            vec![unused],
            crate::mir::EffectSet::empty(),
        ));
        module.push_test_operation(crate::mir::OperationData::new(
            crate::mir::OperationKind::Return,
            vec![used],
            Vec::new(),
            crate::mir::EffectSet::empty(),
        ));

        let without = optimize_release(&module, &PassSet::empty());
        assert_eq!(without.report().dead_ops_removed(), 0);
        assert!(crate::mir::text::render_module(without.module()).contains("literal"));

        let with_dead = optimize_release(&module, &PassSet::only(Pass::DeadCode));
        assert_eq!(with_dead.report().dead_ops_removed(), 1);
        assert!(!crate::mir::text::render_module(with_dead.module()).contains("literal"));
    }

    #[test]
    fn table_loop_fusion_does_not_cross_lambda_regions() {
        use crate::mir::{
            BlockData, Effect, EffectSet, MirModule, OperationData, OperationKind, RegionData,
            ValueData,
        };

        fn push_compatible_reduce(
            module: &mut MirModule,
            block: crate::mir::BlockId,
        ) -> crate::mir::OperationId {
            let result = module.push_value(ValueData::new(crate::mir::MirType::Scalar(
                crate::mir::ScalarType::U64,
            )));
            let mut facts = crate::mir::OperationFacts::empty();
            facts.set_state_edge(true);
            module.push_operation(
                block,
                OperationData::new(
                    OperationKind::ReduceRows {
                        table: "packets".to_string(),
                        mask: "valid".to_string(),
                        field: "len".to_string(),
                        rows: 256,
                    },
                    Vec::new(),
                    vec![result],
                    EffectSet::single(Effect::Mutate),
                )
                .with_facts(facts),
            )
        }

        let mut module = MirModule::new();
        let region_a = module.push_region(RegionData::lambda("bench.fn_a".to_string()));
        let block_a = module.push_block(region_a, BlockData::new("entry".to_string()));
        push_compatible_reduce(&mut module, block_a);

        let region_b = module.push_region(RegionData::lambda("bench.fn_b".to_string()));
        let block_b = module.push_block(region_b, BlockData::new("entry".to_string()));
        push_compatible_reduce(&mut module, block_b);

        let optimized = optimize_release(&module, &PassSet::only(Pass::TableLoopFusion));
        assert_eq!(optimized.report().rewrites_applied(), 0);
        let text = crate::mir::text::render_module(optimized.module());
        assert!(!text.contains("fact fused_with="));
        assert!(!text.contains("fact fused_into="));
    }

    #[test]
    fn table_loop_fusion_skips_when_ledger_is_corrupt() {
        let dir = std::path::PathBuf::from("target")
            .join("wrela")
            .join("ledger");
        std::fs::create_dir_all(&dir).expect("ledger dir");
        let path = dir.join("generic-aarch64.ledger");
        let prior = std::fs::read_to_string(&path).ok();
        std::fs::write(&path, "not a ledger line\n").expect("write corrupt ledger");

        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/mir/dataplane/filter_sum.wrela");
        let check = crate::check::check_root(&root);
        assert!(check.ok());
        let mir = crate::mir::build_mir(&check);
        let module = mir.module().expect("mir module");
        let optimized = optimize_release(module, &PassSet::only(Pass::TableLoopFusion));

        if let Some(text) = prior {
            std::fs::write(&path, text).expect("restore ledger");
        } else {
            let _ = std::fs::remove_file(&path);
        }

        assert_eq!(optimized.report().rewrites_applied(), 0);
        assert!(
            optimized
                .report()
                .details()
                .iter()
                .any(|detail| detail.contains("ledger-corrupt"))
        );
    }
}
