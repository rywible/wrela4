use crate::lexer::{Keyword, Punct, TokenKind};

use super::cst::Checkpoint;
#[cfg(test)]
use super::cst::{ParsedSyntax, SyntaxElement, SyntaxNodeId, SyntaxTree};
use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[allow(dead_code)]
pub(crate) enum BindingPower {
    Lowest = 0,
    Or = 1,
    And = 2,
    Compare = 3,
    Add = 4,
    Mul = 5,
    Prefix = 6,
    Postfix = 7,
}

#[allow(dead_code)]
impl<'a> Parser<'a> {
    pub(crate) fn parse_expr(&mut self) {
        self.parse_expr_bp(BindingPower::Lowest);
    }

    pub(crate) fn parse_expr_bp(&mut self, min_bp: BindingPower) {
        let checkpoint = self.checkpoint();
        self.parse_prefix_or_atom();
        loop {
            if self.parse_postfix(checkpoint) {
                continue;
            }
            let Some((left_bp, right_bp)) = infix_binding_power(self.peek().kind()) else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.start_node_at(checkpoint, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_expr_bp(right_bp);
            self.finish_node();
        }
    }

    fn parse_prefix_or_atom(&mut self) {
        match self.peek().kind() {
            TokenKind::Identifier => self.parse_name_expr(),
            TokenKind::IntLiteral | TokenKind::StringLiteral => self.parse_literal_expr(),
            TokenKind::Punct(Punct::OpenParen) => self.parse_paren_expr(),
            TokenKind::Punct(Punct::Bang)
            | TokenKind::Punct(Punct::Minus)
            | TokenKind::Keyword(Keyword::Read)
            | TokenKind::Keyword(Keyword::Mut)
            | TokenKind::Keyword(Keyword::Own) => self.parse_prefix_expr(),
            TokenKind::Keyword(Keyword::Try) => self.parse_try_expr(),
            _ => self.error_at_current(SyntaxErrorKind::ExpectedExpression, "expected expression"),
        }
    }

    fn parse_name_expr(&mut self) {
        self.start_node(SyntaxKind::NameExpr);
        self.bump();
        self.finish_node();
    }

    fn parse_literal_expr(&mut self) {
        self.start_node(SyntaxKind::LiteralExpr);
        self.bump();
        self.finish_node();
    }

    fn parse_prefix_expr(&mut self) {
        self.start_node(SyntaxKind::PrefixExpr);
        self.bump();
        self.parse_expr_bp(BindingPower::Prefix);
        self.finish_node();
    }

    fn parse_paren_expr(&mut self) {
        self.start_node(SyntaxKind::ParenExpr);
        self.bump();
        self.parse_expr();
        self.expect_close_punct(Punct::CloseParen, "expected ')'");
        self.finish_node();
    }

    fn parse_try_expr(&mut self) {
        self.start_node(SyntaxKind::TryExpr);
        self.bump();
        self.parse_expr_bp(BindingPower::Prefix);
        if self.eat_keyword(Keyword::Else) {
            if self.eat_keyword(Keyword::Return) {
                self.parse_expr();
            } else {
                self.error_at_current(
                    SyntaxErrorKind::ExpectedReturnAfterElse,
                    "expected return after else in try expression",
                );
            }
        }
        self.finish_node();
    }

    fn parse_postfix(&mut self, checkpoint: Checkpoint) -> bool {
        match self.peek().kind() {
            TokenKind::Punct(Punct::OpenParen) => {
                self.start_node_at(checkpoint, SyntaxKind::CallExpr);
                self.bump();
                self.parse_arg_list();
                self.expect_close_punct(Punct::CloseParen, "expected ')'");
                self.finish_node();
                true
            }
            TokenKind::Punct(Punct::Dot) => {
                self.start_node_at(checkpoint, SyntaxKind::FieldExpr);
                self.bump();
                self.expect_identifier();
                self.finish_node();
                true
            }
            TokenKind::Punct(Punct::OpenBracket) => {
                self.start_node_at(checkpoint, SyntaxKind::IndexExpr);
                self.bump();
                self.parse_expr();
                self.expect_close_punct(Punct::CloseBracket, "expected ']'");
                self.finish_node();
                true
            }
            _ => false,
        }
    }

    fn parse_arg_list(&mut self) {
        self.start_node(SyntaxKind::ArgList);
        while self.peek().kind() != TokenKind::Punct(Punct::CloseParen)
            && self.peek().kind() != TokenKind::Eof
        {
            if self.peek().kind() == TokenKind::Identifier
                && self.peek_n(1).kind() == TokenKind::Punct(Punct::Eq)
            {
                self.start_node(SyntaxKind::NamedArg);
                self.bump();
                self.bump();
                self.parse_expr();
                self.finish_node();
            } else {
                self.start_node(SyntaxKind::Arg);
                self.parse_expr();
                self.finish_node();
            }
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        self.finish_node();
    }
}

#[allow(dead_code)]
fn infix_binding_power(kind: TokenKind) -> Option<(BindingPower, BindingPower)> {
    match kind {
        TokenKind::Punct(Punct::PipePipe) => Some((BindingPower::Or, BindingPower::And)),
        TokenKind::Punct(Punct::AmpAmp) => Some((BindingPower::And, BindingPower::Compare)),
        TokenKind::Punct(Punct::EqEq)
        | TokenKind::Punct(Punct::BangEq)
        | TokenKind::Punct(Punct::Less)
        | TokenKind::Punct(Punct::LessEq)
        | TokenKind::Punct(Punct::Greater)
        | TokenKind::Punct(Punct::GreaterEq) => Some((BindingPower::Compare, BindingPower::Add)),
        TokenKind::Punct(Punct::Plus) | TokenKind::Punct(Punct::Minus) => {
            Some((BindingPower::Add, BindingPower::Mul))
        }
        TokenKind::Punct(Punct::Star)
        | TokenKind::Punct(Punct::Slash)
        | TokenKind::Punct(Punct::Percent) => Some((BindingPower::Mul, BindingPower::Prefix)),
        _ => None,
    }
}

#[cfg(test)]
impl<'a> Parser<'a> {
    pub(crate) fn parse_expr_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_expr();
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
mod tests {
    use super::*;
    use crate::diagnostic::has_errors;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn parse_expr_text(text: &str) -> ParsedSyntax {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("expr.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        Parser::new(&lexed, &source).parse_expr_for_test()
    }

    #[test]
    fn postfix_wraps_receiver_with_checkpoints() {
        let parsed = parse_expr_text("foo.bar(x)");

        assert!(!has_errors(parsed.diagnostics()));
        assert!(parsed.tree().node_count() > 0);
        assert!(tree_contains(parsed.tree(), SyntaxKind::CallExpr));
        assert!(tree_contains(parsed.tree(), SyntaxKind::FieldExpr));
    }

    #[test]
    fn named_arg_requires_bare_identifier() {
        let parsed = parse_expr_text("f(name = 1, a.b)");
        assert!(!has_errors(parsed.diagnostics()));
        assert!(tree_contains(parsed.tree(), SyntaxKind::NamedArg));
        assert!(tree_contains(parsed.tree(), SyntaxKind::Arg));
    }

    #[test]
    fn try_else_requires_return() {
        let parsed = parse_expr_text("try load() else recover()");
        assert!(parsed.diagnostics().iter().any(
            |diagnostic| diagnostic.message() == "expected return after else in try expression"
        ));
    }

    fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
        fn walk(tree: &SyntaxTree, node: SyntaxNodeId, kind: SyntaxKind) -> bool {
            tree.node(node).kind() == kind
                || tree
                    .elements(tree.node(node).children())
                    .iter()
                    .any(|element| {
                        matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind))
                    })
        }
        walk(tree, tree.root(), kind)
    }
}
