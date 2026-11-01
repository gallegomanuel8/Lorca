//! Temas de color: los incluidos (con nombres de obras de Lorca) y los del usuario.
//!
//! Un tema es un archivo TOML con la paleta de la interfaz, colores opcionales del
//! editor y el estilo de cada categoría de resaltado. Los temas del usuario se leen de
//! `<datos>/themes/*.toml`; uno con el mismo nombre que un tema incluido lo sustituye.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lorca_syntax::HighlightKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::WorkspaceError;

/// Nombre de la carpeta de temas del usuario dentro del directorio de datos.
pub const THEMES_DIR: &str = "themes";
/// Tema claro por defecto.
pub const DEFAULT_LIGHT_THEME: &str = "Mariana Pineda";
/// Tema oscuro por defecto.
pub const DEFAULT_DARK_THEME: &str = "Romancero gitano";

/// Temas incluidos en el binario.
const BUILTIN: &[&str] = &[
    include_str!("../themes/romancero-gitano.toml"),
    include_str!("../themes/yerma.toml"),
    include_str!("../themes/bodas-de-sangre.toml"),
    include_str!("../themes/poeta-en-nueva-york.toml"),
    include_str!("../themes/mariana-pineda.toml"),
    include_str!("../themes/bernarda-alba.toml"),
    include_str!("../themes/dona-rosita.toml"),
];

/// Color sRGB con opacidad. En TOML se escribe `"#rrggbb"` o `"#rrggbbaa"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

impl FromStr for Rgba {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("color inválido «{s}» (se espera #rrggbb o #rrggbbaa)");
        let hex = s.strip_prefix('#').ok_or_else(invalid)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
            return Err(invalid());
        }
        let byte = |i: usize| {
            hex.get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(invalid)
        };
        Ok(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 255 },
        })
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Si el tema es claro u oscuro (decide cuál usa el modo automático).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Appearance {
    Light,
    Dark,
}

/// Colores base de la interfaz (de ellos se derivan botones, campos y paneles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiPalette {
    pub background: Rgba,
    pub text: Rgba,
    pub primary: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
}

/// Colores del área de edición. Los ausentes se derivan de la paleta de la interfaz.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EditorPalette {
    pub background: Option<Rgba>,
    pub gutter: Option<Rgba>,
    pub line_number: Option<Rgba>,
    pub line_number_active: Option<Rgba>,
    pub current_line: Option<Rgba>,
    pub selection: Option<Rgba>,
    pub cursor: Option<Rgba>,
    pub find: Option<Rgba>,
    pub find_current: Option<Rgba>,
    pub mark: Option<Rgba>,
    pub bookmark: Option<Rgba>,
    pub whitespace: Option<Rgba>,
    pub guide: Option<Rgba>,
    pub error: Option<Rgba>,
    pub warning: Option<Rgba>,
    pub info: Option<Rgba>,
}

/// Estilo de una categoría de resaltado. En TOML vale `"#rrggbb"` o una tabla
/// `{ color = "#rrggbb", bold = true, italic = true }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TokenStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
}

impl<'de> Deserialize<'de> for TokenStyle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Table {
            color: Option<Rgba>,
            #[serde(default)]
            bold: bool,
            #[serde(default)]
            italic: bool,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Color(Rgba),
            Table(Table),
        }

        Ok(match Raw::deserialize(deserializer)? {
            Raw::Color(color) => Self {
                color: Some(color),
                ..Self::default()
            },
            Raw::Table(table) => Self {
                color: table.color,
                bold: table.bold,
                italic: table.italic,
            },
        })
    }
}

/// Un tema completo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSpec {
    pub name: String,
    pub appearance: Appearance,
    /// Una línea sobre el tema (se muestra en el selector).
    #[serde(default)]
    pub description: String,
    pub palette: UiPalette,
    #[serde(default)]
    pub editor: EditorPalette,
    /// Estilo por categoría; las claves son [`HighlightKind::name`].
    #[serde(default)]
    pub syntax: BTreeMap<String, TokenStyle>,
}

