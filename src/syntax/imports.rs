use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::{Keyword, LexedFile, Punct, Token, TokenKind};
use crate::source::{SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePath {
    segments: Vec<String>,
}

impl ModulePath {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn as_dotted(&self) -> String {
        self.segments.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportEdge {
    module: ModulePath,
    span: Span,
}

impl ImportEdge {
    pub fn new(module: ModulePath, span: Span) -> Self {
        Self { module, span }
    }

    pub fn module(&self) -> &ModulePath {
        &self.module
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug)]
pub struct ImportSummary {
    imports: Vec<ImportEdge>,
    diagnostics: Vec<Diagnostic>,
}

impl ImportSummary {
    pub fn new(imports: Vec<ImportEdge>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            imports,
            diagnostics,
        }
    }

    pub fn imports(&self) -> &[ImportEdge] {
        &self.imports
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub fn parse_import_summary(lexed: &LexedFile, source: &SourceFile) -> ImportSummary {
    let tokens = lexed.tokens();
    let mut imports = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if !matches!(tokens[index].kind(), TokenKind::Keyword(Keyword::Use)) {
            index += 1;
            continue;
        }

        let mut use_span = tokens[index].span();
        index += 1;

        loop {
            if index >= tokens.len() {
                diagnostics.push(parse_import_diagnostic(
                    DiagnosticCode::ParseExpectedFrom,
                    use_span,
                    "expected from in use import",
                ));
                break;
            }

            match tokens[index].kind() {
                TokenKind::Keyword(Keyword::From) => {
                    let from_span = tokens[index].span();
                    index += 1;

                    match parse_module_path(tokens, source, from_span, &mut index) {
                        Ok((module, span)) => {
                            imports.push(ImportEdge::new(module, span));
                        }
                        Err(diagnostic) => {
                            diagnostics.push(*diagnostic);
                            index = skip_to_next_use(tokens, index);
                        }
                    }
                    break;
                }
                TokenKind::Keyword(Keyword::Use) => {
                    diagnostics.push(parse_import_diagnostic(
                        DiagnosticCode::ParseExpectedFrom,
                        use_span,
                        "expected from in use import",
                    ));
                    use_span = tokens[index].span();
                    index += 1;
                }
                TokenKind::Eof => {
                    diagnostics.push(parse_import_diagnostic(
                        DiagnosticCode::ParseExpectedFrom,
                        use_span,
                        "expected from in use import",
                    ));
                    index = tokens.len();
                    break;
                }
                _ => index += 1,
            }
        }
    }

    ImportSummary::new(imports, diagnostics)
}

fn parse_import_diagnostic(code: DiagnosticCode, span: Span, message: &'static str) -> Diagnostic {
    Diagnostic::builder(Severity::Error, code, message)
        .primary(span, message)
        .finish()
}

fn parse_module_path(
    tokens: &[Token],
    source: &SourceFile,
    from_span: Span,
    index: &mut usize,
) -> Result<(ModulePath, Span), Box<Diagnostic>> {
    if *index >= tokens.len() || tokens[*index].kind() != TokenKind::Identifier {
        return Err(Box::new(parse_import_diagnostic(
            DiagnosticCode::ParseExpectedModulePath,
            from_span,
            "expected module path after from",
        )));
    }

    let first = tokens[*index];
    let mut segments = vec![token_text(source, first).to_string()];
    let path_start = first.span().start();
    let mut path_end = first.span().end();
    *index += 1;

    loop {
        if *index >= tokens.len() || tokens[*index].kind() != TokenKind::Punct(Punct::Dot) {
            break;
        }

        *index += 1;
        if *index >= tokens.len() || tokens[*index].kind() != TokenKind::Identifier {
            return Err(Box::new(parse_import_diagnostic(
                DiagnosticCode::ParseExpectedIdentifier,
                from_span,
                "expected identifier after dot in module path",
            )));
        }

        let segment = tokens[*index];
        segments.push(token_text(source, segment).to_string());
        path_end = segment.span().end();
        *index += 1;
    }

    if *index < tokens.len() && tokens[*index].kind() == TokenKind::Punct(Punct::Slash) {
        while *index < tokens.len() {
            match tokens[*index].kind() {
                TokenKind::Punct(Punct::Slash) => {
                    path_end = tokens[*index].span().end();
                    *index += 1;
                }
                TokenKind::Identifier => {
                    path_end = tokens[*index].span().end();
                    *index += 1;
                }
                _ => break,
            }
        }
        let span = Span::new(first.span().file_id(), path_start, path_end);
        return Err(Box::new(parse_import_diagnostic(
            DiagnosticCode::ParseExpectedModulePath,
            span,
            "invalid path separator in module path",
        )));
    }

    let span = Span::new(first.span().file_id(), path_start, path_end);
    if let Some(diagnostic) = validate_module_segments(&segments, span) {
        return Err(Box::new(diagnostic));
    }
    Ok((ModulePath::new(segments), span))
}

fn validate_module_segments(segments: &[String], span: Span) -> Option<Diagnostic> {
    for segment in segments {
        if segment == "wrela" {
            return Some(parse_import_diagnostic(
                DiagnosticCode::ParseExpectedModulePath,
                span,
                "module path must not include file extension",
            ));
        }
    }
    None
}

fn skip_to_next_use(tokens: &[Token], mut index: usize) -> usize {
    while index < tokens.len() {
        match tokens[index].kind() {
            TokenKind::Keyword(Keyword::Use) | TokenKind::Eof => return index,
            _ => index += 1,
        }
    }
    index
}

fn token_text(source: &SourceFile, token: Token) -> &str {
    let start = token.span().start() as usize;
    let end = token.span().end() as usize;
    debug_assert!(source.text().is_char_boundary(start));
    debug_assert!(source.text().is_char_boundary(end));
    &source.text()[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn summary(text: &str) -> ImportSummary {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        parse_import_summary(&lexed, &source)
    }

    #[test]
    fn extracts_dotted_import_paths() {
        let summary = summary("use { Console } from app.console\nuse { Storage } from app.storage");
        let modules: Vec<String> = summary
            .imports()
            .iter()
            .map(|import| import.module().as_dotted())
            .collect();

        assert_eq!(modules, vec!["app.console", "app.storage"]);
        assert!(summary.diagnostics().is_empty());
    }

    #[test]
    fn rejects_import_with_missing_module() {
        let summary = summary("use { Console } from");

        assert_eq!(
            summary.diagnostics()[0].message(),
            "expected module path after from"
        );
    }

    #[test]
    fn rejects_use_without_from() {
        let summary = summary("use { Console }");

        assert_eq!(
            summary.diagnostics()[0].message(),
            "expected from in use import"
        );
        assert_eq!(summary.diagnostics()[0].code_string(), Some("W-PARSE-FROM"));
    }

    #[test]
    fn recovers_when_second_use_appears_before_from() {
        let summary = summary("use { Broken } use { Console } from app.console");
        let modules: Vec<String> = summary
            .imports()
            .iter()
            .map(|import| import.module().as_dotted())
            .collect();

        assert_eq!(
            summary.diagnostics()[0].message(),
            "expected from in use import"
        );
        assert_eq!(modules, vec!["app.console"]);
    }

    #[test]
    fn rejects_invalid_module_start_after_from() {
        let summary = summary("use { Console } from 123");

        assert_eq!(
            summary.diagnostics()[0].message(),
            "expected module path after from"
        );
    }

    #[test]
    fn rejects_module_path_with_trailing_dot() {
        let summary = summary("use { Console } from app.console.");

        assert_eq!(
            summary.diagnostics()[0].message(),
            "expected identifier after dot in module path"
        );
    }

    #[test]
    fn rejects_module_path_with_file_extension_segment() {
        let summary = summary("use { Console } from app.console.wrela");

        assert_eq!(
            summary.diagnostics()[0].message(),
            "module path must not include file extension"
        );
    }

    #[test]
    fn rejects_module_path_with_slash_separator() {
        let summary = summary("use { Console } from app/console");

        assert!(summary.imports().is_empty());
        assert_eq!(
            summary.diagnostics()[0].message(),
            "invalid path separator in module path"
        );
    }
}
