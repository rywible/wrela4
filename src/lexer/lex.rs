use crate::diagnostic::Diagnostic;
use crate::source::{FileId, SourceFile};
use super::token::Token;
use super::trivia::Trivia;

#[derive(Debug)]
pub struct LexedFile {
    file_id: FileId,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
}

impl LexedFile {
    pub fn new(file_id: FileId, tokens: Vec<Token>, trivia: Vec<Trivia>, diagnostics: Vec<Diagnostic>) -> Self {
        Self { file_id, tokens, trivia, diagnostics }
    }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn tokens(&self) -> &[Token] { &self.tokens }
    pub fn trivia(&self) -> &[Trivia] { &self.trivia }
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }
}

pub fn lex_file(source: &SourceFile) -> LexedFile {
    LexedFile::new(source.id(), Vec::new(), Vec::new(), Vec::new())
}
