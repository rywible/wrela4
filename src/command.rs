use std::path::Path;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::discover::discover_from_root;
use crate::lexer::{LexedFile, Token, Trivia, lex_file};
use crate::source::{FileId, SourceFile, SourceMap, Span};
use crate::syntax::{parse_files_parallel, summarize_module};

fn write_help_banner<W: std::io::Write>(out: &mut W) -> std::io::Result<()> {
    writeln!(out, "wrela commands: help, version")?;
    writeln!(out, "wrela lex <root.wrela>")?;
    writeln!(out, "wrela parse <root.wrela>")?;
    writeln!(out, "wrela check [--json|--human] <root.wrela>")?;
    writeln!(out, "wrela dump tokens <file.wrela>")?;
    writeln!(out, "wrela dump mir <root.wrela>")?;
    writeln!(out, "wrela dump asm <root.wrela>")?;
    writeln!(
        out,
        "wrela debug mir [--why-rewrite N] [--enable-pass NAME|--disable-pass NAME|--only-pass NAME] <root.wrela>"
    )?;
    writeln!(
        out,
        "wrela perf compile [--mode dev|release] [--repeat N] [--json|--human] <root.wrela>"
    )?;
    writeln!(
        out,
        "wrela perf code [--mode dev|release] [--repeat N] [--enable-pass NAME|--disable-pass NAME|--only-pass NAME] <root.wrela>"
    )?;
    writeln!(out, "wrela perf compare [--repeat N] <root.wrela>")?;
    Ok(())
}

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
        Some("help") => {
            if collected.len() > 2 {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
            let _ = write_help_banner(out);
            0
        }
        None => {
            let _ = write_help_banner(out);
            0
        }
        Some("version") => {
            if collected.len() > 2 {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
            let _ = writeln!(out, "wrela 0.1.0");
            0
        }
        Some("dump") => match collected.get(2).map(String::as_str) {
            Some("tokens") => match collected.get(3) {
                Some(path) => {
                    if collected.len() > 4 {
                        let _ = writeln!(err, "malformed command");
                        2
                    } else {
                        dump_tokens(path, out, err)
                    }
                }
                None => {
                    let _ = writeln!(err, "missing file path");
                    2
                }
            },
            Some("mir") => match collected.get(3) {
                Some(path) => {
                    if collected.len() > 4 {
                        let _ = writeln!(err, "malformed command");
                        2
                    } else {
                        dump_mir(path, out, err)
                    }
                }
                None => {
                    let _ = writeln!(err, "missing root file path");
                    2
                }
            },
            Some("asm") => match collected.get(3) {
                Some(path) => {
                    if collected.len() > 4 {
                        let _ = writeln!(err, "malformed command");
                        2
                    } else {
                        dump_asm(path, out, err)
                    }
                }
                None => {
                    let _ = writeln!(err, "missing root file path");
                    2
                }
            },
            _ => {
                let _ = writeln!(err, "malformed command");
                2
            }
        },
        Some("perf") => run_perf_command(&collected, out, err),
        Some("lex") => match collected.get(2) {
            Some(path) => {
                if collected.len() > 3 {
                    let _ = writeln!(err, "malformed command");
                    2
                } else {
                    lex_root(path, out, err)
                }
            }
            None => {
                let _ = writeln!(err, "missing root file path");
                2
            }
        },
        Some("parse") => match collected.get(2) {
            Some(path) => {
                if collected.len() > 3 {
                    let _ = writeln!(err, "malformed command");
                    2
                } else {
                    parse_root(path, out, err)
                }
            }
            None => {
                let _ = writeln!(err, "missing root file path");
                2
            }
        },
        Some("check") => run_check_command(&collected, out, err),
        Some("debug") => match collected.get(2).map(String::as_str) {
            Some("mir") => run_debug_mir(&collected, out, err),
            _ => {
                let _ = writeln!(err, "malformed command");
                2
            }
        },
        Some(other) => {
            let _ = writeln!(err, "unknown command: {other}");
            2
        }
    }
}

fn dump_asm<W, E>(path: &str, out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let check = crate::check::check_root(path);
    if !check.ok() {
        print_diagnostics(out, check.diagnostics());
        return 1;
    }
    let mir = crate::mir::build_mir(&check);
    if !mir.ok() {
        print_diagnostics(out, mir.diagnostics());
        return 1;
    }
    let lir = crate::mir::lower_to_lir(mir.module().expect("ok MIR has module"));
    let lir_check = crate::mir::verify_lir(&lir);
    if !lir_check.ok() {
        for message in lir_check.messages() {
            let _ = writeln!(err, "{message}");
        }
        return 1;
    }
    let allocated = match crate::mir::regalloc::allocate_registers(&lir) {
        Ok(allocated) => allocated,
        Err(error) => {
            let _ = writeln!(err, "{}", error.message());
            return 1;
        }
    };
    let asm = crate::mir::emit::emit_aarch64(&allocated);
    let _ = out.write_all(asm.as_bytes());
    0
}

