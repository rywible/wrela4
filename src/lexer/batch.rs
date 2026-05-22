use std::thread;

use crate::source::SourceFile;

use super::lex::{lex_file, LexedFile};

pub fn lex_files_parallel(files: &[SourceFile]) -> Vec<LexedFile> {
    if files.is_empty() {
        return Vec::new();
    }

    let worker_count = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(files.len());
    let chunk_size = files.len().div_ceil(worker_count);

    let mut results = thread::scope(|scope| {
        let mut handles = Vec::new();

        for chunk in files.chunks(chunk_size) {
            handles.push(scope.spawn(move || chunk.iter().map(lex_file).collect::<Vec<_>>()));
        }

        let mut results = Vec::with_capacity(files.len());
        for handle in handles {
            match handle.join() {
                Ok(mut chunk) => results.append(&mut chunk),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }

        results
    });

    results.sort_by_key(|file| file.file_id());
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TokenKind;
    use crate::source::{FileId, SourceFile};
    use std::path::PathBuf;

    fn source(id: u32, text: &str) -> SourceFile {
        SourceFile::new(FileId::new(id), PathBuf::from(format!("{id}.wrela")), text.to_string())
    }

    #[test]
    fn lexes_files_in_deterministic_file_order() {
        let files = vec![
            source(2, "class C {}"),
            source(0, "class A {}"),
            source(1, "class B {}"),
        ];

        let lexed = lex_files_parallel(&files);
        let ids: Vec<u32> = lexed.iter().map(|file| file.file_id().raw()).collect();

        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn lexes_each_file_independently() {
        let files = vec![source(0, "class A {}"), source(1, "@")];
        let lexed = lex_files_parallel(&files);

        assert!(lexed[0].tokens().iter().any(|token| token.kind() == TokenKind::Keyword(crate::lexer::Keyword::Class)));
        assert_eq!(lexed[1].diagnostics().len(), 1);
    }
}