impl ThemeSpec {
    /// Lee un tema y comprueba que las categorías de `[syntax]` existen.
    pub fn parse(source: &str) -> Result<Self, String> {
        let spec: Self = toml::from_str(source).map_err(|e| e.to_string())?;
        if spec.name.trim().is_empty() {
            return Err("el tema no tiene nombre".to_owned());
        }
        if let Some(unknown) = spec
            .syntax
            .keys()
            .find(|key| HighlightKind::from_name(key).is_none())
        {
            let known: Vec<&str> = HighlightKind::ALL.iter().map(|k| k.name()).collect();
            return Err(format!(
                "categoría desconocida «{unknown}» en [syntax]; válidas: {}",
                known.join(", ")
            ));
        }
        Ok(spec)
    }

    /// Estilo de una categoría, si el tema le da alguno.
    pub fn token(&self, kind: HighlightKind) -> Option<TokenStyle> {
        self.syntax.get(kind.name()).copied()
    }

    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }
}

/// Origen de un tema de la colección.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeSource {
    Builtin,
    File(PathBuf),
}

/// Temas disponibles: los incluidos más los del usuario, en orden de presentación.
#[derive(Debug, Clone)]
pub struct ThemeSet {
    themes: Vec<(ThemeSpec, ThemeSource)>,
}

impl Default for ThemeSet {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ThemeSet {
    /// Solo los temas incluidos.
    pub fn builtin() -> Self {
        let themes = BUILTIN
            .iter()
            .filter_map(|source| ThemeSpec::parse(source).ok())
            .map(|spec| (spec, ThemeSource::Builtin))
            .collect();
        Self { themes }
    }

    /// Los temas incluidos más los `*.toml` de `dir` (si existe). Los archivos con
    /// errores se omiten y se devuelven sus errores para avisar al usuario.
    pub fn load(dir: &Path) -> (Self, Vec<WorkspaceError>) {
        let mut set = Self::builtin();
        let mut errors = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (set, errors),
            Err(source) => {
                errors.push(WorkspaceError::Read {
                    path: dir.to_owned(),
                    source,
                });
                return (set, errors);
            }
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort();
        for path in paths {
            let parsed = std::fs::read_to_string(&path)
                .map_err(|source| WorkspaceError::Read {
                    path: path.clone(),
                    source,
                })
                .and_then(|source| {
                    ThemeSpec::parse(&source).map_err(|message| WorkspaceError::Parse {
                        path: path.clone(),
                        message,
                    })
                });
            match parsed {
                Ok(spec) => set.insert(spec, ThemeSource::File(path)),
                Err(error) => errors.push(error),
            }
        }
        (set, errors)
    }

    /// Añade un tema; si ya hay uno con el mismo nombre, lo sustituye en su sitio.
    pub fn insert(&mut self, spec: ThemeSpec, source: ThemeSource) {
        match self.themes.iter_mut().find(|(t, _)| t.name == spec.name) {
            Some(slot) => *slot = (spec, source),
            None => self.themes.push((spec, source)),
        }
    }

    pub fn get(&self, name: &str) -> Option<&ThemeSpec> {
        self.themes
            .iter()
            .map(|(spec, _)| spec)
            .find(|spec| spec.name == name)
    }

