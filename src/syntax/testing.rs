use crate::lexer::{LexedFile, TokenKind};
use crate::source::SourceFile;

use super::cst::{ParsedSyntax, SyntaxElement, SyntaxNodeId, SyntaxTree};
use super::parse::Parser;
use super::syntax_kind::SyntaxKind;

pub fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(tree: &SyntaxTree, node: SyntaxNodeId, kind: SyntaxKind) -> bool {
        tree.node(node).kind() == kind
            || tree.elements(tree.node(node).children()).iter().any(
                |element| matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind)),
            )
    }
    walk(tree, tree.root(), kind)
}

pub fn parse_fragment(
    lexed: &LexedFile,
    source: &SourceFile,
    parse: impl FnOnce(&mut Parser<'_>),
) -> ParsedSyntax {
    let mut parser = Parser::new(lexed, source);
    parser.start_node(SyntaxKind::Module);
    parse(&mut parser);
    while !parser.at(TokenKind::Eof) {
        parser.bump();
    }
    parser.bump();
    parser.finish_node();
    let tree = parser.builder.finish();
    ParsedSyntax::new(parser.lexed.file_id(), tree, parser.diagnostics)
}

pub fn parse_fragment_text(
    text: &str,
    filename: &str,
    parse: impl FnOnce(&mut Parser<'_>),
) -> ParsedSyntax {
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    let source = SourceFile::new(FileId::new(0), PathBuf::from(filename), text.to_string());
    let lexed = lex_file(&source);
    parse_fragment(&lexed, &source, parse)
}