struct PerfCompileArgs {
    path: String,
    mode: crate::mir::perf::PerfMode,
    repeats: usize,
    json: bool,
}

struct PerfCodeArgs {
    path: String,
    mode: crate::mir::perf::PerfMode,
    repeats: usize,
    pass_set: crate::mir::PassSet,
}

fn run_debug_mir<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let mut rewrite_index: Option<usize> = None;
    let mut pass_set = crate::mir::PassSet::release_default();
    let mut path = None;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--why-rewrite" => {
                let Some(value) = args.get(index + 1) else {
                    let _ = writeln!(err, "missing value for --why-rewrite");
                    return 2;
                };
                rewrite_index = value
                    .parse()
                    .map_err(|_| {
                        let _ = writeln!(err, "invalid --why-rewrite value");
                    })
                    .ok();
                if rewrite_index.is_none() {
                    return 2;
                }
                index += 2;
            }
            "--only-pass" => {
                let Some(name) = args.get(index + 1) else {
                    let _ = writeln!(err, "missing value for --only-pass");
                    return 2;
                };
                let Some(pass) = crate::mir::Pass::from_name(name) else {
                    let _ = writeln!(err, "unknown release pass: {name}");
                    return 2;
                };
                pass_set = crate::mir::PassSet::only(pass);
                index += 2;
            }
            flag if flag.starts_with("--") => {
                let _ = writeln!(err, "unknown flag: {flag}");
                return 2;
            }
            value => {
                if path.is_some() {
                    let _ = writeln!(err, "malformed command");
                    return 2;
                }
                path = Some(value.to_string());
                index += 1;
            }
        }
    }
    let Some(path) = path else {
        let _ = writeln!(err, "missing root file path");
        return 2;
    };
    let Some(rewrite_index) = rewrite_index else {
        let _ = writeln!(err, "missing --why-rewrite index");
        return 2;
    };

    let check = crate::check::check_root(&path);
    if !check.ok() {
        print_diagnostics(out, check.diagnostics());
        return 1;
    }
    let mir = crate::mir::build_mir(&check);
    if !mir.ok() {
        print_diagnostics(out, mir.diagnostics());
        return 1;
    }
    let optimized =
        crate::mir::optimize_release(mir.module().expect("ok MIR has module"), &pass_set);
    let Some(event): Option<&crate::mir::RewriteEvent> =
        optimized.report().rewrite_events().get(rewrite_index)
    else {
        let _ = writeln!(err, "rewrite event index out of range");
        return 2;
    };
    let cert = event.certificate();
    let _ = writeln!(out, "rule: {}", cert.rule().name());
    let _ = writeln!(out, "pass: {}", cert.pass().name());
    let _ = writeln!(out, "before: {}", cert.before_hash());
    let _ = writeln!(out, "after: {}", cert.after_hash());
    for fact in cert.facts() {
        let _ = writeln!(out, "fact: {fact:?}");
    }
    0
}

fn run_perf_command<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    match args.get(2).map(String::as_str) {
        Some("compile") => run_perf_compile(args, out, err),
        Some("code") => run_perf_code(args, out, err),
        Some("compare") => run_perf_compare(args, out, err),
        _ => {
            let _ = writeln!(err, "malformed command");
            2
        }
    }
}

fn run_perf_compile<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let parsed = match parse_perf_compile_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let report = match crate::mir::perf::measure_compile(&parsed.path, parsed.mode, parsed.repeats)
    {
        Ok(report) => report,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    let rendered = if parsed.json {
        crate::mir::perf::render_compile_json(&report)
    } else {
        crate::mir::perf::render_compile_human(&report)
    };
    let _ = out.write_all(rendered.as_bytes());
    0
}

fn run_perf_compare<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let mut repeats = 7usize;
    let mut path = None;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--repeat" => {
                let Some(value) = args.get(index + 1) else {
                    let _ = writeln!(err, "missing value for --repeat");
                    return 2;
                };
                repeats = match value.parse::<usize>() {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        let _ = writeln!(err, "invalid --repeat value");
                        return 2;
                    }
                };
                index += 2;
            }
            "--json" => {
                let _ = writeln!(err, "perf compare always emits JSON; omit --json");
                return 2;
            }
            flag if flag.starts_with("--") => {
                let _ = writeln!(err, "unknown flag: {flag}");
                return 2;
            }
            value => {
                if path.is_some() {
                    let _ = writeln!(err, "malformed command");
                    return 2;
                }
                path = Some(value.to_string());
                index += 1;
            }
        }
    }
    let Some(path) = path else {
        let _ = writeln!(err, "missing root file path");
        return 2;
    };
    let report = match crate::mir::perf::compare_code(&path, repeats) {
        Ok(report) => report,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            if message.contains("generated-code execution is not supported") {
                return 2;
            }
            return 1;
        }
    };
    let rendered = crate::mir::perf::render_compare_json(&report);
    let _ = out.write_all(rendered.as_bytes());
    0
}

