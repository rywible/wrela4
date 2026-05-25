use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

impl<'a> Parser<'a> {
    pub(crate) fn parse_block(&mut self) {
        self.start_node(SyntaxKind::Block);
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        while self.before_close(Punct::CloseBrace) {
            let index_before = self.token_index;
            self.parse_stmt();
            if self.token_index == index_before {
                self.recover_to_statement_boundary();
            }
        }
        self.expect_close_punct(Punct::CloseBrace, "expected '}'");
        self.finish_node();
    }

    pub(crate) fn parse_stmt(&mut self) {
        match self.peek().kind() {
            TokenKind::Keyword(Keyword::Let) => self.parse_let_stmt(),
            TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt(),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_stmt(),
            TokenKind::Keyword(Keyword::Repeat) => self.parse_repeat_stmt(),
            TokenKind::Keyword(Keyword::For) => self.parse_for_stmt(),
            TokenKind::Keyword(Keyword::Drain) => self.parse_drain_stmt(),
            TokenKind::Keyword(Keyword::Loop) => self.parse_loop_stmt(),
            TokenKind::Keyword(Keyword::Assert) => self.parse_assert_stmt(),
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

    fn parse_match_stmt(&mut self) {
        self.start_node(SyntaxKind::MatchStmt);
        self.bump();
        self.parse_expr();
        self.expect_punct(Punct::OpenBrace, "expected '{'");
        self.parse_list_until(Punct::CloseBrace, "expected '}'", |parser| {
            parser.parse_match_arm();
        });
        self.finish_node();
    }

    fn parse_match_arm(&mut self) {
        self.start_node(SyntaxKind::MatchArm);
        self.parse_match_pattern();
        self.expect_punct(Punct::FatArrow, "expected match arm");
        if self.peek().kind() == TokenKind::Punct(Punct::OpenBrace) {
            self.parse_block();
        } else {
            self.parse_stmt();
        }
        self.finish_node();
    }

    fn parse_match_pattern(&mut self) {
        self.start_node(SyntaxKind::MatchPattern);
        match self.peek().kind() {
            TokenKind::Identifier | TokenKind::IntLiteral | TokenKind::StringLiteral => {
                self.bump();
                while self.eat_punct(Punct::Dot) {
                    self.expect_identifier();
                }
                if self.eat_punct(Punct::DotDot) || self.eat_punct(Punct::DotDotEq) {
                    if self.peek().kind() == TokenKind::IntLiteral {
                        self.bump();
                    } else {
                        self.expect_identifier();
                    }
                }
            }
            _ => self.error_at_current(SyntaxErrorKind::ExpectedMatchArm, "expected match arm"),
        }
        self.finish_node();
    }

    fn parse_repeat_stmt(&mut self) {
        self.start_node(SyntaxKind::RepeatStmt);
        self.bump();
        self.parse_expr();
        self.expect_keyword(Keyword::As, "expected as in repeat statement");
        self.expect_binding_name();
        self.parse_block();
        self.finish_node();
    }

    fn parse_for_stmt(&mut self) {
        self.start_node(SyntaxKind::ForStmt);
        self.bump();
        self.expect_binding_name();
        self.expect_contextual_identifier("in", "expected 'in' in for statement");
        self.parse_expr();
        self.parse_block();
        self.finish_node();
    }

    fn parse_drain_stmt(&mut self) {
        self.start_node(SyntaxKind::DrainStmt);
        self.bump();
        self.parse_expr();
        self.expect_keyword(Keyword::As, "expected as in drain statement");
        self.expect_binding_name();
        self.parse_block();
        self.finish_node();
    }

    fn parse_loop_stmt(&mut self) {
        self.start_node(SyntaxKind::LoopStmt);
        self.bump();
        self.parse_block();
        self.finish_node();
    }

    fn parse_assert_stmt(&mut self) {
        self.start_node(SyntaxKind::AssertStmt);
        self.bump();
        if self.eat_keyword(Keyword::Value) || self.eat_keyword(Keyword::Same) {
            self.parse_expr();
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedAssertKind, "expected assert kind");
        }
        self.finish_node();
    }
}
