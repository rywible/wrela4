use std::path::PathBuf;

mod common;

use common::tree_contains;
use wrela::diagnostic::has_errors;
use wrela::discover::discover_from_root;
use wrela::lexer::lex_file;
use wrela::source::{FileId, SourceFile, SourceMap};
use wrela::syntax::{
    ElementRange, ParsedSyntax, SyntaxElement, SyntaxKind, SyntaxTree, parse_file,
    parse_files_parallel, parse_import_summary, summarize_module,
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
fn parses_top_level_declaration_forms() {
    let parsed = parse_fixture("declarations-top.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::PublicItem,
        SyntaxKind::DataDecl,
        SyntaxKind::LayoutDataDecl,
        SyntaxKind::InterfaceDecl,
        SyntaxKind::ErrorDecl,
        SyntaxKind::ImageDecl,
        SyntaxKind::HostImageDecl,
        SyntaxKind::PhaseDecl,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
}

#[test]
fn parses_member_declaration_forms() {
    let parsed = parse_fixture("declarations-members.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::ClassDecl,
        SyntaxKind::UniqueClassDecl,
        SyntaxKind::ImplementsClause,
        SyntaxKind::FieldDecl,
        SyntaxKind::ConstructorDecl,
        SyntaxKind::MethodDecl,
        SyntaxKind::TestDecl,
        SyntaxKind::Block,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
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

#[test]
fn parses_control_statement_forms() {
    let parsed = parse_fixture("statements-control.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    for kind in [
        SyntaxKind::MatchStmt,
        SyntaxKind::MatchArm,
        SyntaxKind::RepeatStmt,
        SyntaxKind::ForStmt,
        SyntaxKind::DrainStmt,
        SyntaxKind::LoopStmt,
    ] {
        assert!(tree_contains(parsed.tree(), kind), "missing {kind:?}");
    }
}

#[test]
fn parses_assertions_reduce_and_scan() {
    let parsed = parse_fixture("assertions-reduce-scan.wrela");

    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ReduceExpr));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ScanExpr));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::AssertStmt), 2);
}

#[test]
fn recovers_after_bad_statement_and_parses_following_items() {
    let parsed = parse_fixture("recovery.wrela");

    assert!(has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::RecoveryNode));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::ClassDecl), 2);
    assert!(count_nodes(parsed.tree(), SyntaxKind::MethodDecl) >= 3);
    assert_eq!(
        parsed.diagnostics()[0].message(),
        "expected type annotation or initializer in let statement"
    );
}

#[test]
fn parses_files_parallel_in_file_id_order() {
    let mut source_map = SourceMap::new();
    source_map
        .load_file(parser_fixture("declarations-members.wrela"))
        .unwrap();
    source_map
        .load_file(parser_fixture("declarations-top.wrela"))
        .unwrap();
    let lexed_files: Vec<_> = source_map.files().iter().map(lex_file).collect();

    let parsed = parse_files_parallel(&lexed_files, &source_map);
    let ids: Vec<u32> = parsed.iter().map(|parsed| parsed.file_id().raw()).collect();

    assert_eq!(ids, vec![0, 1]);
}

#[test]
fn summarizes_module_counts_top_level_items() {
    let parsed = parse_fixture("declarations-top.wrela");
    let summary = summarize_module(&parsed);

    assert!(summary.item_count() >= 6);
    assert!(summary.node_count() > 0);
    assert!(summary.token_count() > 0);
}

#[test]
fn parse_command_reports_reachable_files() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "parse".to_string(),
            parser_fixture("imports/root.wrela")
                .to_string_lossy()
                .into_owned(),
        ],
        &mut out,
        &mut err,
    );

    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(out.lines().any(|line| line.contains("nodes=")
        && line.contains("tokens=")
        && line.contains("items=")));
}

#[test]
fn recovers_malformed_field_list_without_hanging() {
    let parsed = parse_text("data Foo { , }");
    assert!(has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::RecoveryNode));
}

#[test]
fn recovers_malformed_match_arm_list_without_hanging() {
    let parsed = parse_text("class C { fn m() { match x { , } } }");
    assert!(has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::RecoveryNode));
}

#[test]
fn image_body_recovery_preserves_following_top_level_item() {
    let parsed = parse_text("image App target T { + } class After {}");
    assert!(has_errors(parsed.diagnostics()));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::ClassDecl), 1);
    assert!(tree_contains(parsed.tree(), SyntaxKind::ImageDecl));
}

#[test]
fn reduce_and_scan_accept_full_expression_operands() {
    let parsed = parse_fixture("assertions-reduce-scan.wrela");
    assert!(!has_errors(parsed.diagnostics()));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ReduceExpr));
    assert!(tree_contains(parsed.tree(), SyntaxKind::ScanExpr));
    assert!(tree_contains(parsed.tree(), SyntaxKind::BinaryExpr));
}

#[test]
fn rejects_reserved_keyword_as_let_binding_name() {
    let parsed = parse_text("class C { fn m() { let return = 1 } }");
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.message() == "expected identifier" })
    );
}

#[test]
fn use_without_from_still_parses_module_path() {
    let parsed = parse_text("use { Foo } app.console\nclass After {}");
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.message() == "expected from in use import" })
    );
    assert!(tree_contains(parsed.tree(), SyntaxKind::ModulePath));
    assert_eq!(count_nodes(parsed.tree(), SyntaxKind::ClassDecl), 1);
}
