use crate::lexer::{Keyword, Punct, TokenKind};

#[cfg(test)]
use super::cst::ParsedSyntax;
use super::parse::Parser;
use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

#[allow(dead_code)]
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
        if self.expect_identifier() {
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
            let span = self.peek().span();
            self.diagnostic(span, "expected type");
            self.builder.error(SyntaxErrorKind::ExpectedType, span);
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
impl<'a> Parser<'a> {
    pub(crate) fn parse_type_for_test(mut self) -> ParsedSyntax {
        self.start_node(SyntaxKind::Module);
        self.parse_type_ref();
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

    fn parse_type_text(text: &str) -> ParsedSyntax {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("type.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        Parser::new(&lexed, &source).parse_type_for_test()
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
}
