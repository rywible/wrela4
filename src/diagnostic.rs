use crate::source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    span: Option<Span>,
    message: String,
}

impl Diagnostic {
    pub fn new(severity: Severity, span: Option<Span>, message: impl Into<String>) -> Self {
        Self {
            severity,
            span,
            message: message.into(),
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

    pub fn span(&self) -> Option<Span> {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn render_compact(&self) -> String {
        match self.span {
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

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};

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
}
