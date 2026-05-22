use crate::source::Span;

use super::kind::TriviaKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Trivia {
    kind: TriviaKind,
    span: Span,
}

impl Trivia {
    pub const fn new(kind: TriviaKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub const fn kind(self) -> TriviaKind {
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
    fn trivia_stores_kind_and_span() {
        let span = Span::new(FileId::new(2), 0, 4);
        let trivia = Trivia::new(TriviaKind::Whitespace, span);

        assert_eq!(trivia.kind(), TriviaKind::Whitespace);
        assert_eq!(trivia.span(), span);
    }
}