fn run_perf_code<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let parsed = match parse_perf_code_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let report = match crate::mir::perf::measure_code(
        &parsed.path,
        parsed.mode,
        parsed.repeats,
        &parsed.pass_set,
    ) {
        Ok(report) => report,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            if message.contains("generated-code execution is not supported")
                || message.contains("data-plane generated-code execution is MIR 05 work")
            {
                return 2;
            }
            return 1;
        }
    };
    let rendered = crate::mir::perf::render_code_json(&report);
    let _ = out.write_all(rendered.as_bytes());
    0
}

fn parse_perf_compile_args(args: &[String]) -> Result<PerfCompileArgs, String> {
    let mut mode = crate::mir::perf::PerfMode::Dev;
    let mut repeats = 1usize;
    let mut json = false;
    let mut path = None;

    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--mode" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing value for --mode".to_string());
                };
                mode = match value.as_str() {
                    "dev" => crate::mir::perf::PerfMode::Dev,
                    "release" => crate::mir::perf::PerfMode::Release,
                    other => return Err(format!("unknown mode: {other}")),
                };
                index += 2;
            }
            "--repeat" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing value for --repeat".to_string());
                };
                repeats = value
                    .parse()
                    .map_err(|_| "invalid --repeat value".to_string())?;
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag: {flag}"));
            }
            value => {
                if path.is_some() {
                    return Err("malformed command".to_string());
                }
                path = Some(value.to_string());
                index += 1;
            }
        }
    }

    let Some(path) = path else {
        return Err("missing root file path".to_string());
    };

    Ok(PerfCompileArgs {
        path,
        mode,
        repeats,
        json,
    })
}

fn parse_perf_code_args(args: &[String]) -> Result<PerfCodeArgs, String> {
    let mut mode = crate::mir::perf::PerfMode::Dev;
    let mut repeats = 1usize;
    let mut path = None;
    let mut pass_set = crate::mir::PassSet::release_default();

    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--mode" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing value for --mode".to_string());
                };
                mode = match value.as_str() {
                    "dev" => crate::mir::perf::PerfMode::Dev,
                    "release" => crate::mir::perf::PerfMode::Release,
                    other => return Err(format!("unknown mode: {other}")),
                };
                index += 2;
            }
            "--repeat" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing value for --repeat".to_string());
                };
                repeats = value
                    .parse()
                    .map_err(|_| "invalid --repeat value".to_string())?;
                index += 2;
            }
            "--json" => {
                return Err("perf code always emits JSON; omit --json".to_string());
            }
            "--enable-pass" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("missing value for --enable-pass".to_string());
                };
                let Some(pass) = crate::mir::Pass::from_name(name) else {
                    return Err(format!("unknown release pass: {name}"));
                };
                pass_set = pass_set.with_enabled(pass);
                index += 2;
            }
            "--disable-pass" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("missing value for --disable-pass".to_string());
                };
                let Some(pass) = crate::mir::Pass::from_name(name) else {
                    return Err(format!("unknown release pass: {name}"));
                };
                pass_set = pass_set.with_disabled(pass);
                index += 2;
            }
            "--only-pass" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("missing value for --only-pass".to_string());
                };
                let Some(pass) = crate::mir::Pass::from_name(name) else {
                    return Err(format!("unknown release pass: {name}"));
                };
                pass_set = crate::mir::PassSet::only(pass);
                index += 2;
            }
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag: {flag}"));
            }
            value => {
                if path.is_some() {
                    return Err("malformed command".to_string());
                }
                path = Some(value.to_string());
                index += 1;
            }
        }
    }

    let Some(path) = path else {
        return Err("missing root file path".to_string());
    };

    Ok(PerfCodeArgs {
        path,
        mode,
        repeats,
        pass_set,
    })
}

fn dump_mir<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let check = crate::check::check_root(path);
    if !check.ok() {
        print_diagnostics(out, check.diagnostics());
        return 1;
    }

    let mir = crate::mir::build_mir(&check);
    if !mir.ok() {
        print_diagnostics(out, mir.diagnostics());
        return 1;
    }

    let module = mir.module().expect("ok MIR result has module");
    let text = crate::mir::text::render_module(module);
    let _ = out.write_all(text.as_bytes());
    0
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

    if has_errors(diagnostics) { 1 } else { 0 }
}

