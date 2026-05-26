use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn mir_build_requires_successful_check() {
    let dir = std::env::temp_dir().join("wrela-mir-build-failed-check");
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.join("root.wrela");
    std::fs::write(&root, "module root\npub data Broken { field: }\n").unwrap();

    let check = wrela::check::check_root(&root);
    assert!(!check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(!mir.ok());
    assert!(mir.module().is_none());
    assert_eq!(mir.diagnostics().len(), check.diagnostics().len());
}

#[test]
fn mir_build_creates_lambda_and_omega_regions() {
    let check = wrela::check::check_root(fixture("basic.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let module = mir.module().expect("valid MIR module");

    assert_eq!(mir.report().regions(), 3);
    let region_symbols = module
        .regions()
        .iter()
        .filter_map(|region| match region.kind() {
            wrela::mir::RegionKind::Lambda { symbol }
            | wrela::mir::RegionKind::Omega { symbol } => Some(symbol.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(region_symbols.contains(&"app.main.Worker.run"));
    assert!(region_symbols.contains(&"app.main.Main"));
    assert!(region_symbols.contains(&"app.main.Main.run"));
}

#[test]
fn mir_build_lowers_checked_let_binary_and_return() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());

    assert!(text.contains("read_value left"));
    assert!(text.contains("read_value right"));
    assert!(text.contains("binary +"));
    assert!(text.contains("let sum"));
    assert!(text.contains("return"));
}

#[test]
fn mir_build_output_verifies() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let verification = wrela::mir::verify_module(mir.module().unwrap());
    assert!(verification.ok(), "{:?}", verification.messages());
}

#[test]
fn mir_text_round_trips_for_fixture() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());
    let parsed = wrela::mir::parse_text::parse_module(&text).unwrap();

    assert_eq!(wrela::mir::text::render_module(&parsed), text);
    assert!(wrela::mir::modules_semantically_equal(
        mir.module().unwrap(),
        &parsed
    ));
}

#[test]
fn mir_report_counts_fixture_shape() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    let report = mir.report();

    assert!(report.regions() >= 1);
    assert!(report.blocks() >= 1);
    assert!(report.operations() >= 4);
    assert!(report.values() >= 3);
}

#[test]
fn dump_mir_command_prints_deterministic_text() {
    let root = fixture("data_flow.wrela");
    let args = vec![
        "wrela".to_string(),
        "dump".to_string(),
        "mir".to_string(),
        root.to_string_lossy().to_string(),
    ];
    let mut out1 = Vec::new();
    let mut err1 = Vec::new();
    let mut out2 = Vec::new();
    let mut err2 = Vec::new();

    let code1 = wrela::command::run_with_io(args.clone(), &mut out1, &mut err1);
    let code2 = wrela::command::run_with_io(args, &mut out2, &mut err2);

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert!(err1.is_empty());
    assert!(err2.is_empty());
    assert_eq!(out1, out2);
    assert!(String::from_utf8(out1).unwrap().contains("wmir v1"));
}

#[test]
fn mir_builds_reachable_import_graph_in_stable_order() {
    let check = wrela::check::check_root(fixture("imports/root.wrela"));
    assert!(check.ok());

    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());
    let text = wrela::mir::text::render_module(mir.module().unwrap());

    let app_index = text.find("lambda app.root.App.run").unwrap();
    let helper_index = text.find("lambda lib.Helper.id").unwrap();
    assert!(app_index < helper_index);
}
