use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("wrela-{name}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        if parent != dir {
            fs::create_dir_all(parent).unwrap();
        }
    }
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn check_default_json_is_machine_readable() {
    let dir = temp_dir("check-json");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"schema\":\"wrela.check.v1\""));
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"diagnostics\":[]"));
    assert!(json.contains("\"sourceFiles\""));
    assert!(json.contains("\"semanticReport\""));
}

#[test]
fn check_human_renders_parse_diagnostic_with_source() {
    let dir = temp_dir("check-human");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Broken { field: }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            "--human".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(err.is_empty());
    let rendered = String::from_utf8(out).unwrap();
    assert!(rendered.contains("error[W-PARSE-EXPECTED-TYPE]"));
    assert!(rendered.contains("-->"));
    assert!(rendered.contains("root.wrela"));
    assert!(rendered.contains("field:"));
}

#[test]
fn check_collects_semantic_summaries() {
    let dir = temp_dir("check-summary");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module app.root\nuse { Console } from io\npub unique class Driver { uart: Console fn write(read self, read bytes: U32) -> U32 { } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok());
    let summaries = result.summaries();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].module_path().as_dotted(), "app.root");
    assert_eq!(summaries[0].imports()[0].binders()[0].text(), "Console");
    assert_eq!(summaries[0].items()[0].name().text(), "Driver");
    assert_eq!(summaries[0].items()[0].members().len(), 2);
}

#[test]
fn check_reports_duplicate_top_level_names() {
    let dir = temp_dir("check-duplicates");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub data Bytes { value: U32 }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-RESOLVE-DUPLICATE]"));
    assert!(rendered.contains("first declaration is here"));
}

#[test]
fn check_validates_imported_public_names() {
    let dir = temp_dir("check-imports");
    write_file(
        &dir,
        "io.wrela",
        "module io\nclass Hidden {}\npub unique class Console {}\n",
    );
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console, Hidden, Missing } from io\npub data Bytes { value: U32 }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-RESOLVE-PRIVATE"));
    assert!(rendered.contains("W-RESOLVE-IMPORT"));
    assert!(!rendered.contains("Console is missing"));
}

#[test]
fn check_json_includes_resolve_code_and_source_hash() {
    let dir = temp_dir("check-json-resolve");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub data Bytes { value: U32 }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"code\":\"W-RESOLVE-DUPLICATE\""));
    assert!(json.contains("\"sourceHash\""));
    assert!(json.contains("\"schema\":\"wrela.check.v1\""));
}

#[test]
fn check_rejects_conflicting_format_flags() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            "--json".to_string(),
            "--human".to_string(),
            "root.wrela".to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(
        String::from_utf8(err)
            .unwrap()
            .contains("malformed command")
    );
}

#[test]
fn check_reports_unknown_type_with_suggestion() {
    let dir = temp_dir("check-type-suggestion");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console } from io\npub data Port { value: Consol }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-UNKNOWN]"));
    assert!(rendered.contains("did you mean `Console`?"));
    assert!(rendered.contains("suggestion (likely): replace with `Console`"));
}

#[test]
fn check_reports_wrong_kind_type_reference() {
    let dir = temp_dir("check-wrong-kind");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub image Boot target Mac { phase start(value: Boot) { return } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-RESOLVE-KIND"));
}

#[test]
fn check_reports_return_type_mismatch() {
    let dir = temp_dir("check-return-type");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 { return \"no\" } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-RETURN]"));
    assert!(rendered.contains("expected `U32`"));
    assert!(rendered.contains("found `String`"));
}

#[test]
fn check_reports_unsupported_call_without_passing_silently() {
    let dir = temp_dir("check-unsupported-call");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 { return make() } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-CHECK-UNSUPPORTED]"));
    assert!(rendered.contains("call expressions are not checked yet"));
}

#[test]
fn check_reports_let_annotation_mismatch() {
    let dir = temp_dir("check-let-annotation");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 {\n  let value: U32 = \"no\"\n  return value\n} }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-TYPE-MISMATCH]"));
    assert!(rendered.contains("let annotation expects `U32`"));
}

#[test]
fn check_suppresses_type_cascade_after_unknown_name() {
    let dir = temp_dir("check-cascade");
    let root = write_file(
        &dir,
        "root.wrela",
        "module cascade\npub class C { fn m(read self) -> U32 { return missing + missing } }\n",
    );

    let result = wrela::check::check_root(&root);
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    let unknown_name_count = rendered.matches("W-RESOLVE-NAME").count();
    let mismatch_count = rendered.matches("W-TYPE-MISMATCH").count();

    assert_eq!(unknown_name_count, 1);
    assert_eq!(mismatch_count, 0);
    assert!(rendered.contains("related:"));
}