fn parse_root<W, E>(path: &str, out: &mut W, _err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let result = discover_from_root(path);
    let source_map = result.source_map();
    let discovery_diagnostics = result.diagnostics();
    let parsed = parse_files_parallel(result.lexed_files(), source_map);

    let source_root = source_map
        .files()
        .first()
        .and_then(|file| file.path().parent());

    for parsed_file in &parsed {
        let source = source_map
            .get(parsed_file.file_id())
            .expect("parsed file has source");
        let summary = summarize_module(parsed_file);
        let diagnostic_count = diagnostics_for_file(discovery_diagnostics, parsed_file.file_id())
            + parsed_file.diagnostics().len();
        let path = format_file_path(source_root, source);

        let _ = writeln!(
            out,
            "file {} {} nodes={} tokens={} items={} diagnostics={}",
            parsed_file.file_id().raw(),
            path,
            summary.node_count(),
            summary.token_count(),
            summary.item_count(),
            diagnostic_count
        );
    }

    print_diagnostics(out, discovery_diagnostics);
    for parsed_file in &parsed {
        print_diagnostics(out, parsed_file.diagnostics());
    }

    let has_parser_errors = parsed
        .iter()
        .any(|parsed_file| has_errors(parsed_file.diagnostics()));
    if has_errors(discovery_diagnostics) || has_parser_errors {
        1
    } else {
        0
    }
}

fn run_check_command<W, E>(args: &[String], out: &mut W, err: &mut E) -> i32
where
    W: std::io::Write,
    E: std::io::Write,
{
    let mut format = crate::check::CheckFormat::Json;
    let mut saw_format_flag = false;
    let mut root: Option<&str> = None;

    for arg in &args[2..] {
        match arg.as_str() {
            "--json" if !saw_format_flag && root.is_none() => {
                format = crate::check::CheckFormat::Json;
                saw_format_flag = true;
            }
            "--human" if !saw_format_flag && root.is_none() => {
                format = crate::check::CheckFormat::Human;
                saw_format_flag = true;
            }
            "--json" | "--human" => {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
            value if value.starts_with("--") => {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
            value if root.is_none() => root = Some(value),
            _ => {
                let _ = writeln!(err, "malformed command");
                return 2;
            }
        }
    }

    let Some(root) = root else {
        let _ = writeln!(err, "missing root file path");
        return 2;
    };

    let result = crate::check::check_root(root);
    match format {
        crate::check::CheckFormat::Json => {
            let _ = writeln!(out, "{}", crate::check::json::render_check_json(&result));
        }
        crate::check::CheckFormat::Human => {
            let _ = write!(
                out,
                "{}",
                crate::diagnostic::render_diagnostics(result.diagnostics(), result.source_map())
            );
        }
    }

    if result.ok() { 0 } else { 1 }
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
        let code = run_with_io(
            vec!["wrela".to_string(), "help".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("wrela lex <root.wrela>"));
        assert!(out.contains("wrela parse <root.wrela>"));
        assert!(out.contains("wrela dump tokens <file.wrela>"));
        assert!(out.contains("wrela dump mir <root.wrela>"));
        assert!(err.is_empty());
    }

    #[test]
    fn help_lists_asm_dump_command() {
        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = run_with_io(
            vec!["wrela".to_string(), "help".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("wrela dump asm <root.wrela>")
        );
    }

    #[test]
    fn help_lists_mir_dump_command() {
        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = run_with_io(
            vec!["wrela".to_string(), "help".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("wrela dump mir <root.wrela>")
        );
    }

    #[test]
    fn unknown_command_exits_two() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(
            vec!["wrela".to_string(), "wat".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 2);
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("unknown command: wat")
        );
    }

    #[test]
    fn surplus_lex_args_exit_two() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(
            vec![
                "wrela".to_string(),
                "lex".to_string(),
                "root.wrela".to_string(),
                "extra".to_string(),
            ],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 2);
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("malformed command")
        );
    }

    #[test]
    fn surplus_parse_args_exit_two() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(
            vec![
                "wrela".to_string(),
                "parse".to_string(),
                "root.wrela".to_string(),
                "extra".to_string(),
            ],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 2);
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("malformed command")
        );
        assert!(out.is_empty());
    }

    #[test]
    fn help_lists_debug_mir_and_perf_compare() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(
            vec!["wrela".to_string(), "help".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("wrela debug mir"));
        assert!(out.contains("wrela perf compare"));
        assert!(out.contains("--only-pass"));
        assert!(err.is_empty());
    }

    #[test]
    fn help_lists_parse_command() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_with_io(
            vec!["wrela".to_string(), "help".to_string()],
            &mut out,
            &mut err,
        );

        assert_eq!(code, 0);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("wrela parse <root.wrela>")
        );
        assert!(err.is_empty());
    }
}
