use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileId(u32);

impl FileId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    file_id: FileId,
    start: u32,
    end: u32,
}

impl Span {
    pub const fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self { file_id, start, end }
    }

    pub const fn file_id(self) -> FileId {
        self.file_id
    }

    pub const fn start(self) -> u32 {
        self.start
    }

    pub const fn end(self) -> u32 {
        self.end
    }

    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(id: FileId, path: PathBuf, text: String) -> Self {
        let line_starts = compute_line_starts(&text);
        Self { id, path, text, line_starts }
    }

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_starts(&self) -> &[u32] {
        &self.line_starts
    }

    pub fn span(&self) -> Span {
        Span::new(self.id, 0, self.text.len() as u32)
    }
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add_loaded_file(&mut self, path: PathBuf, text: String) -> FileId {
        let id = FileId::new(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, text));
        id
    }

    pub fn load_file(&mut self, path: impl AsRef<Path>) -> io::Result<FileId> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(self.add_loaded_file(path.to_path_buf(), text))
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.raw() as usize)
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }
}

fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0];
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                starts.push((index + 1) as u32);
                index += 1;
            }
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                starts.push((index + 2) as u32);
                index += 2;
            }
            b'\r' => {
                starts.push((index + 1) as u32);
                index += 1;
            }
            _ => index += 1,
        }
    }

    starts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn source_file_computes_line_starts() {
        let file = SourceFile::new(FileId::new(7), PathBuf::from("sample.wrela"), "one\rtwo\nthree\r\nfour".to_string());

        assert_eq!(file.line_starts(), &[0, 4, 8, 15]);
    }

    #[test]
    fn span_len_is_byte_based() {
        let span = Span::new(FileId::new(1), 2, 9);

        assert_eq!(span.len(), 7);
        assert!(!span.is_empty());
    }

    #[test]
    fn source_map_assigns_stable_ids() {
        let mut map = SourceMap::new();
        let first = map.add_loaded_file(PathBuf::from("a.wrela"), "a".to_string());
        let second = map.add_loaded_file(PathBuf::from("b.wrela"), "b".to_string());

        assert_eq!(first, FileId::new(0));
        assert_eq!(second, FileId::new(1));
        assert_eq!(map.get(first).unwrap().text(), "a");
        assert_eq!(map.get(second).unwrap().text(), "b");
    }
}
