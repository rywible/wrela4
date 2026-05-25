use wrela::syntax::{SyntaxElement, SyntaxKind, SyntaxNodeId, SyntaxTree};

pub fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(tree: &SyntaxTree, node: SyntaxNodeId, kind: SyntaxKind) -> bool {
        if tree.node(node).kind() == kind {
            return true;
        }
        tree.elements(tree.node(node).children()).iter().any(
            |element| matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind)),
        )
    }
    walk(tree, tree.root(), kind)
}
