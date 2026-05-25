pub mod body;
pub mod cst;
pub mod effects;
pub mod json;
pub mod layout;
pub mod ownership;
pub mod report;
pub mod resolve;
pub mod suggest;
pub mod summary;
pub mod types;

use std::collections::BTreeMap;
use std::path::Path;

use crate::check::body::{BodyCheckContext, BodyCheckResult, check_bodies};
use crate::check::effects::{EffectCheck, EffectContext, check_effects};
use crate::check::layout::{LayoutCheck, check_layouts};
use crate::check::ownership::{OwnershipCheck, OwnershipContext, check_ownership};
use crate::check::report::{SemanticReport, build_semantic_report};
use crate::check::resolve::{ResolvedGraph, resolve_modules};
use crate::check::summary::{CheckModuleSummary, summarize_checked_module};
use crate::check::types::{SignatureCheck, check_signatures};
use crate::diagnostic::{Diagnostic, has_errors};
use crate::discover::discover_from_root;
use crate::lexer::LexedFile;
use crate::source::{SourceFile, SourceMap};
use crate::syntax::{ParsedSyntax, parse_files_parallel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckFormat {
    Json,
    Human,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleInput<'a> {
    pub summary: &'a CheckModuleSummary,
    pub parsed: &'a ParsedSyntax,
    pub lexed: &'a LexedFile,
    pub source: &'a SourceFile,
}

fn module_inputs<'a>(
    summaries: &'a [CheckModuleSummary],
    parsed: &'a [ParsedSyntax],
    lexed_files: &'a [LexedFile],
    source_map: &'a SourceMap,
) -> Vec<ModuleInput<'a>> {
    let parsed_by_file = parsed
        .iter()
        .map(|parsed| (parsed.file_id(), parsed))
        .collect::<BTreeMap<_, _>>();
    let lexed_by_file = lexed_files
        .iter()
        .map(|lexed| (lexed.file_id(), lexed))
        .collect::<BTreeMap<_, _>>();
    let mut modules = Vec::new();

    for summary in summaries {
        let Some(parsed) = parsed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(lexed) = lexed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(source) = source_map.get(summary.file_id()) else {
            continue;
        };
        modules.push(ModuleInput {
            summary,
            parsed,
            lexed,
            source,
        });
    }

    modules
}

#[derive(Debug)]
pub struct CheckResult {
    source_map: SourceMap,
    parsed: Vec<ParsedSyntax>,
    summaries: Vec<CheckModuleSummary>,
    resolved_graph: ResolvedGraph,
    signature_check: SignatureCheck,
    body_check: BodyCheckResult,
    ownership_check: OwnershipCheck,
    effect_check: EffectCheck,
    layout_check: LayoutCheck,
    semantic_report: SemanticReport,
    diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn parsed(&self) -> &[ParsedSyntax] {
        &self.parsed
    }

    pub fn summaries(&self) -> &[CheckModuleSummary] {
        &self.summaries
    }

    pub fn resolved_graph(&self) -> &ResolvedGraph {
        &self.resolved_graph
    }

    pub fn signature_check(&self) -> &SignatureCheck {
        &self.signature_check
    }

    pub fn body_check(&self) -> &BodyCheckResult {
        &self.body_check
    }

    pub fn ownership_check(&self) -> &OwnershipCheck {
        &self.ownership_check
    }

    pub fn effect_check(&self) -> &EffectCheck {
        &self.effect_check
    }

    pub fn layout_check(&self) -> &LayoutCheck {
        &self.layout_check
    }

    pub fn semantic_report(&self) -> &SemanticReport {
        &self.semantic_report
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

pub fn check_root(root: impl AsRef<Path>) -> CheckResult {
    let discovered = discover_from_root(root);
    let source_map = discovered.source_map().clone();
    let parsed = parse_files_parallel(discovered.lexed_files(), &source_map);

    let mut diagnostics = Vec::new();
    diagnostics.extend(discovered.diagnostics().iter().cloned());
    for parsed_file in &parsed {
        diagnostics.extend(parsed_file.diagnostics().iter().cloned());
    }

    let source_root = source_map
        .files()
        .first()
        .and_then(|file| file.path().parent())
        .unwrap_or_else(|| Path::new("."));
    let lexed_by_file: BTreeMap<_, _> = discovered
        .lexed_files()
        .iter()
        .map(|lexed| (lexed.file_id(), lexed))
        .collect();
    let mut summaries = Vec::new();
    for parsed_file in &parsed {
        if let (Some(lexed), Some(source)) = (
            lexed_by_file.get(&parsed_file.file_id()),
            source_map.get(parsed_file.file_id()),
        ) {
            let (summary, summary_diagnostics) =
                summarize_checked_module(parsed_file, lexed, source, source_root);
            diagnostics.extend(summary_diagnostics);
            summaries.push(summary);
        }
    }

    let resolved_graph = resolve_modules(&summaries);
    diagnostics.extend(resolved_graph.diagnostics().iter().cloned());

    let signature_check = check_signatures(&summaries, &resolved_graph);
    diagnostics.extend(signature_check.diagnostics().iter().cloned());

    let modules = module_inputs(&summaries, &parsed, discovered.lexed_files(), &source_map);

    let body_check = check_bodies(BodyCheckContext {
        modules: &modules,
        graph: &resolved_graph,
        signatures: &signature_check,
    });
    diagnostics.extend(body_check.diagnostics().iter().cloned());

    let ownership_check = check_ownership(OwnershipContext {
        modules: &modules,
        signatures: &signature_check,
    });
    diagnostics.extend(ownership_check.diagnostics().iter().cloned());

    let effect_check = check_effects(EffectContext { modules: &modules });
    diagnostics.extend(effect_check.diagnostics().iter().cloned());

    let layout_check = check_layouts(&summaries, &signature_check);
    diagnostics.extend(layout_check.diagnostics().iter().cloned());

    let semantic_report = build_semantic_report(
        &summaries,
        ownership_check.diagnostics().len(),
        effect_check.diagnostics().len(),
        layout_check.diagnostics().len(),
    );

    CheckResult {
        source_map,
        parsed,
        summaries,
        resolved_graph,
        signature_check,
        body_check,
        ownership_check,
        effect_check,
        layout_check,
        semantic_report,
        diagnostics,
    }
}
