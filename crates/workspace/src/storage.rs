//! Utilidades de disco: directorio de datos, escritura atómica y huellas de archivos.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::WorkspaceError;

/// Directorio donde Lorca guarda configuración y sesión:
/// `~/Library/Application Support/Lorca` (macOS), `%APPDATA%\Lorca` (Windows) o
/// `~/.config/lorca` (Linux).
pub fn data_dir() -> Option<PathBuf> {
    let name = if cfg!(any(target_os = "macos", windows)) {
        "Lorca"
    } else {
        "lorca"
    };
    dirs::config_dir().map(|dir| dir.join(name))
}

/// Huella de un archivo en disco (fecha de modificación y tamaño) para detectar cambios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiskStamp {
    pub modified_secs: u64,
    pub modified_nanos: u32,
    pub len: u64,
}

impl DiskStamp {
    /// Huella actual del archivo, o `None` si no existe o no se puede consultar.
    pub fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            modified_secs: modified.as_secs(),
            modified_nanos: modified.subsec_nanos(),
            len: metadata.len(),
        })
    }
}

/// Lee un archivo de texto; `None` si no existe.
pub(crate) fn read_optional(path: &Path) -> Result<Option<String>, WorkspaceError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(WorkspaceError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Escribe en un temporal y lo renombra, para no dejar nunca un archivo a medias.
/// Crea los directorios intermedios si faltan.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), WorkspaceError> {
    let write_error = |source| WorkspaceError::Write {
        path: path.to_owned(),
        source,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(write_error)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, contents).map_err(write_error)?;
    std::fs::rename(&temporary, path).map_err(write_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_changes_when_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        assert_eq!(DiskStamp::of(&path), None);
        std::fs::write(&path, "uno").unwrap();
        let first = DiskStamp::of(&path).unwrap();
        std::fs::write(&path, "uno dos").unwrap();
        assert_ne!(DiskStamp::of(&path), Some(first));
    }

    #[test]
    fn atomic_write_creates_dirs_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x").join("y.toml");
        write_atomic(&path, b"1").unwrap();
        write_atomic(&path, b"2").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "2");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }
}
