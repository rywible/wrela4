use std::fmt::Write;
use std::time::{Duration, Instant};

use crate::check::check_root;

use super::OptimizerKey;
use super::cost::TargetProfile;
use super::ledger::{LedgerEntry, OptimizationLedger};
use super::report::MirReport;
use super::{build_mir, emit, lower_to_lir, regalloc, verify_lir};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerfMode {
    Dev,
    Release,
}

#[derive(Clone, Debug)]
pub struct CompilePerfReport {
    mode: PerfMode,
    repeats: usize,
    mir_build_nanos: Vec<u128>,
    lower_nanos: Vec<u128>,
    emit_nanos: Vec<u128>,
    mir_report: MirReport,
}

pub fn measure_compile(
    path: &str,
    mode: PerfMode,
    repeats: usize,
) -> Result<CompilePerfReport, String> {
    let repeats = repeats.max(1);
    let mut mir_build_nanos = Vec::new();
    let mut lower_nanos = Vec::new();
    let mut emit_nanos = Vec::new();
    let mut mir_report = MirReport::default();

    for _ in 0..repeats {
        let check = check_root(path);
        if !check.ok() {
            return Err("check failed before compile benchmark".to_string());
        }

        let start = Instant::now();
        let mir = build_mir(&check);
        mir_build_nanos.push(elapsed_nanos(start.elapsed()));
        if !mir.ok() {
            return Err("MIR build failed before compile benchmark".to_string());
        }
        mir_report = mir.report().clone();

        let start = Instant::now();
        let lir = lower_to_lir(mir.module().expect("ok MIR has module"));
        let lir_check = verify_lir(&lir);
        if !lir_check.ok() {
            return Err(format!(
                "LIR verification failed: {:?}",
                lir_check.messages()
            ));
        }
        lower_nanos.push(elapsed_nanos(start.elapsed()));

        let start = Instant::now();
        let allocated =
            regalloc::allocate_registers(&lir).map_err(|err| err.message().to_string())?;
        let _asm = emit::emit_aarch64(&allocated);
        emit_nanos.push(elapsed_nanos(start.elapsed()));
    }

    Ok(CompilePerfReport {
        mode,
        repeats,
        mir_build_nanos,
        lower_nanos,
        emit_nanos,
        mir_report,
    })
}

pub fn render_compile_json(report: &CompilePerfReport) -> String {
    format!(
        "{{\"schema\":\"wrela.perf.compile.v1\",\"mode\":\"{}\",\"releasePipeline\":\"{}\",\"repeats\":{},\"mirBuild\":{},\"lower\":{},\"emit\":{},\"mirCounters\":{{\"regions\":{},\"blocks\":{},\"operations\":{},\"values\":{},\"places\":{}}}}}\n",
        mode_name(report.mode),
        release_pipeline_name(report.mode),
        report.repeats,
        median(&report.mir_build_nanos),
        median(&report.lower_nanos),
        median(&report.emit_nanos),
        report.mir_report.regions(),
        report.mir_report.blocks(),
        report.mir_report.operations(),
        report.mir_report.values(),
        report.mir_report.places(),
    )
}

pub fn render_compile_human(report: &CompilePerfReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "wrela perf compile ({})", mode_name(report.mode));
    let _ = writeln!(out, "repeats: {}", report.repeats);
    let _ = writeln!(
        out,
        "mir.build median ns: {}",
        median(&report.mir_build_nanos)
    );
    let _ = writeln!(out, "lower median ns: {}", median(&report.lower_nanos));
    let _ = writeln!(out, "emit median ns: {}", median(&report.emit_nanos));
    out
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleSet {
    samples: Vec<u128>,
}

