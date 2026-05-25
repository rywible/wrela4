use crate::lexer::{Keyword, Punct, TokenKind};

use super::parse::Parser;
use super::syntax_kind::SyntaxKind;

impl<'a> Parser<'a> {
    pub(crate) fn recover_to_statement_boundary(&mut self) {
        if self.at_statement_start()
            || matches!(
                self.peek().kind(),
                TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof
            )
        {
            return;
        }
        self.start_node(SyntaxKind::RecoveryNode);
        while !self.at_statement_start()
            && !matches!(
                self.peek().kind(),
                TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof
            )
        {
            self.bump();
        }
        self.finish_node();
    }

    pub(crate) fn consume_to_item_boundary(&mut self) {
        while !self.at_item_start() && self.peek().kind() != TokenKind::Eof {
            self.bump();
        }
    }

    pub(crate) fn consume_to_member_boundary(&mut self) {
        while !self.at_member_start()
            && !matches!(
                self.peek().kind(),
                TokenKind::Punct(Punct::CloseBrace) | TokenKind::Eof
            )
        {
            self.bump();
        }
    }

    pub(crate) fn at_item_start(&self) -> bool {
        matches!(
            self.peek().kind(),
            TokenKind::Keyword(Keyword::Pub)
                | TokenKind::Keyword(Keyword::Module)
                | TokenKind::Keyword(Keyword::Use)
                | TokenKind::Keyword(Keyword::Data)
                | TokenKind::Keyword(Keyword::Layout)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Unique)
                | TokenKind::Keyword(Keyword::Interface)
                | TokenKind::Keyword(Keyword::Error)
                | TokenKind::Keyword(Keyword::Image)
                | TokenKind::Keyword(Keyword::Host)
        )
    }

    pub(crate) fn at_member_start(&self) -> bool {
        matches!(
            self.peek().kind(),
            TokenKind::Keyword(Keyword::Constructor)
                | TokenKind::Keyword(Keyword::Phase)
                | TokenKind::Keyword(Keyword::Fn)
                | TokenKind::Keyword(Keyword::Asm)
                | TokenKind::Keyword(Keyword::Test)
        ) || (self.peek().kind() == TokenKind::Identifier
            && self.peek_n(1).kind() == TokenKind::Punct(Punct::Colon))
    }

    pub(crate) fn at_statement_start(&self) -> bool {
        matches!(
            self.peek().kind(),
            TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Return)
                | TokenKind::Keyword(Keyword::Match)
                | TokenKind::Keyword(Keyword::Repeat)
                | TokenKind::Keyword(Keyword::For)
                | TokenKind::Keyword(Keyword::Drain)
                | TokenKind::Keyword(Keyword::Loop)
                | TokenKind::Keyword(Keyword::Assert)
        )
    }
}
