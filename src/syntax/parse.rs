use crate::diagnostic::Diagnostic;
use crate::lexer::{Keyword, LexedFile, Punct, Token, TokenKind, TriviaKind};
use crate::source::{SourceFile, Span};

use super::cst::{ParsedSyntax, SyntaxToken, SyntaxTreeBuilder, TokenIndex, TriviaRange};
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

pub fn parse_file(lexed: &LexedFile, source: &SourceFile) -> ParsedSyntax {
    Parser::new(lexed, source).parse_module()
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

#[allow(dead_code)]
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

    pub(crate) fn parse_item(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Module) => self.parse_module_decl(),
            TokenKind::Keyword(Keyword::Use) => self.parse_use_decl(),
            _ => self.parse_error_item(),
        }
    }

    fn parse_module_decl(&mut self) {
        self.start_node(SyntaxKind::ModuleDecl);
        self.bump();
        self.parse_module_path();
        self.finish_node();
    }

    fn parse_use_decl(&mut self) {
        self.start_node(SyntaxKind::UseDecl);
        self.bump();
        if self.eat_punct(Punct::OpenBrace) {
            self.start_node(SyntaxKind::UseBinderList);
            if self.peek().kind() != TokenKind::Punct(Punct::CloseBrace) {
                self.parse_use_binder();
                while self.eat_punct(Punct::Comma) {
                    if self.peek().kind() == TokenKind::Punct(Punct::CloseBrace) {
                        break;
                    }
                    self.parse_use_binder();
                }
            }
            self.finish_node();
            self.expect_close_punct(Punct::CloseBrace, "expected '}'");
        } else {
            let span = self.peek().span();
            self.diagnostic(span, "expected import binder list");
            self.builder
                .error(SyntaxErrorKind::ExpectedImportBinderList, span);
        }
        if self.eat_keyword(Keyword::From) {
            self.parse_module_path_after_from();
        } else {
            let span = self.peek().span();
            self.diagnostic(span, "expected from in use import");
            self.builder.error(SyntaxErrorKind::ExpectedFrom, span);
        }
        self.finish_node();
    }

    fn parse_use_binder(&mut self) {
        self.start_node(SyntaxKind::UseBinder);
        if self.peek().kind() == TokenKind::Punct(Punct::Star) {
            let span = self.peek().span();
            self.diagnostic(span, "wildcard imports are not supported in v1");
            self.builder
                .error(SyntaxErrorKind::InvalidImportBinder, span);
            self.bump();
        } else if self.expect_identifier() && self.eat_keyword(Keyword::As) {
            let span = self.peek().span();
            self.diagnostic(span, "import aliases are not supported in v1");
            self.builder
                .error(SyntaxErrorKind::InvalidImportBinder, span);
            self.expect_identifier();
        }
        self.finish_node();
    }

    fn parse_module_path_after_from(&mut self) {
        if self.peek().kind() != TokenKind::Identifier {
            let span = self.peek().span();
            self.diagnostic(span, "expected module path after from");
            self.builder
                .error(SyntaxErrorKind::ExpectedModulePath, span);
            return;
        }
        self.parse_module_path();
    }

    fn parse_module_path(&mut self) {
        self.start_node(SyntaxKind::ModulePath);
        self.expect_identifier();
        while self.eat_punct(Punct::Dot) {
            self.expect_identifier();
        }
        self.finish_node();
    }

    pub(crate) fn parse_error_item(&mut self) {
        let span = self.peek().span();
        self.start_node(SyntaxKind::RecoveryNode);
        self.diagnostic(span, "expected item");
        self.builder.error(SyntaxErrorKind::ExpectedItem, span);
        self.bump();
        self.finish_node();
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
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

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
