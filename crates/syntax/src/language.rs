//! Lenguajes reconocidos y su detección.

use std::fmt;
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
            Self::Markdown => "Texto con formato",
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

    /// Identificador estable para guardar en sesión o configuración.
    pub const fn id(self) -> &'static str {
        match self {
            Self::PlainText => "plain",
            Self::Markdown => "markdown",
            Self::Sql => "sql",
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Html => "html",
            Self::Css => "css",
            Self::Bash => "bash",
        }
    }

    /// Inverso de [`Language::id`].
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.id() == id)
    }

    /// Lenguaje para un nombre o alias habitual (`rs`, `py`, `sh`, `yml`…), como los que se
    /// usan en los bloques de código de Markdown. No distingue mayúsculas.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let language = match name.as_str() {
            "text" | "txt" | "plain" | "plaintext" => Self::PlainText,
            "markdown" | "md" => Self::Markdown,
            "sql" | "mysql" | "postgresql" | "postgres" | "sqlite" | "pgsql" => Self::Sql,
            "rust" | "rs" => Self::Rust,
            "python" | "py" | "python3" => Self::Python,
            "javascript" | "js" | "jsx" | "node" | "mjs" | "cjs" => Self::JavaScript,
            "typescript" | "ts" | "tsx" => Self::TypeScript,
            "json" | "jsonc" | "json5" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "html" | "htm" | "xhtml" => Self::Html,
            "css" => Self::Css,
            "bash" | "sh" | "shell" | "zsh" | "console" => Self::Bash,
            _ => return None,
        };
        Some(language)
    }

    /// Detecta el lenguaje: primero por nombre o extensión del archivo y, si no lo
    /// determinan, por el shebang de la primera línea (`#!/usr/bin/env python3`).
    pub fn detect(path: Option<&Path>, first_line: &str) -> Self {
        match path.map(Self::from_path) {
            Some(language) if language != Self::PlainText => language,
            _ => Self::from_shebang(first_line).unwrap_or(Self::PlainText),
        }
    }

    /// Lenguaje indicado por un shebang, si la línea lo es.
    pub fn from_shebang(line: &str) -> Option<Self> {
        let command = line.strip_prefix("#!")?;
        let mut words = command.split_whitespace();
        let program = words.next()?.rsplit('/').next()?;
        // `#!/usr/bin/env -S python3 -u`: el intérprete es la primera palabra sin guion.
        let interpreter = if program == "env" {
            words.find(|w| !w.starts_with('-') && !w.contains('='))?
        } else {
            program
        };
        // python3.12 → python, node18 → node.
        let base = interpreter.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        match base {
            "python" | "pypy" => Some(Self::Python),
            "bash" | "sh" | "zsh" | "dash" | "ksh" => Some(Self::Bash),
            "node" | "deno" | "bun" => Some(Self::JavaScript),
            "ts-node" | "tsx" => Some(Self::TypeScript),
            _ => None,
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

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for language in Language::ALL {
            assert_eq!(Language::from_id(language.id()), Some(language));
        }
        assert_eq!(Language::from_id("cobol"), None);
    }

    #[test]
    fn aliases() {
        assert_eq!(Language::from_name("RS"), Some(Language::Rust));
        assert_eq!(Language::from_name("yml"), Some(Language::Yaml));
        assert_eq!(Language::from_name("sh"), Some(Language::Bash));
        assert_eq!(Language::from_name("brainfuck"), None);
    }

    #[test]
    fn shebangs() {
        assert_eq!(
            Language::from_shebang("#!/usr/bin/env python3"),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_shebang("#!/usr/bin/python3.12 -u"),
            Some(Language::Python)
        );
        assert_eq!(Language::from_shebang("#!/bin/bash"), Some(Language::Bash));
        assert_eq!(
            Language::from_shebang("#!/usr/bin/env -S node --flag"),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_shebang("#!/usr/bin/env FOO=1 bash"),
            Some(Language::Bash)
        );
        assert_eq!(Language::from_shebang("#!/usr/bin/env perl"), None);
        assert_eq!(Language::from_shebang("# comentario"), None);
    }

    #[test]
    fn detect_prefers_extension_then_shebang() {
        let script = Path::new("deploy");
        assert_eq!(Language::detect(Some(script), "#!/bin/sh"), Language::Bash);
        assert_eq!(
            Language::detect(Some(Path::new("x.py")), "#!/bin/sh"),
            Language::Python
        );
        assert_eq!(
            Language::detect(None, "#!/usr/bin/env python"),
            Language::Python
        );
        assert_eq!(Language::detect(None, "hola"), Language::PlainText);
    }

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
