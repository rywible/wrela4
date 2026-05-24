#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    As,
    Assert,
    Asm,
    Class,
    Constructor,
    Data,
    Drain,
    Else,
    Error,
    Fn,
    For,
    From,
    Host,
    Image,
    Implements,
    Interface,
    Layout,
    Let,
    Loop,
    Match,
    Module,
    Mut,
    Own,
    Phase,
    Pub,
    Read,
    Reduce,
    Repeat,
    Return,
    Same,
    Scan,
    Target,
    Test,
    Trap,
    Try,
    Unique,
    Until,
    Use,
    Value,
    With,
}

impl Keyword {
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "as" => Some(Self::As),
            "assert" => Some(Self::Assert),
            "asm" => Some(Self::Asm),
            "class" => Some(Self::Class),
            "constructor" => Some(Self::Constructor),
            "data" => Some(Self::Data),
            "drain" => Some(Self::Drain),
            "else" => Some(Self::Else),
            "error" => Some(Self::Error),
            "fn" => Some(Self::Fn),
            "for" => Some(Self::For),
            "from" => Some(Self::From),
            "host" => Some(Self::Host),
            "image" => Some(Self::Image),
            "implements" => Some(Self::Implements),
            "interface" => Some(Self::Interface),
            "layout" => Some(Self::Layout),
            "let" => Some(Self::Let),
            "loop" => Some(Self::Loop),
            "match" => Some(Self::Match),
            "module" => Some(Self::Module),
            "mut" => Some(Self::Mut),
            "own" => Some(Self::Own),
            "phase" => Some(Self::Phase),
            "pub" => Some(Self::Pub),
            "read" => Some(Self::Read),
            "reduce" => Some(Self::Reduce),
            "repeat" => Some(Self::Repeat),
            "return" => Some(Self::Return),
            "same" => Some(Self::Same),
            "scan" => Some(Self::Scan),
            "target" => Some(Self::Target),
            "test" => Some(Self::Test),
            "trap" => Some(Self::Trap),
            "try" => Some(Self::Try),
            "unique" => Some(Self::Unique),
            "until" => Some(Self::Until),
            "use" => Some(Self::Use),
            "value" => Some(Self::Value),
            "with" => Some(Self::With),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Punct {
    Amp,
    AmpAmp,
    Arrow,
    Bang,
    BangEq,
    Caret,
    Colon,
    Comma,
    Dot,
    DotDot,
    DotDotEq,
    Eq,
    EqEq,
    FatArrow,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Minus,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Percent,
    Pipe,
    PipePipe,
    Plus,
    Semicolon,
    Slash,
    Star,
    Tilde,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier,
    Keyword(Keyword),
    IntLiteral,
    StringLiteral,
    Punct(Punct),
    Unknown,
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    DocComment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_keywords() {
        assert_eq!(Keyword::from_text("use"), Some(Keyword::Use));
        assert_eq!(Keyword::from_text("class"), Some(Keyword::Class));
        assert_eq!(Keyword::from_text("identifier"), None);
    }

    #[test]
    fn punctuation_debug_is_stable() {
        assert_eq!(format!("{:?}", Punct::Arrow), "Arrow");
        assert_eq!(format!("{:?}", Punct::OpenBrace), "OpenBrace");
    }
}
