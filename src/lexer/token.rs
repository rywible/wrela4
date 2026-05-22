use crate::source::Span;

use super::kind::TokenKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    kind: TokenKind,
    span: Span,
}

impl Token {
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub const fn kind(self) -> TokenKind {
        self.kind
    }

    pub const fn span(self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};

    #[test]
    fn token_stores_kind_and_span() {
        let span = Span::new(FileId::new(2), 10, 15);
        let token = Token::new(TokenKind::Identifier, span);

        assert_eq!(token.kind(), TokenKind::Identifier);
        assert_eq!(token.span(), span);
    }
}
