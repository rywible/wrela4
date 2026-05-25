use crate::check::CheckResult;
use crate::diagnostic::{
    Applicability, Diagnostic, DiagnosticGroupRole, DiagnosticLabel, RelatedLocation, Severity,
    SourceEdit, source_edits_are_sorted_and_disjoint,
};
use crate::source::{SourceFile, SourceMap, Span};

pub fn render_check_json(result: &CheckResult) -> String {
    let mut out = String::new();
    out.push('{');
    field_str(&mut out, "schema", "wrela.check.v1");
    comma(&mut out);
    field_bool(&mut out, "ok", result.ok());
    comma(&mut out);
    field_usize(
        &mut out,
        "checkedFileCount",
        result.source_map().files().len(),
    );
    comma(&mut out);
    out.push_str("\"sourceFiles\":");
    render_source_files(&mut out, result.source_map());
    comma(&mut out);
    out.push_str("\"semanticReport\":");
    render_semantic_report(&mut out, result.semantic_report());
    comma(&mut out);
    out.push_str("\"diagnostics\":");
    render_diagnostics(&mut out, result.diagnostics(), result.source_map());
    out.push('}');
    out
}

fn render_semantic_report(out: &mut String, report: &crate::check::report::SemanticReport) {
    out.push('{');
    field_usize(out, "checkedModules", report.checked_modules());
    comma(out);
    field_usize(out, "checkedItems", report.checked_items());
    comma(out);
    field_usize(out, "checkedBodies", report.checked_bodies());
    comma(out);
    field_usize(out, "ownershipDiagnostics", report.ownership_diagnostics());
    comma(out);
    field_usize(out, "effectDiagnostics", report.effect_diagnostics());
    comma(out);
    field_usize(out, "layoutDiagnostics", report.layout_diagnostics());
    out.push('}');
}

fn render_source_files(out: &mut String, source_map: &SourceMap) {
    out.push('[');
    for (index, file) in source_map.files().iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_source_file(out, file);
    }
    out.push(']');
}

fn render_source_file(out: &mut String, file: &SourceFile) {
    out.push('{');
    field_u32(out, "id", file.id().raw());
    comma(out);
    let path = file.path().to_string_lossy().replace('\\', "/");
    field_str(out, "path", &path);
    comma(out);
    field_str(out, "hash", &file.source_hash().to_hex());
    out.push('}');
}

fn render_diagnostics(out: &mut String, diagnostics: &[Diagnostic], source_map: &SourceMap) {
    out.push('[');
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_diagnostic(out, diagnostic, source_map);
    }
    out.push(']');
}

fn render_diagnostic(out: &mut String, diagnostic: &Diagnostic, source_map: &SourceMap) {
    out.push('{');
    field_str(out, "severity", severity_text(diagnostic.severity()));
    comma(out);
    field_str(
        out,
        "phase",
        &format!("{:?}", diagnostic.phase()).to_ascii_lowercase(),
    );
    comma(out);
    out.push_str("\"code\":");
    match diagnostic.code_string() {
        Some(code) => json_string(out, code),
        None => out.push_str("null"),
    }
    comma(out);
    field_str(out, "message", diagnostic.message());
    comma(out);
    out.push_str("\"primary\":");
    match diagnostic.primary() {
        Some(label) => render_label(out, label.span(), label.message(), source_map),
        None => out.push_str("null"),
    }
    comma(out);
    out.push_str("\"secondary\":");
    render_labels(out, diagnostic.secondary(), source_map);
    comma(out);
    out.push_str("\"related\":");
    render_related(out, diagnostic.related(), source_map);
    comma(out);
    out.push_str("\"notes\":");
    render_string_array(out, diagnostic.notes());
    comma(out);
    out.push_str("\"help\":");
    render_string_array(out, diagnostic.help());
    comma(out);
    out.push_str("\"suggestions\":");
    render_suggestions(out, diagnostic, source_map);
    comma(out);
    out.push_str("\"group\":");
    render_group(out, diagnostic);
    out.push('}');
}

