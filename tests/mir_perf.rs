use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn perf_compile_json_reports_phase_and_counters() {
    let root = fixture("data_flow.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "compile".to_string(),
            "--mode".to_string(),
            "dev".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            "--json".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let json = String::from_utf8(out).unwrap();
    assert!(json.contains("\"schema\":\"wrela.perf.compile.v1\""));
    assert!(json.contains("\"mode\":\"dev\""));
    assert!(json.contains("\"releasePipeline\":\"dev\""));
    assert!(json.contains("\"mirBuild\""));
    assert!(json.contains("\"mirCounters\""));
}

#[test]
fn perf_code_json_reports_checksum_or_unsupported_host() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("perf")
        .join("scalar_const.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "code".to_string(),
            "--mode".to_string(),
            "dev".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert!(code == 0 || code == 2);
    if code == 0 {
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"schema\":\"wrela.perf.code.v1\""));
        assert!(json.contains("\"checksum\""));
        assert!(json.contains("\"runtime\""));
    } else {
        assert!(out.is_empty());
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("generated-code execution is not supported")
        );
    }
}

#[test]
fn perf_code_accepts_release_pass_controls_and_reports_rewrites() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("perf")
        .join("scalar_identity.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "code".to_string(),
            "--mode".to_string(),
            "release".to_string(),
            "--only-pass".to_string(),
            "scalar-peephole".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert!(code == 0 || code == 2);
    if code == 0 {
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"mode\":\"release\""));
        assert!(json.contains("\"passes\":[\"scalar-peephole\"]"));
        assert!(json.contains("\"rewriteCount\":1"));
        assert!(json.contains("\"rewriteEventCount\":1"));
        assert!(json.contains("\"eventsTruncated\":false"));
        assert!(json.contains("\"beforeHash\""));
        assert!(json.contains("\"afterHash\""));
    }
}

#[test]
fn measurement_classifies_inconclusive_noise() {
    let baseline = wrela::mir::SampleSet::new(vec![100, 101, 102, 140, 141, 142, 143]).unwrap();
    let candidate = wrela::mir::SampleSet::new(vec![99, 100, 101, 139, 140, 141, 142]).unwrap();
    let result = wrela::mir::classify_measurement(1, 1, &baseline, &candidate);

    assert_eq!(
        result.classification(),
        wrela::mir::MeasurementClassification::Inconclusive
    );
}

#[test]
fn measurement_classifies_inconclusive_for_small_delta() {
    let baseline = wrela::mir::SampleSet::new(vec![100, 101, 102, 103, 104, 105, 106]).unwrap();
    let candidate = wrela::mir::SampleSet::new(vec![99, 100, 101, 102, 103, 104, 105]).unwrap();
    let result = wrela::mir::classify_measurement(1, 1, &baseline, &candidate);

    assert_eq!(
        result.classification(),
        wrela::mir::MeasurementClassification::Inconclusive
    );
}

#[test]
fn measurement_classifies_stable_win() {
    let baseline = wrela::mir::SampleSet::new(vec![100, 101, 102, 103, 104, 105, 106]).unwrap();
    let candidate = wrela::mir::SampleSet::new(vec![90, 91, 92, 93, 94, 95, 96]).unwrap();
    let result = wrela::mir::classify_measurement(7, 7, &baseline, &candidate);

    assert_eq!(
        result.classification(),
        wrela::mir::MeasurementClassification::StableWin
    );
}

#[test]
fn perf_code_runs_filter_sum_dataplane_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "perf".into(),
            "code".into(),
            "--mode".into(),
            "release".into(),
            "--only-pass".into(),
            "mask-algebra".into(),
            "--repeat".into(),
            "7".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    if cfg!(target_arch = "aarch64") {
        assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&err));
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"checksum\""));
        assert!(json.contains("\"expectedChecksum\""));
        assert!(json.contains("\"checksumMatched\":true"));
        assert!(json.contains("\"sampleCount\":7"));
    } else {
        assert_eq!(code, 2);
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("unsupported generated-code execution host")
        );
    }
}

#[test]
fn perf_compare_records_ledger_for_filter_sum() {
    if !(cfg!(target_arch = "aarch64") && cfg!(target_os = "macos")) {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let report = wrela::mir::compare_code(root.to_str().unwrap(), 7).unwrap();
    assert!(report.baseline().checksum_matched());
    assert!(report.candidate().checksum_matched());
    assert!(report.ledger_recorded());
    let ledger_dir = std::path::PathBuf::from("target/wrela/ledger");
    assert!(ledger_dir.join("generic-aarch64.ledger").exists());
}

#[test]
fn perf_code_reports_dataplane_codegen_unsupported() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "perf".into(),
            "code".into(),
            "--mode".into(),
            "release".into(),
            "--only-pass".into(),
            "mask-algebra".into(),
            "--repeat".into(),
            "7".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    if cfg!(target_arch = "aarch64") {
        assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&err));
    } else {
        assert_eq!(code, 2);
        assert!(out.is_empty());
        let message = String::from_utf8(err).unwrap();
        assert!(message.contains("unsupported generated-code execution host"));
    }
}
