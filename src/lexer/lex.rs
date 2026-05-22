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
            } else if byte == b'"' {
                self.scan_string(start);
            } else if byte.is_ascii_digit() {
                self.scan_integer(start);
            } else if is_ident_start(byte) {
                self.scan_identifier(start);
            } else if byte == b'/'
                && matches!(self.bytes.get(self.cursor + 1), Some(&b'/') | Some(&b'*'))
            {
                if self.bytes[self.cursor + 1] == b'/' {
                    self.scan_line_comment(start);
                } else {
                    self.scan_block_comment(start);
                }
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

    fn scan_integer(&mut self, start: usize) {
        if self.bytes[start] == b'0' && self.bytes.get(start + 1) == Some(&b'x') {
            self.cursor = start + 2;
            if !self
                .bytes
                .get(self.cursor)
                .is_some_and(|&byte| is_hex_digit(byte))
            {
                self.diagnostics.push(Diagnostic::error(
                    self.span(start, self.cursor),
                    "hex literal requires digits",
                ));
            }
            while self.bytes.get(self.cursor).is_some_and(|&byte| {
                is_hex_digit(byte) || byte == b'_'
            }) {
                self.cursor += 1;
            }
        } else {
            self.cursor = start + 1;
            while self.bytes.get(self.cursor).is_some_and(|&byte| {
                byte.is_ascii_digit() || byte == b'_'
            }) {
                self.cursor += 1;
            }
        }

        if self.cursor > start && self.bytes[self.cursor - 1] == b'_' {
            self.diagnostics.push(Diagnostic::error(
                self.span(start, self.cursor),
                "numeric literal cannot end with underscore",
            ));
        }

        self.push_token(TokenKind::IntLiteral, start, self.cursor);
    }

    fn scan_string(&mut self, start: usize) {
        debug_assert_eq!(self.bytes[start], b'"');
        self.cursor = start + 1;
        let mut closed = false;

        while self.cursor < self.bytes.len() {
            let byte = self.bytes[self.cursor];
            if byte == b'"' {
                self.cursor += 1;
                closed = true;
                break;
            }
            if is_newline_start(byte) {
                break;
            }
            if byte == b'\\' {
                let escape_start = self.cursor;
                self.cursor += 1;
                if self.cursor >= self.bytes.len() {
                    break;
                }
                let escaped = self.bytes[self.cursor];
                if matches!(
                    escaped,
                    b'\\' | b'"' | b'n' | b'r' | b't' | b'0'
                ) {
                    self.cursor += 1;
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        self.span(escape_start, self.cursor + 1),
                        "invalid string escape",
                    ));
                    self.cursor += 1;
                }
                continue;
            }
            self.cursor += 1;
        }

        if !closed {
            self.diagnostics.push(Diagnostic::error(
                self.span(start, self.cursor),
                "unterminated string literal",
            ));
        }

        self.push_token(TokenKind::StringLiteral, start, self.cursor);
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

    fn scan_line_comment(&mut self, start: usize) {
        let kind = if self.bytes.get(start + 2) == Some(&b'!') {
            TriviaKind::DocComment
        } else if self.bytes.get(start + 2) == Some(&b'/') {
            if self.bytes.get(start + 3) == Some(&b'/') {
                TriviaKind::LineComment
            } else {
                TriviaKind::DocComment
            }
        } else {
            TriviaKind::LineComment
        };

        self.cursor = start + 2;
        while self.cursor < self.bytes.len() && !is_newline_start(self.bytes[self.cursor]) {
            self.cursor += 1;
        }
        self.push_trivia(kind, start, self.cursor);
    }

    fn scan_block_comment(&mut self, start: usize) {
        let kind = if self.bytes.get(start + 2) == Some(&b'!') {
            TriviaKind::DocComment
        } else if self.bytes.get(start + 2) == Some(&b'*') {
            if self.bytes.get(start + 3) == Some(&b'*') {
                TriviaKind::BlockComment
            } else {
                TriviaKind::DocComment
            }
        } else {
            TriviaKind::BlockComment
        };

        self.cursor = start + 2;
        let mut depth = 1;
        while self.cursor < self.bytes.len() && depth > 0 {
            if self.bytes[self.cursor] == b'/'
                && self.bytes.get(self.cursor + 1) == Some(&b'*')
            {
                self.cursor += 2;
                depth += 1;
            } else if self.bytes[self.cursor] == b'*'
                && self.bytes.get(self.cursor + 1) == Some(&b'/')
            {
                self.cursor += 2;
                depth -= 1;
            } else {
                self.cursor += 1;
            }
        }

        let end = self.cursor;
        if depth > 0 {
            self.diagnostics.push(Diagnostic::error(
                self.span(start, end),
                "unterminated block comment",
            ));
        }
        self.push_trivia(kind, start, end);
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

fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F')
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
    use crate::lexer::{Keyword, Punct, TokenKind, TriviaKind};
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

    #[test]
    fn preserves_whitespace_newlines_and_comments_as_trivia() {
        let source = file("use  // hello\nfrom");
        let lexed = lex_file(&source);
        let trivia_kinds: Vec<TriviaKind> =
            lexed.trivia().iter().map(|trivia| trivia.kind()).collect();
        let token_kinds: Vec<TokenKind> =
            lexed.tokens().iter().map(|token| token.kind()).collect();

        assert_eq!(
            token_kinds,
            vec![
                TokenKind::Keyword(Keyword::Use),
                TokenKind::Keyword(Keyword::From),
                TokenKind::Eof,
            ]
        );
        assert_eq!(
            trivia_kinds,
            vec![
                TriviaKind::Whitespace,
                TriviaKind::LineComment,
                TriviaKind::Newline,
            ]
        );
    }

    #[test]
    fn preserves_doc_comments_as_distinct_trivia() {
        let source = file("/// docs\nclass Thing {}");
        let lexed = lex_file(&source);
        let trivia_kinds: Vec<TriviaKind> =
            lexed.trivia().iter().map(|trivia| trivia.kind()).collect();

        assert_eq!(trivia_kinds[0], TriviaKind::DocComment);
    }

    #[test]
    fn nested_block_comments_are_one_trivia_item() {
        let source = file("/* outer /* inner */ done */class Thing {}");
        let lexed = lex_file(&source);

        assert_eq!(lexed.trivia()[0].kind(), TriviaKind::BlockComment);
        assert!(lexed.diagnostics().is_empty());
    }

    #[test]
    fn comment_edge_cases_have_stable_kinds() {
        let source = file("//// not docs\n/**** also not docs */\n//! docs");
        let lexed = lex_file(&source);
        let trivia_kinds: Vec<TriviaKind> =
            lexed.trivia().iter().map(|trivia| trivia.kind()).collect();

        assert_eq!(
            trivia_kinds,
            vec![
                TriviaKind::LineComment,
                TriviaKind::Newline,
                TriviaKind::BlockComment,
                TriviaKind::Newline,
                TriviaKind::DocComment,
            ]
        );
    }

    #[test]
    fn lexes_string_and_integer_literals() {
        let source = file("let answer = 42\nlet path = \"disk\"");
        let lexed = lex_file(&source);
        let kinds: Vec<TokenKind> = lexed.tokens().iter().map(|token| token.kind()).collect();

        assert!(kinds.contains(&TokenKind::IntLiteral));
        assert!(kinds.contains(&TokenKind::StringLiteral));
        assert!(lexed.diagnostics().is_empty());
    }

    #[test]
    fn reports_unterminated_string_and_continues() {
        let source = file("let name = \"unterminated\nclass Next {}");
        let lexed = lex_file(&source);

        assert!(lexed
            .tokens()
            .iter()
            .any(|token| token.kind() == TokenKind::StringLiteral));
        assert_eq!(
            lexed.diagnostics()[0].message(),
            "unterminated string literal"
        );
        assert!(lexed.tokens().iter().any(|token| {
            token.kind() == TokenKind::Keyword(Keyword::Class)
        }));
    }

    #[test]
    fn reports_invalid_escape() {
        let source = file("\"bad\\q\"");
        let lexed = lex_file(&source);

        assert_eq!(lexed.diagnostics()[0].message(), "invalid string escape");
    }

    #[test]
    fn reports_trailing_numeric_underscore() {
        let source = file("123_");
        let lexed = lex_file(&source);

        assert_eq!(
            lexed.diagnostics()[0].message(),
            "numeric literal cannot end with underscore"
        );
    }
}
