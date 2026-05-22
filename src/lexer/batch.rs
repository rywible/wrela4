use crate::source::SourceFile;
use super::lex::{lex_file, LexedFile};

pub fn lex_files_parallel(files: &[SourceFile]) -> Vec<LexedFile> {
    files.iter().map(lex_file).collect()
}
