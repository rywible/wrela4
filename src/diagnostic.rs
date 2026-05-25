use crate::source::{SourceMap, Span};
use crate::syntax::SyntaxErrorKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPhase {
    Lex,
    Parse,
    Summary,
    Resolve,
    Type,
    Ownership,
    Effect,
    Layout,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    ParseExpectedItem,
    ParseExpectedIdentifier,
    ParseExpectedImportBinderList,
    ParseExpectedFrom,
    ParseExpectedModulePath,
    ParseInvalidImportBinder,
    ParseExpectedType,
    ParseExpectedExpression,
    ParseExpectedReturnAfterElse,
    ParseExpectedMatchArm,
    ParseExpectedAssertKind,
    ParseExpectedToken,
    ParseUnexpectedToken,
    ParseMissingCloseDelimiter,
    ParseExpressionTooDeep,
    SummaryInvalidModule,
    ResolveDuplicate,
    ResolveImport,
    ResolvePrivate,
    ResolveUnknownName,
    ResolveWrongKind,
    TypeUnknownType,
    TypeMismatch,
    TypeReturn,
    TypeCall,
    TypeArgument,
    OwnershipMove,
    OwnershipAccess,
    EffectUnsupported,
    LayoutInvalid,
    CheckUnsupported,
    CheckIo,
}

