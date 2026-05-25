use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

impl<'a> Parser<'a> {
    pub(crate) fn parse_type_ref(&mut self) {
        self.start_node(SyntaxKind::TypeRef);
        if matches!(
            self.peek().kind(),
            TokenKind::Keyword(Keyword::Read)
                | TokenKind::Keyword(Keyword::Mut)
                | TokenKind::Keyword(Keyword::Own)
                | TokenKind::Keyword(Keyword::Unique)
        ) {
            self.bump();
        }
        if self.peek().kind() == TokenKind::Identifier {
            self.bump();
            while self.eat_punct(Punct::Dot) {
                self.expect_identifier();
            }
            if self.eat_punct(Punct::OpenBracket) {
                self.start_node(SyntaxKind::GenericArgList);
                if self.peek().kind() != TokenKind::Punct(Punct::CloseBracket) {
                    self.parse_generic_arg();
                    while self.eat_punct(Punct::Comma) {
                        if self.peek().kind() == TokenKind::Punct(Punct::CloseBracket) {
                            break;
                        }
                        self.parse_generic_arg();
                    }
                }
                self.finish_node();
                self.expect_close_punct(Punct::CloseBracket, "expected ']'");
            }
        } else {
            self.error_at_current(SyntaxErrorKind::ExpectedType, "expected type");
        }
        self.finish_node();
    }

    pub(crate) fn parse_generic_param_list(&mut self) {
        if !self.eat_punct(Punct::Less) {
            return;
        }
        self.start_node(SyntaxKind::GenericParamList);
        self.parse_generic_param();
        while self.eat_punct(Punct::Comma) {
            if self.peek().kind() == TokenKind::Punct(Punct::Greater) {
                break;
            }
            self.parse_generic_param();
        }
        self.finish_node();
        self.expect_punct(Punct::Greater, "expected '>'");
    }

    fn parse_generic_param(&mut self) {
        self.start_node(SyntaxKind::GenericParam);
        self.expect_identifier();
        if self.eat_punct(Punct::Colon) {
            self.parse_type_ref();
        }
        self.finish_node();
    }

    fn parse_generic_arg(&mut self) {
        if self.peek().kind() == TokenKind::IntLiteral {
            self.bump();
        } else {
            self.parse_type_ref();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::has_errors;
    use crate::syntax::ParsedSyntax;
    use crate::syntax::testing::parse_fragment_text;

    fn parse_type_text(text: &str) -> ParsedSyntax {
        parse_fragment_text(text, "type.wrela", |parser| parser.parse_type_ref())
    }

    #[test]
    fn parses_generic_type_arguments() {
        let parsed = parse_type_text("Table[Session, 4096]");
        assert!(!has_errors(parsed.diagnostics()));
    }

    #[test]
    fn parses_access_qualified_type() {
        let parsed = parse_type_text("unique MacOSHost");
        assert!(!has_errors(parsed.diagnostics()));
    }

    #[test]
    fn missing_type_name_emits_expected_type() {
        let parsed = parse_type_text("=");
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message() == "expected type" })
        );
    }
}
