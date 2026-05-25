use crate::diagnostic::Diagnostic;
use crate::lexer::{Keyword, LexedFile, Punct, Token, TokenKind, TriviaKind};
use crate::source::{SourceFile, SourceMap, Span};

use super::cst::{ParsedSyntax, SyntaxToken, SyntaxTreeBuilder, TokenIndex, TriviaRange};
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

pub fn parse_file(lexed: &LexedFile, source: &SourceFile) -> ParsedSyntax {
    Parser::new(lexed, source).parse_module()
}

pub fn parse_files_parallel(
    lexed_files: &[LexedFile],
    source_map: &SourceMap,
) -> Vec<ParsedSyntax> {
    if lexed_files.is_empty() {
        return Vec::new();
    }

    let worker_count = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(lexed_files.len());
    let chunk_size = lexed_files.len().div_ceil(worker_count);

    let mut parsed = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in lexed_files.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|lexed| {
                        let source = source_map
                            .get(lexed.file_id())
                            .expect("lexed file has source");
                        parse_file(lexed, source)
                    })
                    .collect::<Vec<_>>()
            }));
        }

        let mut parsed = Vec::with_capacity(lexed_files.len());
        for handle in handles {
            match handle.join() {
                Ok(mut chunk) => parsed.append(&mut chunk),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
        parsed
    });
    parsed.sort_by_key(|module| module.file_id().raw());
    parsed
}

