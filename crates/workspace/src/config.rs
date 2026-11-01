//! Configuración del usuario (`config.toml`).

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::WorkspaceError;
use crate::storage::{read_optional, write_atomic};

/// Modo de color de la interfaz.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    /// Sigue el modo claro/oscuro del sistema.
    #[default]
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Light, Self::Dark];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automático",
            Self::Light => "Claro",
            Self::Dark => "Oscuro",
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Preferencias persistentes. Los campos ausentes del archivo toman su valor por defecto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Tema elegido. `None` significa que el usuario aún no ha elegido (primer arranque).
    pub theme: Option<ThemeMode>,
    /// Mostrar espacios (·) y tabuladores (→).
    pub show_whitespace: bool,
    /// Mostrar los finales de línea (LF / CRLF).
    pub show_line_endings: bool,
    /// Mostrar guías verticales de indentación.
    pub indent_guides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            show_whitespace: false,
            show_line_endings: false,
            indent_guides: true,
        }
    }
}

impl Config {
    /// Nombre del archivo dentro del directorio de datos.
    pub const FILE_NAME: &'static str = "config.toml";

    /// Lee la configuración. Si el archivo no existe, devuelve la configuración por defecto.
    pub fn load(path: &Path) -> Result<Self, WorkspaceError> {
        let Some(contents) = read_optional(path)? else {
            return Ok(Self::default());
        };
        toml::from_str(&contents).map_err(|e| WorkspaceError::Parse {
            path: path.to_owned(),
            message: e.to_string(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), WorkspaceError> {
        let contents = toml::to_string_pretty(self).map_err(|e| WorkspaceError::Serialize {
            path: path.to_owned(),
            message: e.to_string(),
        })?;
        write_atomic(path, contents.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&dir.path().join(Config::FILE_NAME)).unwrap();
        assert_eq!(config.theme, None);
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(Config::FILE_NAME);
        let config = Config {
            theme: Some(ThemeMode::Dark),
            show_whitespace: true,
            ..Config::default()
        };
        config.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("theme = \"dark\""));
        assert!(written.contains("show_whitespace = true"));
        assert_eq!(Config::load(&path).unwrap(), config);
    }

    #[test]
    fn old_files_get_defaults_for_new_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(Config::FILE_NAME);
        std::fs::write(&path, "theme = \"light\"").unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.theme, Some(ThemeMode::Light));
        assert!(config.indent_guides);
        assert!(!config.show_whitespace);
    }

    #[test]
    fn invalid_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(Config::FILE_NAME);
        std::fs::write(&path, "theme = \"violeta\"").unwrap();
        assert!(matches!(
            Config::load(&path),
            Err(WorkspaceError::Parse { .. })
        ));
    }
}
