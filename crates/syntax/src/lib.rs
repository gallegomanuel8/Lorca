//! Detección de lenguaje y resaltado de sintaxis.
//!
//! En la Fase 1 solo contiene la detección por extensión o nombre de archivo, que usa la
//! barra de estado. El resaltado llegará en la Fase 2.

use std::path::Path;

/// Lenguajes que Lorca reconoce.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    PlainText,
    Markdown,
    Sql,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Json,
    Yaml,
    Toml,
    Html,
    Css,
    Bash,
}

impl Language {
    /// Todos los lenguajes, en el orden en que se muestran al usuario.
    pub const ALL: [Self; 13] = [
        Self::PlainText,
        Self::Markdown,
        Self::Sql,
        Self::Rust,
        Self::Python,
        Self::JavaScript,
        Self::TypeScript,
        Self::Json,
        Self::Yaml,
        Self::Toml,
        Self::Html,
        Self::Css,
        Self::Bash,
    ];

    /// Nombre para mostrar en la interfaz.
    pub const fn name(self) -> &'static str {
        match self {
            Self::PlainText => "Texto plano",
            Self::Markdown => "Markdown",
            Self::Sql => "SQL",
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Bash => "Bash",
        }
    }

    /// Detecta el lenguaje por el nombre o la extensión del archivo.
    pub fn from_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if let Some(language) = Self::from_file_name(file_name) {
            return language;
        }
        path.extension()
            .and_then(|e| e.to_str())
            .map_or(Self::PlainText, Self::from_extension)
    }

    /// Detecta el lenguaje por la extensión (sin el punto, sin distinguir mayúsculas).
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_ascii_lowercase().as_str() {
            "md" | "markdown" | "mdown" | "mkd" => Self::Markdown,
            "sql" => Self::Sql,
            "rs" => Self::Rust,
            "py" | "pyw" | "pyi" => Self::Python,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "ts" | "mts" | "cts" | "tsx" => Self::TypeScript,
            "json" | "jsonc" | "json5" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "html" | "htm" | "xhtml" => Self::Html,
            "css" => Self::Css,
            "sh" | "bash" | "zsh" => Self::Bash,
            _ => Self::PlainText,
        }
    }

    fn from_file_name(file_name: &str) -> Option<Self> {
        match file_name {
            "Cargo.lock" => Some(Self::Toml),
            ".bashrc" | ".bash_profile" | ".zshrc" | ".profile" => Some(Self::Bash),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_extension() {
        assert_eq!(
            Language::from_path(Path::new("notas.md")),
            Language::Markdown
        );
        assert_eq!(
            Language::from_path(Path::new("src/main.RS")),
            Language::Rust
        );
        assert_eq!(
            Language::from_path(Path::new("consulta.sql")),
            Language::Sql
        );
        assert_eq!(
            Language::from_path(Path::new("app.tsx")),
            Language::TypeScript
        );
        assert_eq!(Language::from_path(Path::new("config.yml")), Language::Yaml);
    }

    #[test]
    fn detects_by_file_name() {
        assert_eq!(
            Language::from_path(Path::new("/home/x/.zshrc")),
            Language::Bash
        );
        assert_eq!(Language::from_path(Path::new("Cargo.lock")), Language::Toml);
    }

    #[test]
    fn unknown_is_plain_text() {
        assert_eq!(Language::from_path(Path::new("LEEME")), Language::PlainText);
        assert_eq!(
            Language::from_path(Path::new("datos.xyz")),
            Language::PlainText
        );
    }

    #[test]
    fn every_language_has_a_name() {
        assert!(Language::ALL.iter().all(|l| !l.name().is_empty()));
    }
}
