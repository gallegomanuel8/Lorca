//! Servidores conocidos por lenguaje, búsqueda de ejecutables y raíz del proyecto.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lorca_syntax::Language;

/// Orden para lanzar un servidor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl ServerCommand {
    pub fn new(command: &str, args: &[&str]) -> Self {
        Self {
            command: command.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        }
    }
}

/// Servidores habituales para cada lenguaje, por orden de preferencia.
pub fn known_servers(language: Language) -> Vec<ServerCommand> {
    let list: &[(&str, &[&str])] = match language {
        Language::Rust => &[("rust-analyzer", &[])],
        Language::Python => &[
            ("pyright-langserver", &["--stdio"]),
            ("basedpyright-langserver", &["--stdio"]),
            ("pylsp", &[]),
        ],
        Language::JavaScript | Language::TypeScript => {
            &[("typescript-language-server", &["--stdio"])]
        }
        Language::Json => &[("vscode-json-language-server", &["--stdio"])],
        Language::Html => &[("vscode-html-language-server", &["--stdio"])],
        Language::Css => &[("vscode-css-language-server", &["--stdio"])],
        Language::Yaml => &[("yaml-language-server", &["--stdio"])],
        Language::Toml => &[("taplo", &["lsp", "stdio"])],
        Language::Bash => &[("bash-language-server", &["start"])],
        Language::Markdown => &[("marksman", &["server"])],
        Language::PlainText | Language::Sql => &[],
    };
    list.iter()
        .map(|(command, args)| ServerCommand::new(command, args))
        .collect()
}

/// Identificador de lenguaje del protocolo (`languageId` de `didOpen`).
pub fn language_id(language: Language, path: Option<&Path>) -> &'static str {
    let extension = path
        .and_then(Path::extension)
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match language {
        Language::TypeScript if extension == "tsx" => "typescriptreact",
        Language::JavaScript if extension == "jsx" => "javascriptreact",
        Language::Bash => "shellscript",
        Language::PlainText => "plaintext",
        other => other.id(),
    }
}

/// Carpetas donde suelen instalarse los servidores. Se añaden al `PATH` porque las
/// aplicaciones abiertas desde el Finder (o un menú de escritorio) no heredan el `PATH`
/// de la terminal.
fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        for sub in [
            ".cargo/bin",
            ".local/bin",
            "go/bin",
            ".npm-global/bin",
            ".bun/bin",
        ] {
            dirs.push(home.join(sub));
        }
    }
    if cfg!(unix) {
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
            dirs.push(PathBuf::from(dir));
        }
    }
    dirs
}

/// `PATH` del proceso más las carpetas habituales, sin repetir.
pub fn search_path() -> OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    for dir in extra_dirs() {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Ruta completa de `command` si está en `path` (con las extensiones de Windows).
pub fn find_executable_in(command: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_owned());
    }
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_owned())
            .split(';')
            .map(str::to_owned)
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(path).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let file = dir.join(format!("{command}{ext}"));
            file.is_file().then_some(file)
        })
    })
}

pub fn find_executable(command: &str) -> Option<PathBuf> {
    find_executable_in(command, &search_path())
}

/// Archivos o carpetas que marcan la raíz de un proyecto.
const ROOT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "compile_commands.json",
];

/// Carpeta raíz del proyecto de `file`: la más cercana con algún marcador
/// ([`ROOT_MARKERS`]); en Rust, la más externa con `Cargo.toml` (la del workspace) sin
/// salir del repositorio. Si no hay marcadores, la más cercana con `.git`, y si tampoco,
/// la carpeta del archivo.
pub fn find_root(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(file);
    let has = |dir: &Path, name: &str| dir.join(name).exists();
    let nearest = parent
        .ancestors()
        .find(|dir| ROOT_MARKERS.iter().any(|m| has(dir, m)));
    match nearest {
        Some(dir) if has(dir, "Cargo.toml") => {
            let mut root = dir;
            for ancestor in dir.ancestors() {
                if has(ancestor, "Cargo.toml") {
                    root = ancestor;
                }
                if has(ancestor, ".git") {
                    break;
                }
            }
            root.to_owned()
        }
        Some(dir) => dir.to_owned(),
        None => parent
            .ancestors()
            .find(|dir| has(dir, ".git"))
            .unwrap_or(parent)
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_uses_rust_analyzer() {
        assert_eq!(
            known_servers(Language::Rust),
            vec![ServerCommand::new("rust-analyzer", &[])]
        );
        assert!(known_servers(Language::PlainText).is_empty());
    }

    #[test]
    fn language_ids_follow_the_protocol() {
        assert_eq!(language_id(Language::Bash, None), "shellscript");
        assert_eq!(
            language_id(Language::TypeScript, Some(Path::new("a.tsx"))),
            "typescriptreact"
        );
        assert_eq!(language_id(Language::Rust, None), "rust");
    }

    #[test]
    fn rust_root_is_the_outermost_cargo_toml_inside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        let member = project.join("crates").join("core");
        std::fs::create_dir_all(member.join("src")).unwrap();
        std::fs::write(member.join("Cargo.toml"), "").unwrap();
        let file = member.join("src").join("lib.rs");
        assert_eq!(find_root(&file), member);
        std::fs::write(project.join("Cargo.toml"), "").unwrap();
        assert_eq!(find_root(&file), project);
        // No se sale del repositorio aunque haya otro Cargo.toml más arriba.
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        std::fs::create_dir(project.join(".git")).unwrap();
        assert_eq!(find_root(&file), project);
    }

    #[test]
    fn other_roots_use_nearest_marker_then_git() {
        let dir = tempfile::tempdir().unwrap();
        let web = dir.path().join("web");
        std::fs::create_dir_all(web.join("src")).unwrap();
        std::fs::write(web.join("package.json"), "{}").unwrap();
        assert_eq!(find_root(&web.join("src").join("a.ts")), web);
        let notes = dir.path().join("notas");
        std::fs::create_dir_all(notes.join("sub")).unwrap();
        assert_eq!(
            find_root(&notes.join("sub").join("a.md")),
            notes.join("sub")
        );
        std::fs::create_dir(notes.join(".git")).unwrap();
        assert_eq!(find_root(&notes.join("sub").join("a.md")), notes);
    }

    #[test]
    fn executables_are_found_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) {
            "servidor.exe"
        } else {
            "servidor"
        };
        std::fs::write(dir.path().join(name), "").unwrap();
        let path = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(
            find_executable_in("servidor", &path),
            Some(dir.path().join(name))
        );
        assert_eq!(find_executable_in("no-existe", &path), None);
    }
}