fn render_group(out: &mut String, diagnostic: &Diagnostic) {
    match diagnostic.group() {
        Some(group) => {
            out.push('{');
            field_u32(out, "id", group.id().raw());
            comma(out);
            field_str(out, "role", group_role_text(group.role()));
            out.push('}');
        }
        None => out.push_str("null"),
    }
}

fn render_string_array(out: &mut String, values: &[String]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        json_string(out, value);
    }
    out.push(']');
}

fn render_suggestions(out: &mut String, diagnostic: &Diagnostic, source_map: &SourceMap) {
    out.push('[');
    let mut first = true;
    for suggestion in diagnostic.suggestions() {
        if !source_edits_are_sorted_and_disjoint(suggestion.edits()) {
            continue;
        }
        if !first {
            comma(out);
        }
        first = false;
        out.push('{');
        field_str(out, "title", suggestion.title());
        comma(out);
        field_str(
            out,
            "applicability",
            applicability_text(suggestion.applicability()),
        );
        comma(out);
        out.push_str("\"edits\":");
        render_edits(out, suggestion.edits(), source_map);
        out.push('}');
    }
    out.push(']');
}

fn render_edits(out: &mut String, edits: &[SourceEdit], source_map: &SourceMap) {
    out.push('[');
    for (index, edit) in edits.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        out.push('{');
        out.push_str("\"span\":");
        render_span(out, edit.span(), source_map);
        comma(out);
        field_str(out, "replacement", edit.replacement());
        out.push('}');
    }
    out.push(']');
}

fn applicability_text(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::Certain => "certain",
        Applicability::Likely => "likely",
        Applicability::Maybe => "maybe",
    }
}

fn render_labels(out: &mut String, labels: &[DiagnosticLabel], source_map: &SourceMap) {
    out.push('[');
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_label(out, label.span(), label.message(), source_map);
    }
    out.push(']');
}

fn render_related(out: &mut String, related: &[RelatedLocation], source_map: &SourceMap) {
    out.push('[');
    for (index, location) in related.iter().enumerate() {
        if index > 0 {
            comma(out);
        }
        render_label(out, location.span(), location.message(), source_map);
    }
    out.push(']');
}

fn render_label(out: &mut String, span: Span, message: &str, source_map: &SourceMap) {
    out.push('{');
    field_str(out, "message", message);
    comma(out);
    out.push_str("\"span\":");
    render_span(out, span, source_map);
    out.push('}');
}

fn render_span(out: &mut String, span: Span, source_map: &SourceMap) {
    out.push('{');
    field_u32(out, "fileId", span.file_id().raw());
    comma(out);
    field_u32(out, "start", span.start());
    comma(out);
    field_u32(out, "end", span.end());
    if let Some(file) = source_map.get(span.file_id()) {
        let start = file.line_col(span.start());
        let end = file.line_col(span.end());
        comma(out);
        field_u32(out, "startLine", start.line());
        comma(out);
        field_u32(out, "startColumn", start.column());
        comma(out);
        field_u32(out, "endLine", end.line());
        comma(out);
        field_u32(out, "endColumn", end.column());
        comma(out);
        field_str(out, "sourceHash", &file.source_hash().to_hex());
    }
    out.push('}');
}

fn severity_text(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Hint => "hint",
    }
}

fn group_role_text(role: DiagnosticGroupRole) -> &'static str {
    match role {
        DiagnosticGroupRole::RootCause => "rootCause",
        DiagnosticGroupRole::Cascade => "cascade",
    }
}

fn field_str(out: &mut String, key: &str, value: &str) {
    json_string(out, key);
    out.push(':');
    json_string(out, value);
}

fn field_bool(out: &mut String, key: &str, value: bool) {
    json_string(out, key);
    out.push(':');
    out.push_str(if value { "true" } else { "false" });
}

fn field_usize(out: &mut String, key: &str, value: usize) {
    json_string(out, key);
    out.push(':');
    out.push_str(&value.to_string());
}

fn field_u32(out: &mut String, key: &str, value: u32) {
    json_string(out, key);
    out.push(':');
    out.push_str(&value.to_string());
}

fn comma(out: &mut String) {
    out.push(',');
}

fn json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}
