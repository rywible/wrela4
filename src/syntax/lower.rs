use super::cst::{ParsedSyntax, SyntaxElement};
use super::syntax_kind::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleSummary {
    item_count: usize,
    node_count: usize,
    token_count: usize,
}

impl ModuleSummary {
    pub const fn new(item_count: usize, node_count: usize, token_count: usize) -> Self {
        Self {
            item_count,
            node_count,
            token_count,
        }
    }

    pub const fn item_count(self) -> usize {
        self.item_count
    }

    pub const fn node_count(self) -> usize {
        self.node_count
    }

    pub const fn token_count(self) -> usize {
        self.token_count
    }
}

pub fn summarize_module(parsed: &ParsedSyntax) -> ModuleSummary {
    let tree = parsed.tree();
    let root = tree.node(tree.root());
    let item_count = tree
        .elements(root.children())
        .iter()
        .filter(|element| {
            matches!(*element, SyntaxElement::Node(id) if is_top_level_item(tree.node(*id).kind()))
        })
        .count();
    ModuleSummary::new(item_count, tree.node_count(), tree.token_count())
}

fn is_top_level_item(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PublicItem
            | SyntaxKind::ModuleDecl
            | SyntaxKind::UseDecl
            | SyntaxKind::DataDecl
            | SyntaxKind::LayoutDataDecl
            | SyntaxKind::ClassDecl
            | SyntaxKind::UniqueClassDecl
            | SyntaxKind::InterfaceDecl
            | SyntaxKind::ErrorDecl
            | SyntaxKind::ImageDecl
            | SyntaxKind::HostImageDecl
    )
}
