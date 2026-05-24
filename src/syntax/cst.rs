use crate::diagnostic::Diagnostic;
use crate::lexer::{LexedFile, TokenKind};
use crate::source::{FileId, SourceFile, Span};

use super::syntax_kind::{SyntaxErrorKind, SyntaxKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxNodeId(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxTokenId(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenIndex(u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriviaRange {
    start: u32,
    end: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementRange {
    start: u32,
    end: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    depth: usize,
    child_index: usize,
}

impl SyntaxNodeId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
}
impl SyntaxTokenId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
}
impl TokenIndex {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
}
impl TriviaRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
    pub const fn start(self) -> u32 {
        self.start
    }
    pub const fn end(self) -> u32 {
        self.end
    }
}
impl ElementRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
    pub const fn start(self) -> u32 {
        self.start
    }
    pub const fn end(self) -> u32 {
        self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxElement {
    Node(SyntaxNodeId),
    Token(SyntaxTokenId),
    Error(SyntaxErrorNode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxErrorNode {
    kind: SyntaxErrorKind,
    span: Span,
}

impl SyntaxErrorNode {
    pub const fn new(kind: SyntaxErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
    pub const fn kind(self) -> SyntaxErrorKind {
        self.kind
    }
    pub const fn span(self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxToken {
    token: TokenIndex,
    span: Span,
    leading_trivia: TriviaRange,
    trailing_trivia: TriviaRange,
}

impl SyntaxToken {
    pub const fn new(
        token: TokenIndex,
        span: Span,
        leading_trivia: TriviaRange,
        trailing_trivia: TriviaRange,
    ) -> Self {
        Self {
            token,
            span,
            leading_trivia,
            trailing_trivia,
        }
    }
    pub const fn token(self) -> TokenIndex {
        self.token
    }
    pub const fn span(self) -> Span {
        self.span
    }
    pub const fn leading_trivia(self) -> TriviaRange {
        self.leading_trivia
    }
    pub const fn trailing_trivia(self) -> TriviaRange {
        self.trailing_trivia
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    kind: SyntaxKind,
    span: Span,
    children: ElementRange,
}

impl SyntaxNode {
    pub const fn new(kind: SyntaxKind, span: Span, children: ElementRange) -> Self {
        Self {
            kind,
            span,
            children,
        }
    }
    pub const fn kind(&self) -> SyntaxKind {
        self.kind
    }
    pub const fn span(&self) -> Span {
        self.span
    }
    pub const fn children(&self) -> ElementRange {
        self.children
    }
}

#[derive(Debug)]
pub struct SyntaxTree {
    file_id: FileId,
    root: SyntaxNodeId,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    tokens: Vec<SyntaxToken>,
}

impl SyntaxTree {
    pub fn new(
        file_id: FileId,
        root: SyntaxNodeId,
        nodes: Vec<SyntaxNode>,
        elements: Vec<SyntaxElement>,
        tokens: Vec<SyntaxToken>,
    ) -> Self {
        Self {
            file_id,
            root,
            nodes,
            elements,
            tokens,
        }
    }
    pub fn file_id(&self) -> FileId {
        self.file_id
    }
    pub fn root(&self) -> SyntaxNodeId {
        self.root
    }
    pub fn node(&self, id: SyntaxNodeId) -> &SyntaxNode {
        &self.nodes[id.raw() as usize]
    }
    pub fn elements(&self, range: ElementRange) -> &[SyntaxElement] {
        &self.elements[range.start() as usize..range.end() as usize]
    }
    pub fn token(&self, id: SyntaxTokenId) -> SyntaxToken {
        self.tokens[id.raw() as usize]
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn source_text(&self, lexed: &LexedFile, source: &SourceFile) -> String {
        let mut text = String::new();
        self.push_node_text(self.root, lexed, source, &mut text);
        text
    }

    fn push_node_text(
        &self,
        node: SyntaxNodeId,
        lexed: &LexedFile,
        source: &SourceFile,
        out: &mut String,
    ) {
        for element in self.elements(self.node(node).children()) {
            match *element {
                SyntaxElement::Node(child) => self.push_node_text(child, lexed, source, out),
                SyntaxElement::Token(id) => {
                    let token = self.token(id);
                    push_trivia(out, lexed, source, token.leading_trivia());
                    let raw = lexed.tokens()[token.token().raw() as usize];
                    if raw.kind() != TokenKind::Eof {
                        push_span(out, source, raw.span());
                    }
                    push_trivia(out, lexed, source, token.trailing_trivia());
                }
                SyntaxElement::Error(_) => {}
            }
        }
    }
}

fn push_trivia(out: &mut String, lexed: &LexedFile, source: &SourceFile, range: TriviaRange) {
    for index in range.start()..range.end() {
        push_span(out, source, lexed.trivia()[index as usize].span());
    }
}

fn push_span(out: &mut String, source: &SourceFile, span: Span) {
    out.push_str(&source.text()[span.start() as usize..span.end() as usize]);
}

#[derive(Debug)]
pub struct ParsedSyntax {
    file_id: FileId,
    tree: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
}

impl ParsedSyntax {
    pub fn new(file_id: FileId, tree: SyntaxTree, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            file_id,
            tree,
            diagnostics,
        }
    }
    pub fn file_id(&self) -> FileId {
        self.file_id
    }
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[derive(Debug)]
struct NodeFrame {
    kind: SyntaxKind,
    anchor: Span,
    children: Vec<SyntaxElement>,
}

#[derive(Debug)]
pub struct SyntaxTreeBuilder {
    file_id: FileId,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    tokens: Vec<SyntaxToken>,
    stack: Vec<NodeFrame>,
    root: Option<SyntaxNodeId>,
}

impl SyntaxTreeBuilder {
    pub fn new(file_id: FileId) -> Self {
        Self {
            file_id,
            nodes: Vec::new(),
            elements: Vec::new(),
            tokens: Vec::new(),
            stack: Vec::new(),
            root: None,
        }
    }

    pub fn start_node(&mut self, kind: SyntaxKind, anchor: Span) {
        self.stack.push(NodeFrame {
            kind,
            anchor,
            children: Vec::new(),
        });
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let frame = self.stack.last().expect("checkpoint requires an open node");
        Checkpoint {
            depth: self.stack.len(),
            child_index: frame.children.len(),
        }
    }

    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind, anchor: Span) {
        assert_eq!(
            checkpoint.depth,
            self.stack.len(),
            "checkpoint depth must match current node"
        );
        let parent = self
            .stack
            .last_mut()
            .expect("checkpoint requires parent frame");
        let children = parent.children.split_off(checkpoint.child_index);
        self.stack.push(NodeFrame {
            kind,
            anchor,
            children,
        });
    }

    pub fn token(&mut self, token: SyntaxToken) {
        let id = SyntaxTokenId::new(self.tokens.len() as u32);
        self.tokens.push(token);
        self.push_child(SyntaxElement::Token(id));
    }

    pub fn error(&mut self, kind: SyntaxErrorKind, span: Span) {
        self.push_child(SyntaxElement::Error(SyntaxErrorNode::new(kind, span)));
    }

    pub fn finish_node(&mut self) -> SyntaxNodeId {
        let frame = self.stack.pop().expect("finish_node requires open node");
        let span = self.children_span(&frame.children).unwrap_or(frame.anchor);
        let start = self.elements.len() as u32;
        self.elements.extend(frame.children);
        let end = self.elements.len() as u32;
        let id = SyntaxNodeId::new(self.nodes.len() as u32);
        self.nodes.push(SyntaxNode::new(
            frame.kind,
            span,
            ElementRange::new(start, end),
        ));
        if let Some(parent) = self.stack.last_mut() {
            parent.children.push(SyntaxElement::Node(id));
        } else {
            self.root = Some(id);
        }
        id
    }

    pub fn finish(self) -> SyntaxTree {
        SyntaxTree::new(
            self.file_id,
            self.root.expect("syntax tree root must be finished"),
            self.nodes,
            self.elements,
            self.tokens,
        )
    }

    fn push_child(&mut self, element: SyntaxElement) {
        self.stack
            .last_mut()
            .expect("syntax child requires open node")
            .children
            .push(element);
    }

    fn children_span(&self, children: &[SyntaxElement]) -> Option<Span> {
        let first = children
            .first()
            .and_then(|child| self.element_span(*child))?;
        let last = children
            .last()
            .and_then(|child| self.element_span(*child))?;
        Some(Span::new(self.file_id, first.start(), last.end()))
    }

    fn element_span(&self, element: SyntaxElement) -> Option<Span> {
        match element {
            SyntaxElement::Node(id) => Some(self.nodes[id.raw() as usize].span()),
            SyntaxElement::Token(id) => Some(self.tokens[id.raw() as usize].span()),
            SyntaxElement::Error(error) => Some(error.span()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_wraps_existing_child() {
        let file_id = FileId::new(0);
        let anchor = Span::new(file_id, 0, 0);
        let mut builder = SyntaxTreeBuilder::new(file_id);
        builder.start_node(SyntaxKind::Module, anchor);
        let checkpoint = builder.checkpoint();
        builder.token(SyntaxToken::new(
            TokenIndex::new(0),
            Span::new(file_id, 0, 3),
            TriviaRange::new(0, 0),
            TriviaRange::new(0, 0),
        ));
        builder.start_node_at(checkpoint, SyntaxKind::FieldExpr, anchor);
        let field = builder.finish_node();
        let root = builder.finish_node();
        let tree = builder.finish();

        assert_eq!(tree.node(field).kind(), SyntaxKind::FieldExpr);
        assert!(matches!(
            tree.elements(tree.node(root).children())[0],
            SyntaxElement::Node(id) if id == field
        ));
    }

    #[test]
    fn empty_node_uses_anchor_span() {
        let file_id = FileId::new(0);
        let anchor = Span::new(file_id, 8, 8);
        let mut builder = SyntaxTreeBuilder::new(file_id);
        builder.start_node(SyntaxKind::Module, anchor);
        let root = builder.finish_node();
        let tree = builder.finish();

        assert_eq!(tree.node(root).span(), anchor);
    }

    #[test]
    fn error_element_preserves_order_and_span() {
        let file_id = FileId::new(0);
        let mut builder = SyntaxTreeBuilder::new(file_id);
        builder.start_node(SyntaxKind::Module, Span::new(file_id, 0, 0));
        builder.token(SyntaxToken::new(
            TokenIndex::new(0),
            Span::new(file_id, 0, 1),
            TriviaRange::new(0, 0),
            TriviaRange::new(0, 0),
        ));
        builder.error(
            SyntaxErrorKind::ExpectedIdentifier,
            Span::new(file_id, 1, 1),
        );
        let root = builder.finish_node();
        let tree = builder.finish();
        let children = tree.elements(tree.node(root).children());

        assert!(matches!(children[0], SyntaxElement::Token(_)));
        assert!(matches!(
            children[1],
            SyntaxElement::Error(error) if error.span() == Span::new(file_id, 1, 1)
        ));
    }
}
