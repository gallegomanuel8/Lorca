//! Búsqueda en todos los archivos de texto de una carpeta.

use std::io::Read;
use std::path::{Path, PathBuf};

use lorca_core::load_bytes;

use crate::{Hit, SearchQuery, build_hits};

/// Límites para no bloquearse con carpetas enormes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_matches: usize,
}

impl Default for FolderLimits {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 5 * 1024 * 1024,
            max_matches: 10_000,
        }
    }
}

/// Coincidencias de un archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatches {
    pub path: PathBuf,
    pub hits: Vec<Hit>,
}

/// Resultado de buscar en una carpeta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderSearch {
    pub files: Vec<FileMatches>,
    pub files_searched: usize,
    /// Se alcanzó algún límite y puede haber más coincidencias.
    pub truncated: bool,
}

impl FolderSearch {
    pub fn total(&self) -> usize {
        self.files.iter().map(|f| f.hits.len()).sum()
    }
}

/// Busca en los archivos de texto de `root` (recursivamente), respetando `.gitignore` y
/// saltando archivos ocultos, binarios y demasiado grandes. Los archivos se recorren en
/// orden alfabético.
pub fn search_folder(root: &Path, query: &SearchQuery, limits: FolderLimits) -> FolderSearch {
    let mut result = FolderSearch::default();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if result.files_searched >= limits.max_files {
            result.truncated = true;
            break;
        }
        let path = entry.path();
        if entry
            .metadata()
            .map_or(true, |m| m.len() > limits.max_file_bytes)
        {
            continue;
        }
        let Some(text) = read_text(path) else {
            continue;
        };
        result.files_searched += 1;

        let remaining = limits.max_matches.saturating_sub(result.total());
        if remaining == 0 {
            result.truncated = true;
            break;
        }
        let matches = query.find_all(&text, remaining);
        if !matches.is_empty() {
            result.files.push(FileMatches {
                path: path.to_owned(),
                hits: build_hits(&text, &matches),
            });
        }
    }
    result
}

/// Contenido de un archivo de texto (con LF), o `None` si no se puede leer o es binario.
fn read_text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .read_to_end(&mut bytes)
        .ok()?;
    // Un byte nulo al principio es señal casi segura de archivo binario (salvo UTF-16,
    // que se reconoce por su BOM).
    let has_utf16_bom = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    if !has_utf16_bom && bytes.iter().take(8000).any(|b| *b == 0) {
        return None;
    }
    Some(load_bytes(&bytes).text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SearchOptions;

    fn query(pattern: &str) -> SearchQuery {
        SearchQuery::new(pattern, SearchOptions::default()).unwrap()
    }

    #[test]
    fn finds_in_nested_files_in_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("b.txt"), "luna\nsol luna").unwrap();
        std::fs::write(dir.path().join("a.md"), "nada").unwrap();
        std::fs::write(dir.path().join("sub/c.rs"), "// luna").unwrap();

        let result = search_folder(dir.path(), &query("luna"), FolderLimits::default());
        let names: Vec<_> = result
            .files
            .iter()
            .map(|f| {
                // Con «/» en todos los sistemas (Windows separa con «\»).
                f.path
                    .strip_prefix(dir.path())
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        assert_eq!(names, ["b.txt", "sub/c.rs"]);
        assert_eq!(result.total(), 3);
        assert_eq!(result.files_searched, 3);
        assert_eq!(result.files[0].hits[1].line, 1);
    }

    #[test]
    fn respects_gitignore_hidden_and_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "ignorado.txt\n").unwrap();
        std::fs::write(dir.path().join("ignorado.txt"), "luna").unwrap();
        std::fs::write(dir.path().join(".oculto"), "luna").unwrap();
        std::fs::write(dir.path().join("binario.bin"), b"luna\0\0\0").unwrap();
        std::fs::write(dir.path().join("si.txt"), "luna").unwrap();

        let result = search_folder(dir.path(), &query("luna"), FolderLimits::default());
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].path.ends_with("si.txt"));
    }

    #[test]
    fn decodes_other_encodings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("viejo.txt"), b"canci\xF3n").unwrap();
        let result = search_folder(dir.path(), &query("canción"), FolderLimits::default());
        assert_eq!(result.total(), 1);
    }

    #[test]
    fn limits_truncate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x x x x x").unwrap();
        let limits = FolderLimits {
            max_matches: 2,
            ..FolderLimits::default()
        };
        let result = search_folder(dir.path(), &query("x"), limits);
        assert_eq!(result.total(), 2);
    }
}
