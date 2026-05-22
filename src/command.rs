use std::path::Path;

use crate::diagnostic::{has_errors, Diagnostic};
use crate::discover::discover_from_root;
use crate::lexer::{lex_file, LexedFile, Token, Trivia};
use crate::source::{FileId, SourceFile, SourceMap, Span};

pub fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    run_with_io(args, &mut out, &mut err)
}

pub fn run_with_io<I, W, E>(args: I, out: &mut W, err: &mut E) -> i32
where
    I: IntoIterator<Item = String>,
    W: std::io::Write,
    E: std::io::Write,
{
    let collected: Vec<String> = args.into_iter().collect();

    match collected.get(1).map(String::as_str) {
        Some("help") | None => {
            let _ = writeln!(out, "wrela commands: help, version");
            let _ = writeln!(out, "wrela lex <root.wrela>");
            let _ = writeln!(out, "wrela dump tokens <file.wrela>");
            0
        }
        Some("version") => {
            let _ = writeln!(out, "wrela 0.1.0");
            0
        }
        Some("dump") => match collected.get(2).map(String::as_str) {
            Some("tokens") => match collected.get(3) {
                Some(path) => dump_tokens(path, out, err),
                None => {
                    let _ = writeln!(err, "missing file path");
                    2
                }
            },
            _ => {
                let _ = writeln!(err, "malformed command");
                2
            }
        },
        Some("lex") => match collected.get(2) {
            Some(path) => lex_root(path, out, err),
            None => {
                let _ = writeln!(err, "missing root file path");
                2
            }
        },
        Some(other) => {
            let _ = writeln!(err, "unknown command: {other}");
            2
        }
    }
}

fn dump_tokens<W, E>(path: &str, out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let mut source_map = SourceMap::new();
    let file_id = match source_map.load_file(path) {
        Ok(file_id) => file_id,
        Err(error) => {
            let _ = writeln!(err, "{error}");
            return 2;
        }
    };

    let source = source_map.get(file_id).expect("loaded file in source map");
    let lexed = lex_file(source);
    print_lexed_items(out, &lexed);
    print_diagnostics(out, lexed.diagnostics());

    if has_errors(lexed.diagnostics()) {
        1
    } else {
        0
    }
}

fn lex_root<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let result = discover_from_root(path);
    let source_map = result.source_map();
    let diagnostics = result.diagnostics();

    let source_root = source_map
        .files()
        .first()
        .and_then(|file| file.path().parent());

    let lexed_by_file: std::collections::BTreeMap<FileId, &LexedFile> = result
        .lexed_files()
        .iter()
        .map(|lexed| (lexed.file_id(), lexed))
        .collect();

    for file in source_map.files() {
        let file_id = file.id();
        let lexed = lexed_by_file.get(&file_id);
        let token_count = lexed.map(|lexed| lexed.tokens().len()).unwrap_or(0);
        let trivia_count = lexed.map(|lexed| lexed.trivia().len()).unwrap_or(0);
        let diagnostic_count = diagnostics_for_file(diagnostics, file_id);
        let path = format_file_path(source_root, file);

        let _ = writeln!(
            out,
            "file {} {} tokens={} trivia={} diagnostics={}",
            file_id.raw(),
            path,
            token_count,
            trivia_count,
            diagnostic_count
        );
    }

    print_diagnostics(out, diagnostics);

    if has_errors(diagnostics) {
        1
    } else {
        0
    }
}

fn print_lexed_items<W>(out: &mut W, lexed: &LexedFile)
where
    W: std::io::Write,
{
    enum LexItem<'a> {
        Token(&'a Token),
        Trivia(&'a Trivia),
    }

    let mut items: Vec<(u32, u8, LexItem<'_>)> = Vec::new();
    for token in lexed.tokens() {
        items.push((span_start(token.span()), 1, LexItem::Token(token)));
    }
    for trivia in lexed.trivia() {
        items.push((span_start(trivia.span()), 0, LexItem::Trivia(trivia)));
    }
    items.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    for (_, _, item) in items {
        match item {
            LexItem::Token(token) => {
                let span = token.span();
                let _ = writeln!(
                    out,
                    "token {:?} {}..{}",
                    token.kind(),
                    span.start(),
                    span.end()
                );
            }
            LexItem::Trivia(trivia) => {
                let span = trivia.span();
                let _ = writeln!(
                    out,
                    "trivia {:?} {}..{}",
                    trivia.kind(),
                    span.start(),
                    span.end()
                );
            }
        }
    }
}

fn print_diagnostics<W>(out: &mut W, diagnostics: &[Diagnostic])
where
    W: std::io::Write,
{
    for diagnostic in diagnostics {
        let _ = writeln!(out, "{}", diagnostic.render_compact());
    }
}

fn diagnostics_for_file(diagnostics: &[Diagnostic], file_id: FileId) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .span()
                .is_some_and(|span| span.file_id() == file_id)
        })
        .count()
}

fn format_file_path(source_root: Option<&Path>, file: &SourceFile) -> String {
    let path = if let Some(source_root) = source_root {
        file.path()
            .strip_prefix(source_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file.path().to_string_lossy().replace('\\', "/"))
    } else {
        file.path().to_string_lossy().replace('\\', "/")
    };

    path.trim_start_matches('/').to_string()
}

const fn span_start(span: Span) -> u32 {
    span.start()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_lexer_commands() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(vec!["wrela".to_string(), "help".to_string()], &mut out, &mut err);

        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("wrela lex <root.wrela>"));
        assert!(out.contains("wrela dump tokens <file.wrela>"));
        assert!(err.is_empty());
    }

    #[test]
    fn unknown_command_exits_two() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(vec!["wrela".to_string(), "wat".to_string()], &mut out, &mut err);

        assert_eq!(code, 2);
        assert!(String::from_utf8(err).unwrap().contains("unknown command: wat"));
    }
}