impl SampleSet {
    pub fn new(mut samples: Vec<u128>) -> Result<Self, String> {
        if samples.is_empty() {
            return Err("sample set must not be empty".to_string());
        }
        samples.sort_unstable();
        Ok(Self { samples })
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn median(&self) -> u128 {
        self.samples[self.samples.len() / 2]
    }

    pub fn p10(&self) -> u128 {
        percentile(&self.samples, 10)
    }

    pub fn p90(&self) -> u128 {
        percentile(&self.samples, 90)
    }

    pub fn stable_noise(&self) -> bool {
        self.p10() > 0 && self.p90() * 100 <= self.p10() * 110
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementClassification {
    StableWin,
    StableLoss,
    Neutral,
    Inconclusive,
    ChecksumMismatch,
}

impl MeasurementClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            MeasurementClassification::StableWin => "stable-win",
            MeasurementClassification::StableLoss => "stable-loss",
            MeasurementClassification::Neutral => "neutral",
            MeasurementClassification::Inconclusive => "inconclusive",
            MeasurementClassification::ChecksumMismatch => "checksum-mismatch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementResult {
    classification: MeasurementClassification,
    median_delta_percent: i32,
}

impl MeasurementResult {
    pub fn classification(&self) -> MeasurementClassification {
        self.classification
    }

    pub fn median_delta_percent(&self) -> i32 {
        self.median_delta_percent
    }
}

pub fn classify_measurement(
    baseline_checksum: u64,
    candidate_checksum: u64,
    baseline: &SampleSet,
    candidate: &SampleSet,
) -> MeasurementResult {
    if baseline_checksum != candidate_checksum {
        return MeasurementResult {
            classification: MeasurementClassification::ChecksumMismatch,
            median_delta_percent: 0,
        };
    }
    if baseline.len() < 7
        || candidate.len() < 7
        || !baseline.stable_noise()
        || !candidate.stable_noise()
    {
        return MeasurementResult {
            classification: MeasurementClassification::Inconclusive,
            median_delta_percent: 0,
        };
    }
    let base = baseline.median() as i128;
    let cand = candidate.median() as i128;
    let delta = (((cand - base) * 100) / base) as i32;
    let classification = if delta <= -3 {
        MeasurementClassification::StableWin
    } else if delta >= 3 {
        MeasurementClassification::StableLoss
    } else {
        MeasurementClassification::Inconclusive
    };
    MeasurementResult {
        classification,
        median_delta_percent: delta,
    }
}

pub fn record_measurement_in_ledger(
    ledger: &mut OptimizationLedger,
    key: OptimizerKey,
    baseline_samples: &SampleSet,
    result: &MeasurementResult,
    code_size_delta_bytes: i32,
) -> Result<(), String> {
    let samples = baseline_samples.len() as u32;
    let entry = match result.classification() {
        MeasurementClassification::StableWin => LedgerEntry::known_winner(
            key,
            samples,
            result.median_delta_percent(),
            code_size_delta_bytes,
        ),
        MeasurementClassification::StableLoss => LedgerEntry::known_loser(
            key,
            samples,
            result.median_delta_percent(),
            code_size_delta_bytes,
        ),
        MeasurementClassification::Neutral | MeasurementClassification::Inconclusive => {
            LedgerEntry::needs_remeasure(
                key,
                samples,
                result.median_delta_percent(),
                code_size_delta_bytes,
            )
        }
        MeasurementClassification::ChecksumMismatch => {
            return Err(
                "checksum mismatch cannot be recorded as optimization evidence".to_string(),
            );
        }
    };
    ledger.record(entry)?;
    ledger.flush()
}

#[derive(Clone, Debug)]
pub struct CompareCodeReport {
    baseline: CodePerfReport,
    candidate: CodePerfReport,
    measurement: MeasurementResult,
    ledger_recorded: bool,
}

impl CompareCodeReport {
    pub fn baseline(&self) -> &CodePerfReport {
        &self.baseline
    }

    pub fn candidate(&self) -> &CodePerfReport {
        &self.candidate
    }

    pub fn measurement(&self) -> &MeasurementResult {
        &self.measurement
    }

    pub fn ledger_recorded(&self) -> bool {
        self.ledger_recorded
    }
}

pub fn compare_code(path: &str, repeats: usize) -> Result<CompareCodeReport, String> {
    let baseline_pass_set =
        crate::mir::PassSet::release_default().with_disabled(crate::mir::Pass::TableLoopFusion);
    let candidate_pass_set = crate::mir::PassSet::only(crate::mir::Pass::TableLoopFusion);
    let baseline = measure_code(path, PerfMode::Release, repeats, &baseline_pass_set)?;
    let candidate = measure_code(path, PerfMode::Release, repeats, &candidate_pass_set)?;
    let baseline_samples = SampleSet::new(baseline.runtime_nanos.clone())?;
    let candidate_samples = SampleSet::new(candidate.runtime_nanos.clone())?;
    let measurement = classify_measurement(
        baseline.checksum,
        candidate.checksum,
        &baseline_samples,
        &candidate_samples,
    );
    let code_size_delta = candidate.emitted_bytes as i32 - baseline.emitted_bytes as i32;
    let mut ledger_recorded = false;
    if baseline.checksum_matched
        && candidate.checksum_matched
        && measurement.classification() != MeasurementClassification::ChecksumMismatch
    {
        let region_hash = parse_report_hash(&candidate.before_hash)?;
        let key = OptimizerKey::new(
            region_hash,
            TargetProfile::GenericAArch64,
            &candidate_pass_set,
            env!("CARGO_PKG_VERSION"),
            crate::mir::CERTIFICATE_VERSION,
        );
        let ledger_dir = std::path::PathBuf::from("target")
            .join("wrela")
            .join("ledger");
        if let Ok(mut ledger) = OptimizationLedger::open(ledger_dir, TargetProfile::GenericAArch64)
        {
            if record_measurement_in_ledger(
                &mut ledger,
                key,
                &baseline_samples,
                &measurement,
                code_size_delta,
            )
            .is_ok()
            {
                ledger_recorded = true;
            }
        }
    }
    let mut baseline = baseline;
    let mut candidate = candidate;
    if measurement.classification() == MeasurementClassification::Inconclusive {
        baseline.trident_failure = Some("measurement-inconclusive".to_string());
        candidate.trident_failure = Some("measurement-inconclusive".to_string());
    }
    Ok(CompareCodeReport {
        baseline,
        candidate,
        measurement,
        ledger_recorded,
    })
}

fn parse_report_hash(text: &str) -> Result<crate::mir::StableHash, String> {
    u64::from_str_radix(text, 16)
        .map(crate::mir::StableHash::new)
        .map_err(|_| format!("invalid report hash `{text}`"))
}

pub fn render_compare_json(report: &CompareCodeReport) -> String {
    format!(
        "{{\"schema\":\"wrela.perf.compare.v1\",\"baseline\":{},\"candidate\":{},\"classification\":\"{}\",\"medianDeltaPercent\":{},\"ledgerRecorded\":{}}}\n",
        render_code_json_inner(&report.baseline),
        render_code_json_inner(&report.candidate),
        report.measurement.classification().as_str(),
        report.measurement.median_delta_percent(),
        report.ledger_recorded,
    )
}

fn render_code_json_inner(report: &CodePerfReport) -> String {
    let rendered = render_code_json(report);
    rendered.trim_end().to_string()
}

#[derive(Clone, Debug)]
pub struct CodePerfReport {
    mode: PerfMode,
    repeats: usize,
    runtime_nanos: Vec<u128>,
    checksum: u64,
    expected_checksum: u64,
    checksum_matched: bool,
    sample_count: usize,
    measurement: Option<MeasurementClassification>,
    emitted_bytes: usize,
    lir_instructions: usize,
    passes: Vec<&'static str>,
    rewrite_count: usize,
    rewrite_event_count: usize,
    events_truncated: bool,
    before_hash: String,
    after_hash: String,
    trident_failure: Option<String>,
}

impl CodePerfReport {
    pub fn checksum_matched(&self) -> bool {
        self.checksum_matched
    }
}

pub fn measure_code(
    path: &str,
    mode: PerfMode,
    repeats: usize,
    pass_set: &crate::mir::PassSet,
) -> Result<CodePerfReport, String> {
    if !(std::env::consts::ARCH == "aarch64" && std::env::consts::OS == "macos") {
        return Err("generated-code execution is not supported on this host".to_string());
    }

    let check = check_root(path);
    if !check.ok() {
        return Err("check failed before generated-code benchmark".to_string());
    }
    let mir = build_mir(&check);
    if !mir.ok() {
        return Err("MIR build failed before generated-code benchmark".to_string());
    }

    let optimized_release = match mode {
        PerfMode::Dev => None,
        PerfMode::Release => Some(crate::mir::optimize_release(
            mir.module().expect("ok MIR has module"),
            pass_set,
        )),
    };
    let (module_for_codegen, release_report) = match &optimized_release {
        None => (mir.module().expect("ok MIR has module"), None),
        Some(optimized) => (optimized.module(), Some(optimized.report().clone())),
    };

    let dataplane = contains_dataplane_codegen_required(module_for_codegen);
    let filter_sum = dataplane && path.ends_with("filter_sum.wrela");

    let lir = lower_to_lir(module_for_codegen);
    let lir_check = verify_lir(&lir);
    if !lir_check.ok() {
        return Err(format!(
            "LIR verification failed: {:?}",
            lir_check.messages()
        ));
    }
    let allocated = regalloc::allocate_registers(&lir).map_err(|err| err.message().to_string())?;
    let asm = emit::emit_aarch64(&allocated);
    let function = lir
        .functions()
        .first()
        .ok_or_else(|| "generated-code benchmark has no function".to_string())?;
    if !filter_sum && !function.params().is_empty() {
        return Err(
            "generated-code execution is not supported for parameterized benchmarks in MIR 02"
                .to_string(),
        );
    }
    let symbol = emit::assembly_symbol(function.symbol());

    let temp_dir = std::env::temp_dir().join(format!(
        "wrela-perf-code-{}-{}",
        std::process::id(),
        unique_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
    let asm_path = temp_dir.join("bench.s");
    let harness_path = temp_dir.join("harness.c");
    let exe_path = temp_dir.join("bench");
    std::fs::write(&asm_path, asm.as_bytes()).map_err(|err| err.to_string())?;
    let harness = if filter_sum {
        filter_sum_harness_source(&symbol)
    } else {
        harness_source(&symbol)
    };
    std::fs::write(&harness_path, harness.as_bytes()).map_err(|err| err.to_string())?;

    let compile = std::process::Command::new("cc")
        .arg("-O2")
        .arg(&harness_path)
        .arg(&asm_path)
        .arg("-o")
        .arg(&exe_path)
        .output()
        .map_err(|err| format!("generated-code execution is not supported: {err}"))?;
    if !compile.status.success() {
        return Err("generated-code execution is not supported: cc failed".to_string());
    }

    let repeats = repeats.max(1);
    let mut runtime_nanos = Vec::new();
    let mut checksum = 0u64;
    let mut expected_checksum = 0u64;
    for _ in 0..repeats {
        let start = Instant::now();
        let output = std::process::Command::new(&exe_path)
            .output()
            .map_err(|err| format!("generated-code execution is not supported: {err}"))?;
        if !output.status.success() {
            return Err("generated-code benchmark process failed".to_string());
        }
        if filter_sum {
            let (actual, expected) = parse_checksum_pair(&output.stdout)?;
            checksum = actual;
            expected_checksum = expected;
        } else {
            checksum = parse_checksum(&output.stdout)?;
            expected_checksum = checksum;
        }
        runtime_nanos.push(elapsed_nanos(start.elapsed()));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    let checksum_matched = checksum == expected_checksum;
    let mut trident_failure = if !checksum_matched {
        let _ = crate::mir::preserve_failure_artifact(
            "checksum-mismatch",
            "wmir.checksum-mismatch.v1",
            module_for_codegen,
        );
        Some("checksum-mismatch".to_string())
    } else {
        None
    };
    if trident_failure.is_none() {
        if let Some(report) = &release_report {
            trident_failure = report.trident_failure().map(|reason| reason.to_string());
        }
    }

    let rewrite_count = release_report
        .as_ref()
        .map(|report| report.rewrites_applied())
        .unwrap_or(0);
    let rewrite_event_count = release_report
        .as_ref()
        .map(|report| report.rewrite_events().len())
        .unwrap_or(0);
    let events_truncated = release_report
        .as_ref()
        .map(|report| report.events_truncated())
        .unwrap_or(false);
    let before_hash = release_report
        .as_ref()
        .map(|report| report.before_hash().to_string())
        .unwrap_or_else(|| "0000000000000000".to_string());
    let after_hash = release_report
        .as_ref()
        .map(|report| report.after_hash().to_string())
        .unwrap_or_else(|| "0000000000000000".to_string());

    Ok(CodePerfReport {
        mode,
        repeats,
        runtime_nanos: runtime_nanos.clone(),
        checksum,
        expected_checksum,
        checksum_matched,
        sample_count: runtime_nanos.len(),
        measurement: None,
        emitted_bytes: asm.len(),
        lir_instructions: lir.instructions().len(),
        passes: pass_set.names(),
        rewrite_count,
        rewrite_event_count,
        events_truncated,
        before_hash,
        after_hash,
        trident_failure,
    })
}

pub fn render_code_json(report: &CodePerfReport) -> String {
    let pass_json = report
        .passes
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"wrela.perf.code.v1\",\"mode\":\"{}\",\"repeats\":{},\"runtime\":{},\"checksum\":{},\"expectedChecksum\":{},\"checksumMatched\":{},\"sampleCount\":{},\"measurement\":{},\"emittedBytes\":{},\"lirInstructions\":{},\"passes\":[{}],\"rewriteCount\":{},\"rewriteEventCount\":{},\"eventsTruncated\":{},\"beforeHash\":\"{}\",\"afterHash\":\"{}\",\"tridentFailure\":{}}}\n",
        mode_name(report.mode),
        report.repeats,
        median(&report.runtime_nanos),
        report.checksum,
        report.expected_checksum,
        report.checksum_matched,
        report.sample_count,
        measurement_json(report.measurement),
        report.emitted_bytes,
        report.lir_instructions,
        pass_json,
        report.rewrite_count,
        report.rewrite_event_count,
        report.events_truncated,
        report.before_hash,
        report.after_hash,
        trident_failure_json(report.trident_failure.as_deref()),
    )
}

fn measurement_json(value: Option<MeasurementClassification>) -> String {
    match value {
        Some(classification) => format!("\"{}\"", classification.as_str()),
        None => "null".to_string(),
    }
}

fn trident_failure_json(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("\"{text}\""),
        None => "null".to_string(),
    }
}

fn filter_sum_harness_source(symbol: &str) -> String {
    let c_symbol = c_link_symbol(symbol);
    format!(
        "#include <stdint.h>\n#include <stdio.h>\ntypedef struct Packet {{\n  uint32_t len;\n  uint32_t flags;\n}} Packet;\nstatic uint8_t self_object[1];\nstatic Packet packets[256];\nstatic uint8_t valid_mask[32];\nstatic Packet small[128];\nstatic uint8_t small_valid_mask[16];\nstatic void init(void) {{\n  for (uint32_t i = 0; i < 256; i++) {{\n    packets[i].len = i + 1;\n    packets[i].flags = i ^ 0x55u;\n    if ((i % 3) != 0) valid_mask[i >> 3] |= (uint8_t)(1u << (i & 7));\n  }}\n  for (uint32_t i = 0; i < 128; i++) {{\n    small[i].len = i + 5;\n    small[i].flags = i ^ 0x33u;\n    if ((i % 4) != 1) small_valid_mask[i >> 3] |= (uint8_t)(1u << (i & 7));\n  }}\n}}\nstatic uint64_t expected_checksum(void) {{\n  uint64_t total = 0;\n  uint64_t flagged = 0;\n  uint64_t small_total = 0;\n  for (uint32_t i = 0; i < 256; i++) {{\n    if ((valid_mask[i >> 3] & (uint8_t)(1u << (i & 7))) != 0) {{\n      total += packets[i].len;\n      flagged += packets[i].flags;\n    }}\n  }}\n  for (uint32_t i = 0; i < 128; i++) {{\n    if ((small_valid_mask[i >> 3] & (uint8_t)(1u << (i & 7))) != 0) {{\n      small_total += small[i].len;\n    }}\n  }}\n  return total + flagged + small_total;\n}}\nextern uint64_t {c_symbol}(uint8_t *self, Packet *packets, uint8_t *valid, Packet *small, uint8_t *small_valid);\nint main(void) {{\n  init();\n  uint64_t expected = expected_checksum();\n  uint64_t checksum = {c_symbol}(self_object, packets, valid_mask, small, small_valid_mask);\n  printf(\"%llu %llu\\n\", (unsigned long long)checksum, (unsigned long long)expected);\n  return 0;\n}}\n",
        c_symbol = c_symbol,
    )
}

fn parse_checksum_pair(bytes: &[u8]) -> Result<(u64, u64), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "benchmark checksum was not utf-8")?;
    let mut parts = text.split_whitespace();
    let actual = parts
        .next()
        .ok_or_else(|| "missing benchmark checksum".to_string())?
        .parse::<u64>()
        .map_err(|_| "benchmark checksum was not an unsigned decimal integer".to_string())?;
    let expected = parts
        .next()
        .ok_or_else(|| "missing expected benchmark checksum".to_string())?
        .parse::<u64>()
        .map_err(|_| "expected checksum was not an unsigned decimal integer".to_string())?;
    Ok((actual, expected))
}

fn c_link_symbol(symbol: &str) -> &str {
    symbol.strip_prefix('_').unwrap_or(symbol)
}

fn harness_source(symbol: &str) -> String {
    let c_symbol = c_link_symbol(symbol);
    format!(
        "#include <stdint.h>\n#include <stdio.h>\nextern uint64_t {c_symbol}(void);\nint main(void) {{ uint64_t checksum = 0; for (uint64_t i = 0; i < 100000; i++) {{ checksum += {c_symbol}(); }} printf(\"%llu\\n\", (unsigned long long)checksum); return 0; }}\n",
        c_symbol = c_symbol,
    )
}

fn percentile(samples: &[u128], p: u32) -> u128 {
    let len = samples.len();
    if len == 0 {
        return 0;
    }
    if len == 1 {
        return samples[0];
    }
    let index = ((len - 1) as u128 * p as u128 / 100) as usize;
    samples[index]
}

fn parse_checksum(bytes: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "benchmark checksum was not utf-8")?;
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("benchmark checksum was not an unsigned decimal integer".to_string());
    }
    trimmed
        .parse::<u64>()
        .map_err(|_| "benchmark checksum overflowed u64".to_string())
}

fn unique_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn mode_name(mode: PerfMode) -> &'static str {
    match mode {
        PerfMode::Dev => "dev",
        PerfMode::Release => "release",
    }
}

fn release_pipeline_name(mode: PerfMode) -> &'static str {
    mode_name(mode)
}

fn elapsed_nanos(duration: Duration) -> u128 {
    duration.as_nanos()
}

fn median(values: &[u128]) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

fn contains_dataplane_codegen_required(module: &crate::mir::MirModule) -> bool {
    module.operations().iter().any(|op| {
        matches!(
            op.kind(),
            crate::mir::OperationKind::ReduceRows { .. }
                | crate::mir::OperationKind::RowToken { .. }
                | crate::mir::OperationKind::TableRows { .. }
                | crate::mir::OperationKind::MaskRead { .. }
                | crate::mir::OperationKind::MaskAllTrue { .. }
                | crate::mir::OperationKind::MaskAllFalse { .. }
                | crate::mir::OperationKind::MaskAnd
                | crate::mir::OperationKind::MaskOr
                | crate::mir::OperationKind::MaskNot
        )
    })
}