impl DiagnosticCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ParseExpectedItem => "W-PARSE-ITEM",
            Self::ParseExpectedIdentifier => "W-PARSE-IDENT",
            Self::ParseExpectedImportBinderList => "W-PARSE-IMPORT-BINDERS",
            Self::ParseExpectedFrom => "W-PARSE-FROM",
            Self::ParseExpectedModulePath => "W-PARSE-MODULE-PATH",
            Self::ParseInvalidImportBinder => "W-PARSE-IMPORT-BINDER",
            Self::ParseExpectedType => "W-PARSE-EXPECTED-TYPE",
            Self::ParseExpectedExpression => "W-PARSE-EXPR",
            Self::ParseExpectedReturnAfterElse => "W-PARSE-RETURN-ELSE",
            Self::ParseExpectedMatchArm => "W-PARSE-MATCH-ARM",
            Self::ParseExpectedAssertKind => "W-PARSE-ASSERT-KIND",
            Self::ParseExpectedToken => "W-PARSE-TOKEN",
            Self::ParseUnexpectedToken => "W-PARSE-UNEXPECTED",
            Self::ParseMissingCloseDelimiter => "W-PARSE-DELIM",
            Self::ParseExpressionTooDeep => "W-PARSE-DEPTH",
            Self::SummaryInvalidModule => "W-SUMMARY-INVALID",
            Self::ResolveDuplicate => "W-RESOLVE-DUPLICATE",
            Self::ResolveImport => "W-RESOLVE-IMPORT",
            Self::ResolvePrivate => "W-RESOLVE-PRIVATE",
            Self::ResolveUnknownName => "W-RESOLVE-NAME",
            Self::ResolveWrongKind => "W-RESOLVE-KIND",
            Self::TypeUnknownType => "W-TYPE-UNKNOWN",
            Self::TypeMismatch => "W-TYPE-MISMATCH",
            Self::TypeReturn => "W-TYPE-RETURN",
            Self::TypeCall => "W-TYPE-CALL",
            Self::TypeArgument => "W-TYPE-ARG",
            Self::OwnershipMove => "W-OWN-MOVE",
            Self::OwnershipAccess => "W-OWN-ACCESS",
            Self::EffectUnsupported => "W-EFFECT-UNSUPPORTED",
            Self::LayoutInvalid => "W-LAYOUT-INVALID",
            Self::CheckUnsupported => "W-CHECK-UNSUPPORTED",
            Self::CheckIo => "W-CHECK-IO",
        }
    }

    pub const fn phase(self) -> DiagnosticPhase {
        match self {
            Self::ParseExpectedItem
            | Self::ParseExpectedIdentifier
            | Self::ParseExpectedImportBinderList
            | Self::ParseExpectedFrom
            | Self::ParseExpectedModulePath
            | Self::ParseInvalidImportBinder
            | Self::ParseExpectedType
            | Self::ParseExpectedExpression
            | Self::ParseExpectedReturnAfterElse
            | Self::ParseExpectedMatchArm
            | Self::ParseExpectedAssertKind
            | Self::ParseExpectedToken
            | Self::ParseUnexpectedToken
            | Self::ParseMissingCloseDelimiter
            | Self::ParseExpressionTooDeep => DiagnosticPhase::Parse,
            Self::SummaryInvalidModule => DiagnosticPhase::Summary,
            Self::ResolveDuplicate
            | Self::ResolveImport
            | Self::ResolvePrivate
            | Self::ResolveUnknownName
            | Self::ResolveWrongKind => DiagnosticPhase::Resolve,
            Self::TypeUnknownType
            | Self::TypeMismatch
            | Self::TypeReturn
            | Self::TypeCall
            | Self::TypeArgument => DiagnosticPhase::Type,
            Self::OwnershipMove | Self::OwnershipAccess => DiagnosticPhase::Ownership,
            Self::EffectUnsupported => DiagnosticPhase::Effect,
            Self::LayoutInvalid => DiagnosticPhase::Layout,
            Self::CheckUnsupported | Self::CheckIo => DiagnosticPhase::Internal,
        }
    }

    pub const fn from_syntax_error(kind: SyntaxErrorKind) -> Self {
        match kind {
            SyntaxErrorKind::ExpectedItem => Self::ParseExpectedItem,
            SyntaxErrorKind::ExpectedIdentifier => Self::ParseExpectedIdentifier,
            SyntaxErrorKind::ExpectedImportBinderList => Self::ParseExpectedImportBinderList,
            SyntaxErrorKind::ExpectedFrom => Self::ParseExpectedFrom,
            SyntaxErrorKind::ExpectedModulePath => Self::ParseExpectedModulePath,
            SyntaxErrorKind::InvalidImportBinder => Self::ParseInvalidImportBinder,
            SyntaxErrorKind::ExpectedType => Self::ParseExpectedType,
            SyntaxErrorKind::ExpectedExpression => Self::ParseExpectedExpression,
            SyntaxErrorKind::ExpectedReturnAfterElse => Self::ParseExpectedReturnAfterElse,
            SyntaxErrorKind::ExpectedMatchArm => Self::ParseExpectedMatchArm,
            SyntaxErrorKind::ExpectedAssertKind => Self::ParseExpectedAssertKind,
            SyntaxErrorKind::ExpectedToken => Self::ParseExpectedToken,
            SyntaxErrorKind::UnexpectedToken => Self::ParseUnexpectedToken,
            SyntaxErrorKind::MissingCloseDelimiter => Self::ParseMissingCloseDelimiter,
            SyntaxErrorKind::ExpressionTooDeep => Self::ParseExpressionTooDeep,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    span: Span,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatedLocation {
    span: Span,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Applicability {
    Certain,
    Likely,
    Maybe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEdit {
    span: Span,
    replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedFix {
    title: String,
    applicability: Applicability,
    edits: Vec<SourceEdit>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticGroupId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticGroupRole {
    RootCause,
    Cascade,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticGroup {
    id: DiagnosticGroupId,
    role: DiagnosticGroupRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    code: Option<DiagnosticCode>,
    phase: DiagnosticPhase,
    message: String,
    primary: Option<DiagnosticLabel>,
    secondary: Vec<DiagnosticLabel>,
    related: Vec<RelatedLocation>,
    notes: Vec<String>,
    help: Vec<String>,
    suggestions: Vec<SuggestedFix>,
    group: Option<DiagnosticGroup>,
}

pub struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl Diagnostic {
    pub fn builder(
        severity: Severity,
        code: DiagnosticCode,
        message: impl Into<String>,
    ) -> DiagnosticBuilder {
        DiagnosticBuilder {
            diagnostic: Diagnostic {
                severity,
                code: Some(code),
                phase: code.phase(),
                message: message.into(),
                primary: None,
                secondary: Vec::new(),
                related: Vec::new(),
                notes: Vec::new(),
                help: Vec::new(),
                suggestions: Vec::new(),
                group: None,
            },
        }
    }

    pub fn new(severity: Severity, span: Option<Span>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            severity,
            code: None,
            phase: DiagnosticPhase::Internal,
            primary: span.map(|span| DiagnosticLabel::new(span, "")),
            secondary: Vec::new(),
            related: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            suggestions: Vec::new(),
            group: None,
            message,
        }
    }

    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, Some(span), message)
    }

    pub fn unspanned_error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, None, message)
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, Some(span), message)
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn code(&self) -> Option<DiagnosticCode> {
        self.code
    }

    pub fn code_string(&self) -> Option<&'static str> {
        self.code.map(DiagnosticCode::code)
    }

    pub fn phase(&self) -> DiagnosticPhase {
        self.phase
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> Option<Span> {
        self.primary.as_ref().map(DiagnosticLabel::span)
    }

    pub fn primary(&self) -> Option<&DiagnosticLabel> {
        self.primary.as_ref()
    }

    pub fn secondary(&self) -> &[DiagnosticLabel] {
        &self.secondary
    }

    pub fn related(&self) -> &[RelatedLocation] {
        &self.related
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn help(&self) -> &[String] {
        &self.help
    }

    pub fn suggestions(&self) -> &[SuggestedFix] {
        &self.suggestions
    }

    pub fn group(&self) -> Option<DiagnosticGroup> {
        self.group
    }

    pub fn render_compact(&self) -> String {
        match self.span() {
            Some(span) => format!(
                "{}[file={} {}..{}]: {}",
                self.severity.label(),
                span.file_id().raw(),
                span.start(),
                span.end(),
                self.message
            ),
            None => format!("{}: {}", self.severity.label(), self.message),
        }
    }
}

impl DiagnosticBuilder {
    pub fn primary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.diagnostic.primary = Some(DiagnosticLabel::new(span, label));
        self
    }

    pub fn secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.diagnostic
            .secondary
            .push(DiagnosticLabel::new(span, label));
        self
    }

    pub fn related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.diagnostic
            .related
            .push(RelatedLocation::new(span, message));
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic.notes.push(note.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic.help.push(help.into());
        self
    }

    pub fn suggestion(
        mut self,
        title: impl Into<String>,
        applicability: Applicability,
        edits: Vec<SourceEdit>,
    ) -> Self {
        self.diagnostic.suggestions.push(SuggestedFix {
            title: title.into(),
            applicability,
            edits,
        });
        self
    }

    pub fn root_cause_group(mut self, id: DiagnosticGroupId) -> Self {
        self.diagnostic.group = Some(DiagnosticGroup {
            id,
            role: DiagnosticGroupRole::RootCause,
        });
        self
    }

    pub fn cascade_group(mut self, id: DiagnosticGroupId) -> Self {
        self.diagnostic.group = Some(DiagnosticGroup {
            id,
            role: DiagnosticGroupRole::Cascade,
        });
        self
    }

    pub fn finish(self) -> Diagnostic {
        self.diagnostic
    }
}

impl DiagnosticLabel {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl RelatedLocation {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl SourceEdit {
    pub fn replace(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

pub fn source_edits_are_sorted_and_disjoint(edits: &[SourceEdit]) -> bool {
    let mut previous: Option<Span> = None;
    for edit in edits {
        if let Some(prev) = previous {
            if prev.file_id() > edit.span().file_id()
                || (prev.file_id() == edit.span().file_id() && prev.end() > edit.span().start())
            {
                return false;
            }
        }
        previous = Some(edit.span());
    }
    true
}

impl SuggestedFix {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn applicability(&self) -> Applicability {
        self.applicability
    }

    pub fn edits(&self) -> &[SourceEdit] {
        &self.edits
    }
}

impl DiagnosticGroupId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl DiagnosticGroup {
    pub const fn id(self) -> DiagnosticGroupId {
        self.id
    }

    pub const fn role(self) -> DiagnosticGroupRole {
        self.role
    }
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity() == Severity::Error)
}

pub fn render_diagnostics(diagnostics: &[Diagnostic], source_map: &SourceMap) -> String {
    let mut rendered = String::new();
    for diagnostic in diagnostics {
        rendered.push_str(&render_one_diagnostic(diagnostic, source_map));
    }
    rendered
}

fn render_one_diagnostic(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
    let mut rendered = String::new();
    rendered.push_str(diagnostic.severity().label());
    if let Some(code) = diagnostic.code_string() {
        rendered.push('[');
        rendered.push_str(code);
        rendered.push(']');
    }
    rendered.push_str(": ");
    rendered.push_str(diagnostic.message());
    rendered.push('\n');

    if let Some(primary) = diagnostic.primary() {
        render_label_block(
            &mut rendered,
            "-->",
            primary.span(),
            primary.message(),
            source_map,
        );
    }

    for secondary in diagnostic.secondary() {
        render_label_block(
            &mut rendered,
            "   =",
            secondary.span(),
            secondary.message(),
            source_map,
        );
    }

    for related in diagnostic.related() {
        render_label_block(
            &mut rendered,
            "related:",
            related.span(),
            related.message(),
            source_map,
        );
    }

    for note in diagnostic.notes() {
        rendered.push_str("note: ");
        rendered.push_str(note);
        rendered.push('\n');
    }

    for help in diagnostic.help() {
        rendered.push_str("help: ");
        rendered.push_str(help);
        rendered.push('\n');
    }

    for suggestion in diagnostic.suggestions() {
        rendered.push_str("suggestion (");
        rendered.push_str(applicability_label(suggestion.applicability()));
        rendered.push_str("): ");
        rendered.push_str(suggestion.title());
        rendered.push('\n');
    }

    rendered
}

fn render_label_block(
    rendered: &mut String,
    prefix: &str,
    span: Span,
    label: &str,
    source_map: &SourceMap,
) {
    let Some(file) = source_map.get(span.file_id()) else {
        rendered.push_str(prefix);
        rendered.push_str(" <unknown source>\n");
        return;
    };

    let location = file.line_col(span.start());
    rendered.push_str(prefix);
    rendered.push(' ');
    rendered.push_str(&file.path().to_string_lossy());
    rendered.push(':');
    rendered.push_str(&location.line().to_string());
    rendered.push(':');
    rendered.push_str(&location.column().to_string());
    rendered.push('\n');

    let (line_start, line_end, line_text) = source_line(file.text(), span.start());
    let line_number = location.line();
    rendered.push_str(&format!("{line_number:>4} | {line_text}\n"));

    let line_byte_start = line_start as usize;
    let span_start = span.start() as usize;
    let end = span.end().min(line_end) as usize;
    let start_column = file.text()[line_byte_start..span_start].chars().count();
    let width = file.text()[span_start..end].chars().count().max(1);
    rendered.push_str("     | ");
    rendered.push_str(&" ".repeat(start_column));
    rendered.push_str(&"^".repeat(width));
    if !label.is_empty() {
        rendered.push(' ');
        rendered.push_str(label);
    }
    rendered.push('\n');
}

fn source_line(text: &str, offset: u32) -> (u32, u32, &str) {
    let bytes = text.as_bytes();
    let mut start = offset.min(bytes.len() as u32) as usize;
    while start > 0 && bytes[start - 1] != b'\n' {
        start -= 1;
    }
    let mut end = offset.min(bytes.len() as u32) as usize;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    (start as u32, end as u32, &text[start..end])
}

fn applicability_label(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::Certain => "certain",
        Applicability::Likely => "likely",
        Applicability::Maybe => "maybe",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};
    use crate::syntax::SyntaxErrorKind;

    #[test]
    fn detects_errors() {
        let diagnostics = vec![
            Diagnostic::new(
                Severity::Warning,
                Some(Span::new(FileId::new(0), 1, 2)),
                "warning",
            ),
            Diagnostic::new(
                Severity::Error,
                Some(Span::new(FileId::new(0), 2, 3)),
                "error",
            ),
        ];

        assert!(has_errors(&diagnostics));
    }

    #[test]
    fn renders_single_line() {
        let diagnostic = Diagnostic::new(
            Severity::Error,
            Some(Span::new(FileId::new(3), 4, 9)),
            "bad token",
        );

        assert_eq!(diagnostic.render_compact(), "error[file=3 4..9]: bad token");
    }

    #[test]
    fn renders_unspanned_diagnostics() {
        let diagnostic = Diagnostic::new(Severity::Error, None, "could not load root file");

        assert_eq!(
            diagnostic.render_compact(),
            "error: could not load root file"
        );
    }

    #[test]
    fn structured_diagnostic_stores_codes_labels_groups_and_fixes() {
        let span = Span::new(FileId::new(0), 10, 16);
        let fix_span = Span::new(FileId::new(0), 10, 16);
        let group = DiagnosticGroupId::new(7);

        let diagnostic = Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::ResolveUnknownName,
            "unknown name `Consol`",
        )
        .primary(span, "`Consol` is not in scope")
        .secondary(
            Span::new(FileId::new(0), 1, 8),
            "available import is declared here",
        )
        .related(span, "lookup failed in this scope")
        .note("names are resolved after imports are validated")
        .help("did you mean `Console`?")
        .suggestion(
            "replace with `Console`",
            Applicability::Likely,
            vec![SourceEdit::replace(fix_span, "Console")],
        )
        .root_cause_group(group)
        .finish();

        assert_eq!(diagnostic.severity(), Severity::Error);
        assert_eq!(diagnostic.code(), Some(DiagnosticCode::ResolveUnknownName));
        assert_eq!(diagnostic.code_string(), Some("W-RESOLVE-NAME"));
        assert_eq!(diagnostic.phase(), DiagnosticPhase::Resolve);
        assert_eq!(diagnostic.span(), Some(span));
        assert_eq!(diagnostic.primary().unwrap().span(), span);
        assert_eq!(diagnostic.secondary().len(), 1);
        assert_eq!(diagnostic.related().len(), 1);
        assert_eq!(
            diagnostic.notes(),
            &["names are resolved after imports are validated"]
        );
        assert_eq!(diagnostic.help(), &["did you mean `Console`?"]);
        assert_eq!(
            diagnostic.suggestions()[0].applicability(),
            Applicability::Likely
        );
        assert_eq!(
            diagnostic.suggestions()[0].edits()[0].replacement(),
            "Console"
        );
        assert_eq!(
            diagnostic.group().unwrap().role(),
            DiagnosticGroupRole::RootCause
        );
    }

    #[test]
    fn compatibility_constructors_keep_existing_span_api() {
        let span = Span::new(FileId::new(2), 4, 9);
        let diagnostic = Diagnostic::error(span, "bad token");

        assert_eq!(diagnostic.code(), None);
        assert_eq!(diagnostic.phase(), DiagnosticPhase::Internal);
        assert_eq!(diagnostic.span(), Some(span));
        assert_eq!(diagnostic.message(), "bad token");
        assert_eq!(diagnostic.render_compact(), "error[file=2 4..9]: bad token");
    }

    #[test]
    fn syntax_error_kind_expected_type_is_parse_phase() {
        assert_eq!(
            DiagnosticCode::from_syntax_error(SyntaxErrorKind::ExpectedType),
            DiagnosticCode::ParseExpectedType
        );
        assert_eq!(
            DiagnosticCode::ParseExpectedType.phase(),
            DiagnosticPhase::Parse
        );
        assert_eq!(
            DiagnosticCode::ParseExpectedType.code(),
            "W-PARSE-EXPECTED-TYPE"
        );
    }
}