    pub fn source(&self, name: &str) -> Option<&ThemeSource> {
        self.themes
            .iter()
            .find(|(spec, _)| spec.name == name)
            .map(|(_, source)| source)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ThemeSpec> {
        self.themes.iter().map(|(spec, _)| spec)
    }

    /// Texto TOML del tema (el incluido o el de su archivo).
    pub fn source_text(&self, name: &str) -> Option<String> {
        match self.source(name)? {
            ThemeSource::Builtin => BUILTIN
                .iter()
                .find(|source| ThemeSpec::parse(source).is_ok_and(|spec| spec.name == name))
                .map(|source| (*source).to_owned()),
            ThemeSource::File(path) => std::fs::read_to_string(path).ok(),
        }
    }

    /// Crea en `dir` una copia editable del tema `name` con un nombre nuevo
    /// («<nombre> (personal)», numerado si ya existe). Devuelve el nombre y la ruta.
    pub fn create_personal(
        &self,
        name: &str,
        dir: &Path,
    ) -> Result<(String, PathBuf), WorkspaceError> {
        let source = self
            .source_text(name)
            .ok_or_else(|| WorkspaceError::Parse {
                path: dir.to_owned(),
                message: format!("no se encontró el tema «{name}»"),
            })?;
        let base = format!("{name} (personal)");
        let new_name = (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{name} (personal {n})")
                }
            })
            .find(|candidate| {
                self.get(candidate).is_none() && !dir.join(theme_file_name(candidate)).exists()
            })
            .unwrap_or(base);
        let quoted = toml::Value::String(new_name.clone()).to_string();
        let mut renamed = false;
        let text: Vec<String> = source
            .lines()
            .map(|line| {
                if !renamed && line.trim_start().starts_with("name") && line.contains('=') {
                    renamed = true;
                    format!("name = {quoted}")
                } else {
                    line.to_owned()
                }
            })
            .collect();
        let path = dir.join(theme_file_name(&new_name));
        crate::storage::write_atomic(&path, (text.join("\n") + "\n").as_bytes())?;
        Ok((new_name, path))
    }

    /// Temas de un aspecto, en orden de presentación.
    pub fn with_appearance(&self, appearance: Appearance) -> impl Iterator<Item = &ThemeSpec> {
        self.iter()
            .filter(move |spec| spec.appearance == appearance)
    }

    /// El tema `name` si existe y tiene el aspecto pedido; si no, el primero de ese
    /// aspecto. `None` solo si no hay ningún tema de ese aspecto.
    pub fn resolve(&self, name: &str, appearance: Appearance) -> Option<&ThemeSpec> {
        self.get(name)
            .filter(|spec| spec.appearance == appearance)
            .or_else(|| self.with_appearance(appearance).next())
    }
}

