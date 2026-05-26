use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join("dataplane")
        .join("filter_sum.wrela")
}

#[test]
fn dataplane_mir_round_trips_table_mask_facts() {
    let check = wrela::check::check_root(fixture());
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok(), "{:?}", mir.diagnostics());
    let module = mir.module().unwrap();

    let text = wrela::mir::text::render_module(module);
    assert!(text.contains("table_rows packets rows=256"));
    assert!(text.contains("mask_read valid table=packets rows=256"));
    assert!(text.contains("row_token table=packets rows=256"));

    let parsed = wrela::mir::parse_text::parse_module(&text).unwrap();
    assert_eq!(wrela::mir::text::render_module(&parsed), text);
    assert!(wrela::mir::modules_semantically_equal(module, &parsed));
    assert!(wrela::mir::verify_module(&parsed).ok());
}

#[test]
fn table_loop_fusion_emits_authority_certificate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let check = wrela::check::check_root(&root);
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok(), "{:?}", mir.diagnostics());

    let passes = wrela::mir::PassSet::only(wrela::mir::Pass::TableLoopFusion);
    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);

    assert!(optimized.report().rewrites_applied() >= 1);
    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "table-loop-fusion");
    assert!(
        cert.facts()
            .iter()
            .any(|fact| format!("{fact:?}").contains("same-table"))
    );
    assert!(
        cert.facts()
            .iter()
            .any(|fact| format!("{fact:?}").contains("state-edges-preserved"))
    );
    assert!(
        cert.facts()
            .iter()
            .any(|fact| format!("{fact:?}").contains("trap-order-preserved"))
    );
    assert!(wrela::mir::verify_module(optimized.module()).ok());
    let text = wrela::mir::text::render_module(optimized.module());
    assert!(text.contains("fact fused_with="));
    assert!(text.contains("fact fused_into="));
    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_ne!(cert.before_hash(), cert.after_hash());
}

#[test]
fn debug_mir_why_rewrite_prints_certificate_facts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "debug".into(),
            "mir".into(),
            "--why-rewrite".into(),
            "0".into(),
            "--only-pass".into(),
            "table-loop-fusion".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&err));
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("rule: table-loop-fusion"));
    assert!(text.contains("same-table"));
    assert!(text.contains("before:"));
    assert!(text.contains("after:"));
}

#[test]
fn table_loop_fusion_does_not_fuse_across_separate_lambda_regions() {
    let check = wrela::check::check_root(fixture());
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok(), "{:?}", mir.diagnostics());
    let module = mir.module().unwrap();

    let reduce_count = module
        .operations()
        .iter()
        .filter(|op| matches!(op.kind(), wrela::mir::OperationKind::ReduceRows { .. }))
        .count();
    assert!(reduce_count >= 2, "fixture should have multiple reduces");

    let passes = wrela::mir::PassSet::only(wrela::mir::Pass::TableLoopFusion);
    let optimized = wrela::mir::optimize_release(module, &passes);
    for event in optimized.report().rewrite_events() {
        let cert = event.certificate();
        let ops = cert.source_ops();
        assert_eq!(ops.len(), 2);
        let first_region = containing_region(optimized.module(), ops[0]);
        let second_region = containing_region(optimized.module(), ops[1]);
        assert_eq!(first_region, second_region);
        assert_eq!(cert.region(), first_region.expect("region"));
    }
}

fn containing_region(
    module: &wrela::mir::MirModule,
    op_id: wrela::mir::OperationId,
) -> Option<wrela::mir::RegionId> {
    for (region_index, region) in module.regions().iter().enumerate() {
        for block in region.blocks() {
            if module.block(*block).operations().contains(&op_id) {
                return Some(wrela::mir::RegionId::new(region_index as u32));
            }
        }
    }
    None
}

#[test]
fn branch_select_cost_prefers_csel_outside_predictable_loops() {
    let generic = wrela::mir::TargetProfile::GenericAArch64.costs();
    assert!(generic.should_if_convert(false, false, 2));
    assert!(!generic.should_if_convert(true, true, 2));
    assert!(!generic.should_if_convert(false, false, 4));
}