#[test]
fn check_default_json_includes_fix_metadata() {
    let dir = temp_dir("check-json-fix");
    write_file(&dir, "io.wrela", "module io\npub unique class Console {}\n");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { Console } from io\npub data Port { value: Consol }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"suggestions\""));
    assert!(json.contains("\"applicability\":\"likely\""));
    assert!(json.contains("\"replacement\":\"Console\""));
    assert!(json.contains("\"sourceHash\""));
}

#[test]
fn check_reports_use_after_move() {
    let dir = temp_dir("check-use-after-move");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(own bytes: Bytes) -> Bytes {\n  let saved = bytes\n  return bytes\n} }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-OWN-MOVE]"));
    assert!(rendered.contains("value was moved here"));
}

#[test]
fn check_reports_moving_read_parameter() {
    let dir = temp_dir("check-read-move");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(read bytes: Bytes) -> Bytes { return bytes } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-OWN-ACCESS]"));
    assert!(rendered.contains("`read` value cannot be moved"));
}

#[test]
fn check_reports_unknown_effect_in_phase() {
    let dir = temp_dir("check-phase-effect");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub host image Mac {}\npub image Boot target Mac { phase start() { loop { return } } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-EFFECT-UNSUPPORTED]"));
    assert!(rendered.contains("phase bodies cannot carry unknown effects"));
}

#[test]
fn check_accepts_primitive_layout_c_data() {
    let dir = temp_dir("check-layout-ok");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub layout C data Packet { value: U32 flag: Bool }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}

#[test]
fn check_rejects_string_in_layout_c_data() {
    let dir = temp_dir("check-layout-string");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub layout C data Packet { name: String }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("error[W-LAYOUT-INVALID]"));
    assert!(rendered.contains("layout C fields must be fixed primitive scalars"));
}

#[test]
fn diagnostic_examples_have_rich_human_output() {
    for fixture in [
        "fixtures/check/diagnostics/unknown-type.wrela",
        "fixtures/check/diagnostics/use-after-move.wrela",
        "fixtures/check/diagnostics/layout-invalid.wrela",
    ] {
        let result = wrela::check::check_root(fixture);
        assert!(!result.ok(), "{fixture} should be invalid");
        let rendered =
            wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
        assert!(rendered.contains("error["), "{fixture} missing code");
        assert!(
            rendered.contains("-->"),
            "{fixture} missing source reference"
        );
        assert!(rendered.contains("^"), "{fixture} missing underline");
    }
}

#[test]
fn semantic_report_counts_checked_facts() {
    let dir = temp_dir("check-report");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(read self) -> None { return } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
    assert_eq!(result.semantic_report().checked_modules(), 1);
    assert_eq!(result.semantic_report().checked_items(), 2);
    assert_eq!(result.semantic_report().checked_bodies(), 1);
}

#[test]
fn check_survives_duplicate_module_paths_without_panic() {
    let dir = temp_dir("check-dup-module");
    write_file(
        &dir,
        "app/console.wrela",
        "module io\npub data A { value: U32 }\n",
    );
    write_file(
        &dir,
        "lib/console.wrela",
        "module io\npub data B { value: U32 }\n",
    );
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\nuse { A } from app.console\nuse { B } from lib.console\npub data Root { value: U32 }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-RESOLVE-DUPLICATE"));
}

#[test]
fn check_accepts_return_of_same_item_type_parameter() {
    let dir = temp_dir("check-item-type-return");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\npub class C { fn m(own bytes: Bytes) -> Bytes { return bytes } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}

#[test]
fn check_accepts_integer_literal_for_u32_annotation() {
    let dir = temp_dir("check-u32-literal");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn m(read self) -> U32 { return 1 } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}

#[test]
fn check_reports_prefix_expression_as_unsupported() {
    let dir = temp_dir("check-prefix");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub class C { fn id(read self) -> U32 { return -1 } }\n",
    );

    let result = wrela::check::check_root(&root);
    assert!(!result.ok());
    let rendered = wrela::diagnostic::render_diagnostics(result.diagnostics(), result.source_map());
    assert!(rendered.contains("W-CHECK-UNSUPPORTED"));
    assert!(rendered.contains("prefix expressions are not checked yet"));
}

#[test]
fn check_rejects_unknown_flags() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            "--bogus".to_string(),
            "root.wrela".to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(
        String::from_utf8(err)
            .unwrap()
            .contains("malformed command")
    );
}

#[test]
fn check_rejects_format_flag_after_root_path() {
    let dir = temp_dir("check-trailing-flag");
    let root = write_file(
        &dir,
        "root.wrela",
        "module root\npub data Bytes { value: U32 }\n",
    );
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "check".to_string(),
            root.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(
        String::from_utf8(err)
            .unwrap()
            .contains("malformed command")
    );
}

#[test]
fn check_accepts_filter_sum_dataplane_subset() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join("dataplane")
        .join("filter_sum.wrela");

    let result = wrela::check::check_root(root);
    assert!(result.ok(), "{:?}", result.diagnostics());
}
