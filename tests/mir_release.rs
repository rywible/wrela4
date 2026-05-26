use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn release_optimizer_reports_lifted_pure_operations_without_rewrites() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    let passes = wrela::mir::PassSet::release_default();

    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);

    assert!(optimized.report().regions_visited() >= 1);
    assert!(optimized.report().pure_ops_lifted() >= 1);
    assert_eq!(optimized.report().rewrite_events().len(), 0);
    assert_eq!(
        wrela::mir::text::render_module(optimized.module()),
        wrela::mir::text::render_module(mir.module().unwrap())
    );
}

#[test]
fn release_optimizer_applies_scalar_identity_with_certificate() {
    let check = wrela::check::check_root(fixture("scalar_identity.wrela"));
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    let passes = wrela::mir::PassSet::only(wrela::mir::Pass::ScalarPeephole);

    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);
    let before = wrela::mir::text::render_module(mir.module().unwrap());
    let after = wrela::mir::text::render_module(optimized.module());

    assert!(before.contains("binary +"));
    assert!(!after.contains("binary +"));
    assert_eq!(optimized.report().rewrites_applied(), 1);
    assert_eq!(optimized.report().rewrite_events().len(), 1);

    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "scalar-add-zero");
    assert_eq!(cert.pass().name(), "scalar-peephole");
    assert!(!cert.facts().is_empty());
    assert_ne!(cert.before_hash(), cert.after_hash());
    assert!(wrela::mir::verify_module(optimized.module()).ok());
}