/// Nombre de archivo para un tema: minúsculas, sin tildes y con guiones.
pub fn theme_file_name(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    format!("{}.toml", if slug.is_empty() { "tema" } else { slug })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_and_print() {
        let color: Rgba = "#1A2b3c".parse().unwrap();
        assert_eq!(color, Rgba::rgb(0x1a, 0x2b, 0x3c));
        assert_eq!(color.to_string(), "#1a2b3c");
        let translucent: Rgba = "#ffffff80".parse().unwrap();
        assert_eq!(translucent.a, 0x80);
        assert_eq!(translucent.to_string(), "#ffffff80");
        for bad in ["123456", "#12345", "#gg0000", "#ñ00000"] {
            assert!(bad.parse::<Rgba>().is_err(), "{bad}");
        }
    }

    #[test]
    fn builtin_themes_are_valid_and_complete() {
        assert_eq!(ThemeSet::builtin().iter().count(), BUILTIN.len());
        for source in BUILTIN {
            let spec = ThemeSpec::parse(source).unwrap();
            for kind in [
                HighlightKind::Keyword,
                HighlightKind::String,
                HighlightKind::Comment,
                HighlightKind::Function,
            ] {
                assert!(
                    spec.token(kind).and_then(|t| t.color).is_some(),
                    "{} sin color para {}",
                    spec.name,
                    kind.name()
                );
            }
        }
        let set = ThemeSet::builtin();
        for name in ["Yerma", "Bodas de sangre", "Romancero gitano"] {
            assert!(set.get(name).is_some(), "{name}");
        }
        assert!(set.get(DEFAULT_LIGHT_THEME).is_some_and(|t| !t.is_dark()));
        assert!(set.get(DEFAULT_DARK_THEME).is_some_and(ThemeSpec::is_dark));
    }

    #[test]
    fn token_styles_accept_string_or_table() {
        let spec = ThemeSpec::parse(
            r##"
            name = "Prueba"
            appearance = "dark"
            [palette]
            background = "#000000"
            text = "#ffffff"
            primary = "#0000ff"
            success = "#00ff00"
            warning = "#ffff00"
            danger = "#ff0000"
            [syntax]
            keyword = "#ff00ff"
            comment = { color = "#888888", italic = true }
            strong = { bold = true }
            "##,
        )
        .unwrap();
        assert_eq!(
            spec.token(HighlightKind::Keyword).unwrap().color,
            Some(Rgba::rgb(255, 0, 255))
        );
        assert!(spec.token(HighlightKind::Comment).unwrap().italic);
        let strong = spec.token(HighlightKind::Strong).unwrap();
        assert!(strong.bold && strong.color.is_none());
        assert_eq!(spec.token(HighlightKind::Number), None);
    }

    #[test]
    fn unknown_syntax_category_is_rejected() {
        let source = BUILTIN[0].replace("[syntax]", "[syntax]\nverde = \"#00ff00\"");
        let error = ThemeSpec::parse(&source).unwrap_err();
        assert!(error.contains("verde"), "{error}");
    }

    #[test]
    fn user_themes_add_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        let custom = BUILTIN[0].replace("name = \"Romancero gitano\"", "name = \"Mío\"");
        std::fs::write(dir.path().join("mio.toml"), custom).unwrap();
        let replaced = BUILTIN[1].replace("description = ", "description = \"Otro\" #");
        std::fs::write(dir.path().join("yerma.toml"), replaced).unwrap();
        std::fs::write(dir.path().join("roto.toml"), "name = 3").unwrap();
        std::fs::write(dir.path().join("notas.txt"), "no es un tema").unwrap();

        let (set, errors) = ThemeSet::load(dir.path());
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], WorkspaceError::Parse { path, .. } if path.ends_with("roto.toml"))
        );
        assert!(set.get("Mío").is_some());
        assert_eq!(set.get("Yerma").unwrap().description, "Otro");
        assert!(matches!(set.source("Yerma"), Some(ThemeSource::File(_))));
        assert_eq!(set.iter().count(), BUILTIN.len() + 1);
    }

    #[test]
    fn file_names_are_slugs() {
        assert_eq!(
            theme_file_name("Doña Rosita la soltera (personal)"),
            "dona-rosita-la-soltera-personal.toml"
        );
        assert_eq!(theme_file_name("¡¿?!"), "tema.toml");
    }

    #[test]
    fn personal_copy_is_loadable_and_numbered() {
        let dir = tempfile::tempdir().unwrap();
        let set = ThemeSet::builtin();
        let (name, path) = set.create_personal("Yerma", dir.path()).unwrap();
        assert_eq!(name, "Yerma (personal)");
        let (set, errors) = ThemeSet::load(dir.path());
        assert!(errors.is_empty(), "{errors:?}");
        let copy = set.get("Yerma (personal)").unwrap();
        assert_eq!(copy.palette, set.get("Yerma").unwrap().palette);
        assert_eq!(set.source(&name), Some(&ThemeSource::File(path)));
        // Los comentarios del original se conservan.
        assert!(set.source_text(&name).unwrap().starts_with("# Yerma"));

        let (second, _) = set.create_personal("Yerma", dir.path()).unwrap();
        assert_eq!(second, "Yerma (personal 2)");
    }

    #[test]
    fn missing_themes_dir_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let (set, errors) = ThemeSet::load(&dir.path().join("no-existe"));
        assert!(errors.is_empty());
        assert_eq!(set.iter().count(), BUILTIN.len());
    }

    #[test]
    fn resolve_falls_back_to_first_of_appearance() {
        let set = ThemeSet::builtin();
        assert_eq!(
            set.resolve("Yerma", Appearance::Dark).unwrap().name,
            "Yerma"
        );
        // Un tema claro no vale como oscuro.
        let dark = set.resolve(DEFAULT_LIGHT_THEME, Appearance::Dark).unwrap();
        assert!(dark.is_dark());
        assert!(set.resolve("No existe", Appearance::Light).is_some());
    }
}
