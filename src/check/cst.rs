use crate::lexer::{LexedFile, TokenKind};
use crate::source::{SourceFile, Span};
use crate::syntax::{SyntaxElement, SyntaxKind, SyntaxNodeId, SyntaxTokenId, SyntaxTree};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenText<'a> {
    text: &'a str,
    span: Span,
}

impl<'a> TokenText<'a> {
    pub const fn new(text: &'a str, span: Span) -> Self {
        Self { text, span }
    }

    pub const fn text(self) -> &'a str {
        self.text
    }

    pub const fn span(self) -> Span {
        self.span
    }
}

pub struct CstView<'a> {
    tree: &'a SyntaxTree,
    lexed: &'a LexedFile,
    source: &'a SourceFile,
}

impl<'a> CstView<'a> {
    pub const fn new(tree: &'a SyntaxTree, lexed: &'a LexedFile, source: &'a SourceFile) -> Self {
        Self {
            tree,
            lexed,
            source,
        }
    }

    pub fn node_kind(&self, node: SyntaxNodeId) -> SyntaxKind {
        self.tree.node(node).kind()
    }

    pub fn node_span(&self, node: SyntaxNodeId) -> Span {
        self.tree.node(node).span()
    }

    pub fn child_nodes(&self, node: SyntaxNodeId) -> Vec<SyntaxNodeId> {
        self.tree
            .elements(self.tree.node(node).children())
            .iter()
            .filter_map(|element| match *element {
                SyntaxElement::Node(child) => Some(child),
                _ => None,
            })
            .collect()
    }

    pub fn child_tokens(&self, node: SyntaxNodeId) -> Vec<SyntaxTokenId> {
        self.tree
            .elements(self.tree.node(node).children())
            .iter()
            .filter_map(|element| match *element {
                SyntaxElement::Token(token) => Some(token),
                _ => None,
            })
            .collect()
    }

    pub fn first_child(&self, node: SyntaxNodeId, kind: SyntaxKind) -> Option<SyntaxNodeId> {
        self.child_nodes(node)
            .into_iter()
            .find(|child| self.node_kind(*child) == kind)
    }

    pub fn first_descendant(&self, node: SyntaxNodeId, kind: SyntaxKind) -> Option<SyntaxNodeId> {
        for child in self.child_nodes(node) {
            if self.node_kind(child) == kind {
                return Some(child);
            }
            if let Some(found) = self.first_descendant(child, kind) {
                return Some(found);
            }
        }
        None
    }

    pub fn token_text(&self, token: SyntaxTokenId) -> TokenText<'a> {
        let syntax_token = self.tree.token(token);
        let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
        let span = raw.span();
        TokenText::new(
            &self.source.text()[span.start() as usize..span.end() as usize],
            span,
        )
    }

    pub fn first_identifier_text(&self, node: SyntaxNodeId) -> Option<TokenText<'a>> {
        for token in self.child_tokens(node) {
            let syntax_token = self.tree.token(token);
            let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
            if raw.kind() == TokenKind::Identifier {
                return Some(self.token_text(token));
            }
        }
        for child in self.child_nodes(node) {
            if let Some(found) = self.first_identifier_text(child) {
                return Some(found);
            }
        }
        None
    }

    pub fn identifiers_in_node(&self, node: SyntaxNodeId) -> Vec<TokenText<'a>> {
        let mut identifiers = Vec::new();
        self.push_identifiers(node, &mut identifiers);
        identifiers
    }

    fn push_identifiers(&self, node: SyntaxNodeId, out: &mut Vec<TokenText<'a>>) {
        for token in self.child_tokens(node) {
            let syntax_token = self.tree.token(token);
            let raw = self.lexed.tokens()[syntax_token.token().raw() as usize];
            if raw.kind() == TokenKind::Identifier {
                out.push(self.token_text(token));
            }
        }
        for child in self.child_nodes(node) {
            self.push_identifiers(child, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use crate::syntax::{SyntaxKind, parse_file};
    use std::path::PathBuf;

    #[test]
    fn finds_child_nodes_and_identifier_text() {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            "module root\npub data Bytes { value: U32 }\n".to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        let view = CstView::new(parsed.tree(), &lexed, &source);

        let module = parsed.tree().root();
        let data = view
            .child_nodes(module)
            .into_iter()
            .find(|node| view.node_kind(*node) == SyntaxKind::PublicItem)
            .and_then(|public| view.first_descendant(public, SyntaxKind::DataDecl))
            .unwrap();

        assert_eq!(view.first_identifier_text(data).unwrap().text(), "Bytes");
        assert!(view.child_nodes(data).iter().any(|node| {
            view.node_kind(*node) == SyntaxKind::GenericParamList
                || view.node_kind(*node) == SyntaxKind::FieldDecl
        }));
    }
}
