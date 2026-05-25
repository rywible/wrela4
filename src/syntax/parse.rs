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
            TokenKind::Keyword(Keyword::Pub) => self.parse_pub_item(),
            TokenKind::Keyword(Keyword::Module) => self.parse_module_decl(),
            TokenKind::Keyword(Keyword::Use) => self.parse_use_decl(),
            TokenKind::Keyword(Keyword::Data) => self.parse_data_decl(),
            TokenKind::Keyword(Keyword::Layout) => self.parse_layout_data_decl(),
            TokenKind::Keyword(Keyword::Class) => self.parse_class_decl(),
            TokenKind::Keyword(Keyword::Unique) => self.parse_unique_class_decl(),
            TokenKind::Keyword(Keyword::Interface) => self.parse_interface_decl(),
            TokenKind::Keyword(Keyword::Error) => self.parse_error_decl(),
            TokenKind::Keyword(Keyword::Image) => self.parse_image_decl(),
            TokenKind::Keyword(Keyword::Host) => self.parse_host_image_decl(),
            _ => self.parse_error_item(),
        }
    }

    fn parse_pub_item(&mut self) {
        self.start_node(SyntaxKind::PublicItem);
        self.start_node(SyntaxKind::PubModifier);
        self.bump();
        self.finish_node();
        self.parse_item();
        self.finish_node();
    }

    fn parse_data_decl(&mut self) {
        self.start_node(SyntaxKind::DataDecl);
        self.bump();
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_layout_data_decl(&mut self) {
        self.start_node(SyntaxKind::LayoutDataDecl);
        self.bump(); // layout
        self.expect_identifier(); // layout ABI, such as C
        self.expect_keyword(Keyword::Data, "expected data after layout");
        self.expect_identifier();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_interface_decl(&mut self) {
        self.start_node(SyntaxKind::InterfaceDecl);
        self.bump();
        self.expect_identifier();
        self.parse_generic_param_list();
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
            && self.peek().kind() != TokenKind::Eof
        {
            self.parse_method_signature_decl();
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
        self.finish_node();
    }

    fn parse_error_decl(&mut self) {
        self.start_node(SyntaxKind::ErrorDecl);
        self.bump();
        self.expect_identifier();
        self.parse_field_block();
        self.finish_node();
    }

    fn parse_unique_class_decl(&mut self) {
        self.start_node(SyntaxKind::UniqueClassDecl);
        self.bump();
        self.expect_keyword(Keyword::Class, "expected class after unique");
        self.parse_class_tail();
        self.finish_node();
    }

    fn parse_class_decl(&mut self) {
        self.start_node(SyntaxKind::ClassDecl);
        self.bump();
        self.parse_class_tail();
        self.finish_node();
    }

    fn parse_class_tail(&mut self) {
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_implements_clause();
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
            && self.peek().kind() != TokenKind::Eof
        {
            self.parse_member();
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    }

    fn parse_implements_clause(&mut self) {
        if self.eat_keyword(Keyword::Implements) {
            self.start_node(SyntaxKind::ImplementsClause);
            self.parse_type_ref();
            while self.eat_punct(Punct::Comma) {
                self.parse_type_ref();
            }
            self.finish_node();
        }
    }

    fn parse_field_block(&mut self) {
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
            && self.peek().kind() != TokenKind::Eof
        {
            self.parse_field_decl();
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    }

    fn parse_field_decl(&mut self) {
        self.start_node(SyntaxKind::FieldDecl);
        self.expect_binding_name();
        self.expect_punct(Punct::Colon, "expected ':'");
        self.parse_type_ref();
        self.finish_node();
    }

    fn parse_member(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Constructor) => self.parse_constructor_decl(),
            TokenKind::Keyword(Keyword::Fn) | TokenKind::Keyword(Keyword::Asm) => {
                self.parse_method_decl()
            }
            TokenKind::Keyword(Keyword::Test) => self.parse_test_decl(),
            TokenKind::Identifier
                if self.peek_n(1).kind() == TokenKind::Punct(Punct::Colon) =>
            {
                self.parse_field_decl()
            }
            _ => self.parse_member_error(),
        }
    }

    fn parse_method_decl(&mut self) {
        self.start_node(SyntaxKind::MethodDecl);
        self.parse_method_head();
        self.parse_block();
        self.finish_node();
    }

    fn parse_constructor_decl(&mut self) {
        self.start_node(SyntaxKind::ConstructorDecl);
        self.bump();
        self.parse_param_list();
        if self.eat_punct(Punct::Arrow) {
            self.parse_return_type();
        }
        self.parse_block();
        self.finish_node();
    }

    fn parse_test_decl(&mut self) {
        self.start_node(SyntaxKind::TestDecl);
        self.bump();
        if self.peek().kind() == TokenKind::StringLiteral {
            self.bump();
        } else {
            self.expect_identifier();
        }
        self.parse_block();
        self.finish_node();
    }

    fn parse_phase_decl(&mut self) {
        self.start_node(SyntaxKind::PhaseDecl);
        self.bump();
        self.expect_identifier();
        self.parse_param_list();
        self.parse_block();
        self.finish_node();
    }

    fn parse_method_signature_decl(&mut self) {
        self.start_node(SyntaxKind::MethodDecl);
        self.parse_method_head();
        self.finish_node();
    }

    fn parse_method_head(&mut self) {
        if self.eat_keyword(Keyword::Asm) {
            self.expect_keyword(Keyword::Fn, "expected item");
        } else {
            self.expect_keyword(Keyword::Fn, "expected item");
        }
        self.expect_identifier();
        self.parse_generic_param_list();
        self.parse_param_list();
        if self.eat_punct(Punct::Arrow) {
            self.parse_return_type();
        }
    }

    fn parse_return_type(&mut self) {
        self.start_node(SyntaxKind::ReturnType);
        self.parse_type_ref();
        self.finish_node();
    }

    fn parse_param_list(&mut self) {
        self.start_node(SyntaxKind::ParamList);
        self.expect_punct(Punct::OpenParen, "expected '('");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseParen)
            && self.peek().kind() != TokenKind::Eof
        {
            self.start_node(SyntaxKind::Param);
            if matches!(
                self.peek().kind(),
                TokenKind::Keyword(Keyword::Read)
                    | TokenKind::Keyword(Keyword::Mut)
                    | TokenKind::Keyword(Keyword::Own)
            ) {
                self.bump();
            }
            self.expect_binding_name();
            if self.eat_punct(Punct::Colon) {
                self.parse_type_ref();
            }
            self.finish_node();
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        self.expect_close_punct(Punct::CloseParen, "expected ')'");
        self.finish_node();
    }

    fn parse_image_decl(&mut self) {
        self.start_node(SyntaxKind::ImageDecl);
        self.bump();
        self.expect_identifier();
        self.expect_keyword(Keyword::Target, "expected target in image declaration");
        self.parse_type_ref();
        self.parse_image_body();
        self.finish_node();
    }

    fn parse_host_image_decl(&mut self) {
        self.start_node(SyntaxKind::HostImageDecl);
        self.bump();
        self.expect_keyword(Keyword::Image, "expected image after host");
        self.expect_identifier();
        self.parse_image_body();
        self.finish_node();
    }

    fn parse_image_body(&mut self) {
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
            && self.peek().kind() != TokenKind::Eof
        {
            if self.peek().kind() == TokenKind::Keyword(Keyword::Phase) {
                self.parse_phase_decl();
            } else {
                self.parse_error_item();
            }
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
    }

    fn parse_member_error(&mut self) {
        let span = self.peek().span();
        self.start_node(SyntaxKind::RecoveryNode);
        self.diagnostic(span, "unexpected token in class body");
        self.builder.error(SyntaxErrorKind::UnexpectedToken, span);
        self.bump();
        self.finish_node();
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

    pub(crate) fn parse_block(&mut self) {
        self.start_node(SyntaxKind::Block);
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
            && self.peek().kind() != TokenKind::Eof
        {
            let index_before = self.token_index;
            self.parse_stmt();
            if self.token_index == index_before
                && self.peek().kind() != TokenKind::Punct(Punct::CloseBrace)
                && self.peek().kind() != TokenKind::Eof
            {
                self.error_at_current(SyntaxErrorKind::UnexpectedToken, "unexpected token in statement");
                self.bump();
            }
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
        self.finish_node();
    }

    pub(crate) fn parse_stmt(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Let) => self.parse_let_stmt(),
            TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) {
        self.start_node(SyntaxKind::LetStmt);
        self.bump();
        self.expect_binding_name();
        let has_type = if self.eat_punct(Punct::Colon) {
            self.parse_type_ref();
            true
        } else {
            false
        };
        let has_initializer = if self.eat_punct(Punct::Eq) {
            self.parse_expr();
            true
        } else {
            false
        };
        if !has_type && !has_initializer {
            self.error_at_current(
                SyntaxErrorKind::ExpectedToken,
                "expected type annotation or initializer in let statement",
            );
        }
        self.eat_punct(Punct::Semicolon);
        self.finish_node();
    }

    fn parse_return_stmt(&mut self) {
        self.start_node(SyntaxKind::ReturnStmt);
        self.bump();
        if !self.at_statement_boundary() {
            self.parse_expr();
        }
        self.eat_punct(Punct::Semicolon);
        self.finish_node();
    }

    fn parse_expr_stmt(&mut self) {
        self.start_node(SyntaxKind::ExprStmt);
        self.parse_expr();
        self.eat_punct(Punct::Semicolon);
        self.finish_node();
    }

    pub(crate) fn at_statement_boundary(&self) -> bool {
        matches!(
            self.peek().kind(),
            TokenKind::Eof
                | TokenKind::Punct(Punct::CloseBrace)
                | TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Return)
                | TokenKind::Keyword(Keyword::Match)
                | TokenKind::Keyword(Keyword::Repeat)
                | TokenKind::Keyword(Keyword::For)
                | TokenKind::Keyword(Keyword::Drain)
                | TokenKind::Keyword(Keyword::Loop)
                | TokenKind::Keyword(Keyword::Assert)
        )
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

    /// Parameter, field, and local names may use lexed keywords (e.g. `host` in `host: Type`).
    pub(crate) fn expect_binding_name(&mut self) -> bool {
        match self.peek().kind() {
            TokenKind::Identifier | TokenKind::Keyword(_) => {
                self.bump();
                true
            }
            _ => {
                self.error_at_current(SyntaxErrorKind::ExpectedIdentifier, "expected identifier");
                false
            }
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
impl<'a> Parser<'a> {
    pub(crate) fn parse_block_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_block();
        while !self.at(TokenKind::Eof) {
            self.bump();
        }
        self.bump();
        self.finish_node();
        let tree = self.builder.finish();
        ParsedSyntax::new(self.lexed.file_id(), tree, self.diagnostics)
    }
}

#[cfg(test)]
fn tree_contains(tree: &super::cst::SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(
        tree: &super::cst::SyntaxTree,
        node: super::cst::SyntaxNodeId,
        kind: SyntaxKind,
    ) -> bool {
        tree.node(node).kind() == kind
            || tree.elements(tree.node(node).children()).iter().any(|element| {
                matches!(*element, super::cst::SyntaxElement::Node(child) if walk(tree, child, kind))
            })
    }
    walk(tree, tree.root(), kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::has_errors;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn parse_block_text(text: &str) -> ParsedSyntax {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("block.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        Parser::new(&lexed, &source).parse_block_for_test()
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
