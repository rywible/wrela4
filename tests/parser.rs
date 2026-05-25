use std::path::PathBuf;

use wrela::diagnostic::has_errors;
use wrela::discover::discover_from_root;
use wrela::lexer::lex_file;
use wrela::source::{FileId, SourceFile};
use wrela::syntax::{
    ElementRange, ParsedSyntax, SyntaxElement, SyntaxKind, SyntaxTree, parse_file,
    parse_import_summary,
};

fn parser_fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/parser")
        .join(rel)
}

fn source_from_text(name: &str, text: &str) -> SourceFile {
    SourceFile::new(FileId::new(0), PathBuf::from(name), text.to_string())
}

fn parse_text(text: &str) -> ParsedSyntax {
    let source = source_from_text("inline.wrela", text);
    let lexed = lex_file(&source);
    parse_file(&lexed, &source)
}

fn parse_fixture(rel: &str) -> ParsedSyntax {
    let path = parser_fixture(rel);
    let text = std::fs::read_to_string(&path).expect("parser fixture is readable");
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);
    parse_file(&lexed, &source)
}

fn tree_contains(tree: &SyntaxTree, kind: SyntaxKind) -> bool {
    fn walk(tree: &SyntaxTree, node: wrela::syntax::SyntaxNodeId, kind: SyntaxKind) -> bool {
        if tree.node(node).kind() == kind {
            return true;
        }
        tree.elements(tree.node(node).children()).iter().any(
            |element| matches!(*element, SyntaxElement::Node(child) if walk(tree, child, kind)),
        )
    }
    walk(tree, tree.root(), kind)
}

fn count_nodes(tree: &SyntaxTree, kind: SyntaxKind) -> usize {
    fn walk(tree: &SyntaxTree, node: wrela::syntax::SyntaxNodeId, kind: SyntaxKind) -> usize {
        let self_count = usize::from(tree.node(node).kind() == kind);
        self_count
            + tree
                .elements(tree.node(node).children())
                .iter()
                .map(|element| match *element {
                    SyntaxElement::Node(child) => walk(tree, child, kind),
                    _ => 0,
                })
                .sum::<usize>()
    }
    walk(tree, tree.root(), kind)
}

fn child_kinds(tree: &SyntaxTree, range: ElementRange) -> Vec<SyntaxKind> {
    tree.elements(range)
        .iter()
        .filter_map(|element| match *element {
            SyntaxElement::Node(id) => Some(tree.node(id).kind()),
            _ => None,
        })
        .collect()
}

#[test]
fn parses_imports_after_discovery() {
    let result = discover_from_root(parser_fixture("imports/root.wrela"));
    assert!(!has_errors(result.diagnostics()));
    let source = result.source_map().files().first().unwrap();
    let lexed = result.lexed_files().first().unwrap();
    let parsed = parse_file(lexed, source);

    assert!(!has_errors(parsed.diagnostics()));
    assert_eq!(parsed.tree().source_text(lexed, source), source.text());
}

#[test]
fn rejects_import_aliases_and_wildcards_in_v1() {
    for (text, expected) in [
        (
            "use { Console as Terminal } from app.console\n",
            "import aliases are not supported in v1",
        ),
        (
            "use { * } from app.console\n",
            "wildcard imports are not supported in v1",
        ),
    ] {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("bad_import.wrela"),
            text.to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message() == expected)
        );
    }
}

#[test]
fn cst_import_path_agrees_with_discovery_import_parser() {
    let text = "use { Console } from app.console\n";
    let source = SourceFile::new(
        FileId::new(0),
        PathBuf::from("root.wrela"),
        text.to_string(),
    );
    let lexed = lex_file(&source);
    let summary = parse_import_summary(&lexed, &source);
    let parsed = parse_file(&lexed, &source);

    assert_eq!(summary.imports()[0].module().as_dotted(), "app.console");
    assert!(!has_errors(parsed.diagnostics()));
}

#[test]
fn parser_harness_smoke_parses_empty_fixture() {
    let parsed = parse_fixture("parser_harness_smoke.wrela");
    let inline = parse_text("");

    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::Module));
    assert_eq!(count_nodes(inline.tree(), SyntaxKind::Module), 1);
    let root = inline.tree().node(inline.tree().root());
    assert!(child_kinds(inline.tree(), root.children()).is_empty());
}
