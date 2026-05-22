use std::path::PathBuf;

use wrela::diagnostic::has_errors;
use wrela::discover::discover_from_root;
use wrela::lexer::{lex_file, TokenKind, TriviaKind};
use wrela::source::{FileId, SourceFile};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/lexer").join(path)
}

#[test]
fn basic_fixture_lexes_without_errors() {
    let path = fixture("basic.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);

    assert!(!has_errors(lexed.diagnostics()));
    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::StringLiteral));
}

#[test]
fn comments_fixture_preserves_doc_and_block_comment_trivia() {
    let path = fixture("comments.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);
    let trivia: Vec<TriviaKind> = lexed.trivia().iter().map(|item| item.kind()).collect();

    assert!(trivia.contains(&TriviaKind::DocComment));
    assert!(trivia.contains(&TriviaKind::BlockComment));
}

#[test]
fn errors_fixture_reports_recoverable_lexer_errors() {
    let path = fixture("errors.wrela");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(FileId::new(0), path, text);
    let lexed = lex_file(&source);

    assert!(has_errors(lexed.diagnostics()));
    assert!(lexed.tokens().iter().any(|token| token.kind() == TokenKind::Keyword(wrela::lexer::Keyword::Let)));
}

#[test]
fn root_discovery_reaches_imported_files() {
    let result = discover_from_root(fixture("imports/root.wrela"));
    let files: Vec<String> = result
        .source_map()
        .files()
        .iter()
        .map(|file| file.path().file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert_eq!(files, vec!["root.wrela", "console.wrela", "storage.wrela"]);
    assert!(!has_errors(result.diagnostics()));
}
