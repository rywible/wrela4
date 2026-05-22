use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::lexer::{LexedFile, lex_files_parallel};
use crate::source::{FileId, SourceMap, Span};
use crate::syntax::imports::{ImportSummary, ModulePath, parse_import_summary};

#[derive(Debug)]
pub struct DiscoverResult {
    source_map: SourceMap,
    lexed_files: Vec<LexedFile>,
    import_summaries: Vec<ImportSummary>,
    diagnostics: Vec<Diagnostic>,
}

impl DiscoverResult {
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn lexed_files(&self) -> &[LexedFile] {
        &self.lexed_files
    }

    pub fn import_summaries(&self) -> &[ImportSummary] {
        &self.import_summaries
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

struct ImportLoadRecord {
    importing_file: FileId,
    span: Span,
    target_path: PathBuf,
}

fn try_load_canonical(path: &Path) -> Result<(PathBuf, String), ()> {
    let canonical = path.canonicalize().map_err(|_| ())?;
    let text = fs::read_to_string(&canonical).map_err(|_| ())?;
    Ok((canonical, text))
}

fn resolve_module(source_root: &Path, module: &ModulePath) -> PathBuf {
    let mut path = source_root.to_path_buf();
    for segment in module.segments() {
        path.push(segment);
    }
    path.set_extension("wrela");
    path
}

fn merge_diagnostics(
    source_map: &SourceMap,
    lexed_files: &[LexedFile],
    import_summaries: &[ImportSummary],
    import_load_records: &[ImportLoadRecord],
) -> Vec<Diagnostic> {
    let summary_by_file: BTreeMap<FileId, &ImportSummary> = lexed_files
        .iter()
        .zip(import_summaries.iter())
        .map(|(lexed, summary)| (lexed.file_id(), summary))
        .collect();

    let mut load_by_file: BTreeMap<FileId, Vec<&ImportLoadRecord>> = BTreeMap::new();
    for record in import_load_records {
        load_by_file
            .entry(record.importing_file)
            .or_default()
            .push(record);
    }

    let mut diagnostics = Vec::new();

    for file in source_map.files() {
        let file_id = file.id();

        if let Some(lexed) = lexed_files.iter().find(|lexed| lexed.file_id() == file_id) {
            diagnostics.extend(lexed.diagnostics().iter().cloned());
        }

        if let Some(summary) = summary_by_file.get(&file_id) {
            diagnostics.extend(summary.diagnostics().iter().cloned());
        }

        if let Some(records) = load_by_file.get(&file_id) {
            let mut sorted = records.to_vec();
            sorted.sort_by(|left, right| {
                left.span
                    .start()
                    .cmp(&right.span.start())
                    .then(left.span.end().cmp(&right.span.end()))
                    .then(left.target_path.cmp(&right.target_path))
            });
            for record in sorted {
                diagnostics.push(Diagnostic::error(
                    record.span,
                    "could not load imported file",
                ));
            }
        }
    }

    diagnostics
}

pub fn discover_from_root(root: impl AsRef<Path>) -> DiscoverResult {
    let root_input = root.as_ref();

    let (root_path, root_text) = match try_load_canonical(root_input) {
        Ok(loaded) => loaded,
        Err(()) => {
            return DiscoverResult {
                source_map: SourceMap::new(),
                lexed_files: Vec::new(),
                import_summaries: Vec::new(),
                diagnostics: vec![Diagnostic::unspanned_error("could not load root file")],
            };
        }
    };

    let source_root = root_path.parent().unwrap_or(Path::new(".")).to_path_buf();

    let mut source_map = SourceMap::new();
    let root_id = source_map.add_loaded_file(root_path.clone(), root_text);
    debug_assert_eq!(root_id.raw(), 0);

    let mut path_index: BTreeMap<PathBuf, FileId> = BTreeMap::new();
    path_index.insert(root_path.clone(), root_id);

    let mut lexed_files = Vec::new();
    let mut import_summaries = Vec::new();
    let mut import_load_records = Vec::new();
    let mut pending_imports: BTreeMap<PathBuf, Vec<(FileId, Span)>> = BTreeMap::new();
    let mut lexed_up_to = 0usize;

    let mut batch = vec![root_path];

    while !batch.is_empty() {
        batch.sort();

        for path in &batch {
            if path_index.contains_key(path) {
                continue;
            }

            let import_requests = pending_imports.remove(path).unwrap_or_default();
            match try_load_canonical(path) {
                Ok((canonical, text)) => {
                    let file_id = source_map.add_loaded_file(canonical.clone(), text);
                    path_index.insert(canonical, file_id);
                }
                Err(()) => {
                    for (importing_file, span) in import_requests {
                        import_load_records.push(ImportLoadRecord {
                            importing_file,
                            span,
                            target_path: path.clone(),
                        });
                    }
                }
            }
        }

        let mut files_to_lex = Vec::new();
        for path in &batch {
            if let Some(file_id) = path_index.get(path) {
                let index = file_id.raw() as usize;
                if index >= lexed_up_to {
                    files_to_lex.push(index);
                }
            }
        }
        files_to_lex.sort_unstable();
        files_to_lex.dedup();

        if !files_to_lex.is_empty() {
            let start = files_to_lex[0];
            let end = files_to_lex.last().unwrap() + 1;
            let batch_lexed = lex_files_parallel(&source_map.files()[start..end]);
            lexed_up_to = end;

            for lexed in &batch_lexed {
                let source = source_map
                    .get(lexed.file_id())
                    .expect("lexed file in source map");
                import_summaries.push(parse_import_summary(lexed, source));
            }

            lexed_files.extend(batch_lexed);
        }

        let mut next_paths: BTreeSet<PathBuf> = BTreeSet::new();
        let mut next_pending: BTreeMap<PathBuf, Vec<(FileId, Span)>> = BTreeMap::new();

        let batch_summary_start = import_summaries.len().saturating_sub(files_to_lex.len());
        for (lexed, summary) in lexed_files
            .iter()
            .skip(lexed_files.len().saturating_sub(files_to_lex.len()))
            .zip(import_summaries.iter().skip(batch_summary_start))
        {
            for import in summary.imports() {
                let resolved = resolve_module(&source_root, import.module());
                let key = resolved.canonicalize().unwrap_or(resolved);
                if path_index.contains_key(&key) {
                    continue;
                }
                next_paths.insert(key.clone());
                next_pending
                    .entry(key)
                    .or_default()
                    .push((lexed.file_id(), import.span()));
            }
        }

        batch = next_paths.into_iter().collect();
        pending_imports = next_pending;
    }

    let diagnostics = merge_diagnostics(
        &source_map,
        &lexed_files,
        &import_summaries,
        &import_load_records,
    );

    DiscoverResult {
        source_map,
        lexed_files,
        import_summaries,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_temp_dir(name: &str) -> TestDir {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "wrela-{name}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&dir).unwrap();
        TestDir {
            path: fs::canonicalize(dir).unwrap(),
        }
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn discovers_imported_files_from_root() {
        let dir = unique_temp_dir("discover");
        let root = dir.path().join("root.wrela");
        write(&root, "use { Console } from app.console\nimage Root {}");
        write(&dir.path().join("app/console.wrela"), "class Console {}");

        let result = discover_from_root(&root);
        let paths: Vec<String> = result
            .source_map()
            .files()
            .iter()
            .map(|file| {
                file.path()
                    .strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();

        assert_eq!(paths, vec!["root.wrela", "app/console.wrela"]);
        assert!(result.diagnostics().is_empty());
    }

    #[test]
    fn reports_missing_imported_file() {
        let dir = unique_temp_dir("missing");
        let root = dir.path().join("root.wrela");
        write(&root, "use { Missing } from app.missing\nimage Root {}");

        let result = discover_from_root(&root);

        assert!(
            result
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message() == "could not load imported file" })
        );
    }

    #[test]
    fn reports_missing_root_without_invented_file_id() {
        let dir = unique_temp_dir("missing-root");
        let result = discover_from_root(dir.path().join("missing.wrela"));

        assert_eq!(result.source_map().files().len(), 0);
        assert_eq!(
            result.diagnostics()[0].message(),
            "could not load root file"
        );
        assert!(result.diagnostics()[0].span().is_none());
    }
}
