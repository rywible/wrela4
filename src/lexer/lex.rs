use crate::diagnostic::Diagnostic;
use crate::source::{FileId, SourceFile, Span};

use super::kind::{Keyword, Punct, TokenKind, TriviaKind};
use super::token::Token;
use super::trivia::Trivia;

#[derive(Debug)]
pub struct LexedFile {
    file_id: FileId,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
}

impl LexedFile {
    pub fn new(
        file_id: FileId,
        tokens: Vec<Token>,
        trivia: Vec<Trivia>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            file_id,
            tokens,
            trivia,
            diagnostics,
        }
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn trivia(&self) -> &[Trivia] {
        &self.trivia
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub fn lex_file(source: &SourceFile) -> LexedFile {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a SourceFile,
    bytes: &'a [u8],
    cursor: usize,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            bytes: source.text().as_bytes(),
            cursor: 0,
            tokens: Vec::new(),
            trivia: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> LexedFile {
        while self.cursor < self.bytes.len() {
            let start = self.cursor;
            let byte = self.bytes[self.cursor];

            if is_whitespace(byte) {
                self.scan_whitespace(start);
            } else if is_newline_start(byte) {
                self.scan_newline(start);
            } else if is_ident_start(byte) {
                self.scan_identifier(start);
            } else if let Some(punct) = self.scan_punctuation() {
                let end = self.cursor;
                self.push_token(TokenKind::Punct(punct), start, end);
            } else if byte.is_ascii() {
                self.cursor += 1;
                self.push_unknown(start, self.cursor);
            } else {
                let len = next_char_len(self.source.text(), start);
                self.cursor = start + len;
                self.push_unknown(start, self.cursor);
            }
        }

        let end = self.bytes.len();
        self.push_token(TokenKind::Eof, end, end);

        LexedFile::new(
            self.source.id(),
            self.tokens,
            self.trivia,
            self.diagnostics,
        )
    }

    fn file_id(&self) -> FileId {
        self.source.id()
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.file_id(), start as u32, end as u32)
    }

    fn push_token(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token::new(kind, self.span(start, end)));
    }

    fn push_trivia(&mut self, kind: TriviaKind, start: usize, end: usize) {
        self.trivia.push(Trivia::new(kind, self.span(start, end)));
    }

    fn push_unknown(&mut self, start: usize, end: usize) {
        let span = self.span(start, end);
        self.diagnostics
            .push(Diagnostic::error(span, "unknown character"));
        self.tokens.push(Token::new(TokenKind::Unknown, span));
    }

    fn scan_whitespace(&mut self, start: usize) {
        while self.cursor < self.bytes.len() && is_whitespace(self.bytes[self.cursor]) {
            self.cursor += 1;
        }
        self.push_trivia(TriviaKind::Whitespace, start, self.cursor);
    }

    fn scan_newline(&mut self, start: usize) {
        if self.bytes[self.cursor] == b'\r'
            && self.bytes.get(self.cursor + 1) == Some(&b'\n')
        {
            self.cursor += 2;
        } else {
            self.cursor += 1;
        }
        self.push_trivia(TriviaKind::Newline, start, self.cursor);
    }

    fn scan_identifier(&mut self, start: usize) {
        while self.cursor < self.bytes.len() && is_ident_continue(self.bytes[self.cursor]) {
            self.cursor += 1;
        }
        let text = &self.source.text()[start..self.cursor];
        let kind = match Keyword::from_text(text) {
            Some(keyword) => TokenKind::Keyword(keyword),
            None => TokenKind::Identifier,
        };
        self.push_token(kind, start, self.cursor);
    }

    fn scan_punctuation(&mut self) -> Option<Punct> {
        let start = self.cursor;
        let remaining = &self.bytes[start..];

        let (punct, len) = match remaining {
            [b'-', b'>', ..] => (Punct::Arrow, 2),
            [b'=', b'>', ..] => (Punct::FatArrow, 2),
            [b'=', b'=', ..] => (Punct::EqEq, 2),
            [b'!', b'=', ..] => (Punct::BangEq, 2),
            [b'<', b'=', ..] => (Punct::LessEq, 2),
            [b'>', b'=', ..] => (Punct::GreaterEq, 2),
            [b'&', b'&', ..] => (Punct::AmpAmp, 2),
            [b'|', b'|', ..] => (Punct::PipePipe, 2),
            [b'{', ..] => (Punct::OpenBrace, 1),
            [b'}', ..] => (Punct::CloseBrace, 1),
            [b'[', ..] => (Punct::OpenBracket, 1),
            [b']', ..] => (Punct::CloseBracket, 1),
            [b'(', ..] => (Punct::OpenParen, 1),
            [b')', ..] => (Punct::CloseParen, 1),
            [b',', ..] => (Punct::Comma, 1),
            [b'.', ..] => (Punct::Dot, 1),
            [b':', ..] => (Punct::Colon, 1),
            [b';', ..] => (Punct::Semicolon, 1),
            [b'+', ..] => (Punct::Plus, 1),
            [b'-', ..] => (Punct::Minus, 1),
            [b'*', ..] => (Punct::Star, 1),
            [b'/', ..] => (Punct::Slash, 1),
            [b'%', ..] => (Punct::Percent, 1),
            [b'=', ..] => (Punct::Eq, 1),
            [b'!', ..] => (Punct::Bang, 1),
            [b'<', ..] => (Punct::Less, 1),
            [b'>', ..] => (Punct::Greater, 1),
            [b'&', ..] => (Punct::Amp, 1),
            [b'|', ..] => (Punct::Pipe, 1),
            [b'^', ..] => (Punct::Caret, 1),
            [b'~', ..] => (Punct::Tilde, 1),
            _ => return None,
        };

        self.cursor = start + len;
        Some(punct)
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\x0B' | b'\x0C')
}

fn is_newline_start(byte: u8) -> bool {
    matches!(byte, b'\n' | b'\r')
}

fn next_char_len(text: &str, cursor: usize) -> usize {
    text[cursor..].chars().next().unwrap().len_utf8()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{Keyword, Punct, TokenKind};
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn file(text: &str) -> SourceFile {
        SourceFile::new(FileId::new(0), PathBuf::from("test.wrela"), text.to_string())
    }

    #[test]
    fn lexes_keywords_identifiers_and_punctuation() {
        let source = file("use { RingBufferTests } from tests.ring_buffer");
        let lexed = lex_file(&source);
        let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword(Keyword::Use),
                TokenKind::Punct(Punct::OpenBrace),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::CloseBrace),
                TokenKind::Keyword(Keyword::From),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::Dot),
                TokenKind::Identifier,
                TokenKind::Eof,
            ]
        );
        assert!(lexed.diagnostics().is_empty());
    }

    #[test]
    fn reports_unknown_characters_and_continues() {
        let source = file("let @ name");
        let lexed = lex_file(&source);
        let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

        assert!(kinds.contains(&TokenKind::Unknown));
        assert_eq!(lexed.diagnostics().len(), 1);
        assert_eq!(lexed.diagnostics()[0].message(), "unknown character");
    }

    #[test]
    fn non_ascii_unknown_spans_cover_the_full_codepoint() {
        let source = file("let café");
        let lexed = lex_file(&source);
        let unknown = lexed
            .tokens()
            .iter()
            .find(|token| token.kind() == TokenKind::Unknown)
            .unwrap();
        let span = unknown.span();

        assert_eq!(
            &source.text()[span.start() as usize..span.end() as usize],
            "é"
        );
        assert_eq!(lexed.diagnostics().len(), 1);
    }
}