pub(crate) struct Parser<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) lexed: &'a LexedFile,
    pub(crate) tokens: &'a [Token],
    pub(crate) token_index: usize,
    pub(crate) trivia_index: usize,
    eof_emitted: bool,
    pub(crate) builder: SyntaxTreeBuilder,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(lexed: &'a LexedFile, source: &'a SourceFile) -> Self {
        Self {
            source,
            lexed,
            tokens: lexed.tokens(),
            token_index: 0,
            trivia_index: 0,
            eof_emitted: false,
            builder: SyntaxTreeBuilder::new(lexed.file_id()),
            diagnostics: Vec::new(),
        }
    }

    fn parse_module(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        while !self.at(TokenKind::Eof) {
            self.parse_item();
        }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }

    pub(crate) fn before_close(&self, close: Punct) -> bool {
        self.peek().kind() != TokenKind::Punct(close) && self.peek().kind() != TokenKind::Eof
    }

    pub(crate) fn parse_list_until<F>(
        &mut self,
        close: Punct,
        close_label: &'static str,
        mut parse_element: F,
    ) where
        F: FnMut(&mut Self),
    {
        while self.before_close(close) {
            let index_before = self.token_index;
            parse_element(self);
            if self.token_index == index_before {
                self.recover_delimited_element(close);
            }
        }
        self.expect_close_punct(close, close_label);
    }

    pub(crate) fn parse_braced<F>(
        &mut self,
        open_label: &'static str,
        close_label: &'static str,
        parse_element: F,
    ) where
        F: FnMut(&mut Self),
    {
        self.expect_punct(Punct::OpenBrace, open_label);
        self.parse_list_until(Punct::CloseBrace, close_label, parse_element);
    }
    pub(crate) fn peek(&self) -> Token {
        self.tokens[self.token_index]
    }

    pub(crate) fn peek_n(&self, offset: usize) -> Token {
        self.tokens[(self.token_index + offset).min(self.tokens.len() - 1)]
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind() == kind
    }

    pub(crate) fn checkpoint(&self) -> super::cst::Checkpoint {
        self.builder.checkpoint()
    }

    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind, self.peek().span());
    }

    pub(crate) fn start_node_at(&mut self, checkpoint: super::cst::Checkpoint, kind: SyntaxKind) {
        self.builder
            .start_node_at(checkpoint, kind, self.peek().span());
    }

    pub(crate) fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    pub(crate) fn bump(&mut self) {
        let token_index = self.token_index;
        let token = self.tokens[token_index];
        if token.kind() == TokenKind::Eof && self.eof_emitted {
            return;
        }
        let leading = self.take_leading_trivia();
        let trailing = self.take_trailing_trivia(token);
        self.builder.token(SyntaxToken::new(
            TokenIndex::new(token_index as u32),
            token.span(),
            leading,
            trailing,
        ));
        if token.kind() == TokenKind::Eof {
            self.eof_emitted = true;
        } else {
            self.token_index += 1;
        }
    }

    fn take_leading_trivia(&mut self) -> TriviaRange {
        let start = self.trivia_index as u32;
        let token_start = self.peek().span().start();
        while self
            .lexed
            .trivia()
            .get(self.trivia_index)
            .is_some_and(|trivia| trivia.span().end() <= token_start)
        {
            self.trivia_index += 1;
        }
        TriviaRange::new(start, self.trivia_index as u32)
    }

    fn take_trailing_trivia(&mut self, token: Token) -> TriviaRange {
        let start = self.trivia_index as u32;
        let next_start = self
            .tokens
            .get(self.token_index + 1)
            .map(|next| next.span().start())
            .unwrap_or(u32::MAX);
        while let Some(trivia) = self.lexed.trivia().get(self.trivia_index) {
            if trivia.span().start() < token.span().end() {
                break;
            }
            if trivia.span().start() >= next_start {
                break;
            }
            if trivia.kind() == TriviaKind::Newline {
                self.trivia_index += 1;
                break;
            }
            self.trivia_index += 1;
        }
        TriviaRange::new(start, self.trivia_index as u32)
    }

    pub(crate) fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        if self.peek().kind() == TokenKind::Keyword(keyword) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_punct(&mut self, punct: Punct) -> bool {
        if self.peek().kind() == TokenKind::Punct(punct) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_keyword(&mut self, keyword: Keyword, label: &'static str) -> bool {
        if self.eat_keyword(keyword) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> bool {
        if self.peek().kind() == TokenKind::Identifier {
            self.bump();
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedIdentifier, "expected identifier");
            false
        }
    }

    /// Parameter, field, and local names may use a small set of contextual keywords
    /// (e.g. `host` in `host: Type`).
    pub(crate) fn expect_binding_name(&mut self) -> bool {
        match self.peek().kind() {
            TokenKind::Identifier => {
                self.bump();
                true
            }
            TokenKind::Keyword(keyword) if Self::is_contextual_binding_keyword(keyword) => {
                self.bump();
                true
            }
            _ => {
                self.error_at_current(SyntaxErrorKind::ExpectedIdentifier, "expected identifier");
                false
            }
        }
    }

    fn is_contextual_binding_keyword(keyword: Keyword) -> bool {
        matches!(
            keyword,
            Keyword::Host | Keyword::Target | Keyword::Value | Keyword::Same | Keyword::Until
        )
    }

    pub(crate) fn expect_contextual_identifier(
        &mut self,
        text: &'static str,
        label: &'static str,
    ) -> bool {
        if self.peek().kind() == TokenKind::Identifier && self.token_text(self.peek()) == text {
            self.bump();
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_punct(&mut self, punct: Punct, label: &'static str) -> bool {
        if self.eat_punct(punct) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedToken, label);
            false
        }
    }

    pub(crate) fn expect_close_punct(&mut self, punct: Punct, label: &'static str) -> bool {
        if self.eat_punct(punct) {
            true
        } else {
            self.error_at_current(SyntaxErrorKind::MissingCloseDelimiter, label);
            false
        }
    }

    pub(crate) fn error_at_current(&mut self, kind: SyntaxErrorKind, message: &'static str) {
        let span = self.peek().span();
        self.diagnostic(span, message);
        self.builder.error(kind, span);
    }

    pub(crate) fn diagnostic(&mut self, span: Span, message: &'static str) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }

    pub(crate) fn token_text(&self, token: Token) -> &str {
        &self.source.text()[token.span().start() as usize..token.span().end() as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::has_errors;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use crate::syntax::testing::{parse_fragment_text, tree_contains};
    use std::path::PathBuf;

    fn parse_block_text(text: &str) -> ParsedSyntax {
        parse_fragment_text(text, "block.wrela", |parser| parser.parse_block())
    }

    #[test]
    fn parses_basic_block_statements() {
        let parsed = parse_block_text("{ let result = worker.run(input = 1) return result }");

        assert!(!has_errors(parsed.diagnostics()));
        assert!(tree_contains(parsed.tree(), SyntaxKind::LetStmt));
        assert!(tree_contains(parsed.tree(), SyntaxKind::ReturnStmt));
    }

    #[test]
    fn let_requires_type_or_initializer() {
        let parsed = parse_block_text("{ let dangling }");
        assert!(parsed.diagnostics().iter().any(|diagnostic| {
            diagnostic.message() == "expected type annotation or initializer in let statement"
        }));
    }

    #[test]
    fn parse_file_reconstructs_source_with_attached_trivia_and_indentation() {
        let text = "  use { Console } from app.console // trailing\n\n    module app.root\n";
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);

        assert_eq!(parsed.tree().source_text(&lexed, &source), text);
    }
}
