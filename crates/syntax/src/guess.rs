//! Deducción del lenguaje a partir del contenido (p. ej. al pegar texto en un documento
//! sin lenguaje).
//!
//! Heurística por líneas: cada lenguaje suma puntos por construcciones típicas y gana el
//! que más sume si supera un umbral y no empata. Ante la duda devuelve texto plano: es
//! preferible no resaltar a resaltar con el lenguaje equivocado. JSON se confirma
//! analizándolo con su gramática.

use crate::Language;

/// Líneas que se examinan como máximo (basta una muestra para decidir).
const MAX_LINES: usize = 400;

/// Lenguajes que se puntúan por líneas, en el orden de [`Scores`].
const CANDIDATES: [Language; 10] = [
    Language::Markdown,
    Language::Sql,
    Language::Rust,
    Language::Python,
    Language::JavaScript,
    Language::Yaml,
    Language::Toml,
    Language::Css,
    Language::Bash,
    Language::TypeScript,
];

/// Puntos de cada candidato (índices de [`CANDIDATES`]).
#[derive(Debug, Default)]
struct Scores {
    markdown: u32,
    sql: u32,
    rust: u32,
    python: u32,
    javascript: u32,
    yaml: u32,
    toml: u32,
    css: u32,
    bash: u32,
    /// Construcciones que solo tiene TypeScript (se suman a las de JavaScript).
    typescript: u32,
    toml_headers: u32,
    toml_pairs: u32,
    /// Líneas con estructura propia de YAML (`---`, `- elemento`, `clave:` con bloque).
    yaml_structure: u32,
}

impl Language {
    /// Deduce el lenguaje de `text` por su contenido. [`Language::PlainText`] si no está
    /// claro.
    pub fn guess(text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self::PlainText;
        }
        if let Some(language) = trimmed.lines().next().and_then(Self::from_shebang) {
            return language;
        }
        if looks_like_json(trimmed) {
            return Self::Json;
        }
        if looks_like_html(trimmed) {
            return Self::Html;
        }
        score_lines(trimmed)
    }
}

/// Empieza y termina como un objeto o una lista y la gramática de JSON lo acepta sin
/// errores.
fn looks_like_json(text: &str) -> bool {
    let delimited = (text.starts_with('{') && text.ends_with('}'))
        || (text.starts_with('[') && text.ends_with(']'));
    if !delimited {
        return false;
    }
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_json::LANGUAGE.into())
        .is_err()
    {
        return false;
    }
    parser
        .parse(text, None)
        .is_some_and(|tree| !tree.root_node().has_error())
}

/// Documento HTML o fragmento que empieza con una etiqueta y tiene etiquetas de cierre.
fn looks_like_html(text: &str) -> bool {
    let start: String = text.chars().take(15).collect::<String>().to_lowercase();
    if start.starts_with("<!doctype html") || start.starts_with("<html") {
        return true;
    }
    let starts_with_tag = text
        .strip_prefix('<')
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_alphabetic());
    starts_with_tag && text.ends_with('>') && text.contains("</")
}

fn score_lines(text: &str) -> Language {
    let mut scores = Scores::default();
    let mut lines = 0u32;
    for line in text.lines().take(MAX_LINES) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        lines += 1;
        score_line(line, &mut scores);
    }
    // TOML: los pares `clave = valor` solo cuentan si hay secciones o si todo son pares
    // (si no, una asignación de Python o JavaScript parecería TOML).
    if scores.toml_headers > 0 || scores.toml_pairs == lines {
        scores.toml = scores.toml_headers * 3 + scores.toml_pairs;
    }
    // YAML: unas líneas «Clave: valor» sueltas son también notas en prosa.
    if scores.yaml_structure == 0 {
        scores.yaml = 0;
    }
    if scores.typescript > 0 {
        scores.typescript += scores.javascript;
        scores.javascript = 0;
    }
    let values = [
        scores.markdown,
        scores.sql,
        scores.rust,
        scores.python,
        scores.javascript,
        scores.yaml,
        scores.toml,
        scores.css,
        scores.bash,
        scores.typescript,
    ];
    let Some((best, &top)) = values.iter().enumerate().max_by_key(|&(_, value)| value) else {
        return Language::PlainText;
    };
    let tied = values.iter().filter(|&&value| value == top).count() > 1;
    // Al menos 2 puntos y, en textos largos, uno por cada cuatro líneas.
    let threshold = 2.max(lines.div_ceil(4));
    if tied || top < threshold {
        return Language::PlainText;
    }
    CANDIDATES.get(best).copied().unwrap_or(Language::PlainText)
}

fn score_line(line: &str, scores: &mut Scores) {
    scores.markdown += markdown(line);
    scores.sql += sql(line);
    scores.rust += rust(line);
    scores.python += python(line);
    scores.javascript += javascript(line);
    scores.typescript += typescript(line);
    scores.yaml += yaml(line);
    if line == "---"
        || line.starts_with("- ")
        || (line.ends_with(':') && is_key(&line[..line.len() - 1]))
    {
        scores.yaml_structure += 1;
    }
    scores.css += css(line);
    scores.bash += bash(line);
    if is_toml_header(line) {
        scores.toml_headers += 1;
    } else if is_toml_pair(line) {
        scores.toml_pairs += 1;
    }
}

fn starts_with_any(line: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| line.starts_with(p))
}

fn contains_any(line: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| line.contains(n))
}

/// Identificador sencillo: letras, dígitos, `_` y `-`, sin empezar por dígito.
fn is_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn markdown(line: &str) -> u32 {
    let mut points = 0;
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (2..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        points += 2;
    } else if hashes == 1 && line.starts_with("# ") {
        // También es un comentario de Python, Bash, YAML o TOML.
        points += 1;
    }
    if line.starts_with("```") || line.starts_with("~~~") {
        points += 3;
    }
    if line.contains("](") && line.contains('[') {
        points += 2;
    }
    if line.starts_with("|") && line.contains("---") {
        points += 2;
    }
    if starts_with_any(line, &["- [ ] ", "- [x] ", "> "]) {
        points += 1;
    }
    if line.contains("**") || (line.contains('`') && !line.contains("```")) {
        points += 1;
    }
    points
}

fn sql(line: &str) -> u32 {
    let lower = line.to_lowercase();
    let statement = starts_with_any(
        &lower,
        &[
            "select ",
            "insert into ",
            "update ",
            "delete from ",
            "create table ",
            "create index ",
            "create view ",
            "create database ",
            "alter table ",
            "drop table ",
            "with ",
        ],
    );
    let first_word = line.split_whitespace().next().unwrap_or_default();
    // «Select the best from the list» no es SQL: exige mayúsculas o símbolos de consulta.
    let shouty = first_word.len() > 1 && first_word.chars().all(|c| c.is_ascii_uppercase());
    let symbols = contains_any(line, &["*", ",", ";", "=", "(", "'"]);
    let mut points = 0;
    if statement && (shouty || symbols) {
        points += 3;
    }
    let clauses = [
        " from ", " where ", " join ", "group by", "order by", " values", " set ",
    ];
    if (shouty || symbols) && clauses.iter().any(|c| lower.contains(c)) {
        points += 1;
    }
    if starts_with_any(
        &lower,
        &["from ", "where ", "and ", "left join ", "inner join "],
    ) && line
        .split_whitespace()
        .next()
        .is_some_and(|w| w.chars().all(|c| c.is_ascii_uppercase()))
    {
        points += 2;
    }
    points
}

fn rust(line: &str) -> u32 {
    let mut points = 0;
    if starts_with_any(
        line,
        &[
            "fn ",
            "pub fn ",
            "pub(crate) ",
            "impl ",
            "impl<",
            "let mut ",
            "#[",
            "#![",
            "mod ",
            "pub struct ",
            "pub enum ",
            "trait ",
            "pub trait ",
            "async fn ",
            "pub async fn ",
        ],
    ) {
        points += 2;
    }
    if line.starts_with("use ") && line.contains("::") && line.ends_with(';') {
        points += 3;
    }
    if contains_any(
        line,
        &[
            "println!", "format!", "vec![", "&self", "&mut ", "Option<", "Result<", "::new(",
        ],
    ) {
        points += 2;
    }
    if (line.starts_with("fn ") || line.contains(" fn ")) && line.contains("->") {
        points += 1;
    }
    points
}

fn python(line: &str) -> u32 {
    let mut points = 0;
    let block = line.ends_with(':');
    if block
        && starts_with_any(
            line,
            &[
                "def ",
                "async def ",
                "class ",
                "elif ",
                "try",
                "except",
                "with ",
            ],
        )
    {
        points += 3;
    } else if block
        && starts_with_any(line, &["if ", "for ", "while ", "else"])
        && !line.contains('{')
    {
        points += 1;
    }
    if line.starts_with("from ") && line.contains(" import ") {
        points += 3;
    } else if line.starts_with("import ") && !contains_any(line, &["'", "\"", ";", "{"]) {
        points += 2;
    }
    if contains_any(
        line,
        &[
            "self.", "__name__", "__init__", "print(", "None", "True", "False",
        ],
    ) && !line.ends_with(';')
    {
        points += 1;
    }
    // Decorador: `@property`, `@app.route("/")`.
    if line.starts_with('@') && line.len() > 1 && !line.contains(' ') {
        points += 1;
    }
    points
}

fn javascript(line: &str) -> u32 {
    let mut points = 0;
    if starts_with_any(
        line,
        &[
            "function ",
            "async function ",
            "export default ",
            "export function ",
            "export const ",
        ],
    ) {
        points += 3;
    }
    if starts_with_any(line, &["const ", "let ", "var "]) && line.contains(" = ") {
        points += 2;
    }
    if line.starts_with("import ")
        && contains_any(line, &[" from '", " from \"", "import '", "import \""])
    {
        points += 3;
    }
    if contains_any(
        line,
        &[
            "console.",
            "require(",
            "document.",
            "=> ",
            "===",
            "!==",
            "window.",
            "module.exports",
        ],
    ) {
        points += 2;
    }
    points
}

fn typescript(line: &str) -> u32 {
    let mut points = 0;
    if starts_with_any(
        line,
        &["interface ", "export interface ", "type ", "export type "],
    ) && contains_any(line, &["{", " = "])
    {
        points += 3;
    }
    if contains_any(
        line,
        &[
            ": string",
            ": number",
            ": boolean",
            ": void",
            ": any",
            "as const",
            " implements ",
            "readonly ",
        ],
    ) {
        points += 2;
    }
    points
}

fn yaml(line: &str) -> u32 {
    if line == "---" {
        return 2;
    }
    let item = line.strip_prefix("- ").unwrap_or(line);
    let Some((key, value)) = item.split_once(':') else {
        return 0;
    };
    if !is_key(key) || line.ends_with(';') || line.ends_with(',') || line.ends_with('{') {
        return 0;
    }
    // `clave:` (bloque anidado) o `clave: valor`.
    if value.is_empty() || value.starts_with(' ') {
        1
    } else {
        0
    }
}

fn is_toml_header(line: &str) -> bool {
    let inner = line
        .strip_prefix("[[")
        .and_then(|l| l.strip_suffix("]]"))
        .or_else(|| line.strip_prefix('[').and_then(|l| l.strip_suffix(']')));
    inner.is_some_and(|name| {
        !name.is_empty() && name.split('.').all(|part| is_key(part.trim_matches('"')))
    })
}

fn is_toml_pair(line: &str) -> bool {
    let Some((key, value)) = line.split_once(" = ") else {
        return false;
    };
    let value = value.trim();
    is_key(key.trim().trim_matches('"'))
        && (value.starts_with(['"', '\'', '[', '{'])
            || value == "true"
            || value == "false"
            || value.parse::<f64>().is_ok())
        && !value.ends_with(';')
}

fn css(line: &str) -> u32 {
    if line.starts_with('@')
        && starts_with_any(line, &["@media", "@import", "@font-face", "@keyframes"])
    {
        return 3;
    }
    if line.ends_with('{')
        && starts_with_any(
            line,
            &[
                ".", "#", ":root", "*", "body", "html", "a ", "a:", "div", "h1", "p ",
            ],
        )
        && !line.contains('(')
    {
        return 2;
    }
    let Some((property, value)) = line.split_once(':') else {
        return 0;
    };
    let property_like =
        !property.is_empty() && property.chars().all(|c| c.is_ascii_lowercase() || c == '-');
    if property_like && line.ends_with(';') && !value.trim().is_empty() {
        2
    } else {
        0
    }
}

fn bash(line: &str) -> u32 {
    let line = line.strip_prefix("$ ").unwrap_or(line);
    let mut points = 0;
    if matches!(line, "fi" | "done" | "esac" | "then" | "do") || line.starts_with("if [") {
        points += 3;
    }
    if contains_any(
        line,
        &["$(", "${", " && ", " || ", " | ", "2>&1", ">/dev/null"],
    ) {
        points += 1;
    }
    let first = line.split_whitespace().next().unwrap_or_default();
    let command = matches!(
        first,
        "echo"
            | "export"
            | "source"
            | "cd"
            | "ls"
            | "mkdir"
            | "rm"
            | "cp"
            | "mv"
            | "sudo"
            | "apt"
            | "apt-get"
            | "brew"
            | "npm"
            | "npx"
            | "yarn"
            | "pnpm"
            | "cargo"
            | "pip"
            | "pip3"
            | "docker"
            | "kubectl"
            | "curl"
            | "wget"
            | "chmod"
            | "chown"
            | "git"
            | "grep"
            | "sed"
            | "awk"
            | "ssh"
            | "make"
            | "tar"
            | "cat"
    );
    // Una orden lleva opciones, rutas o variables; una frase en prosa termina en punto.
    let has_arguments = contains_any(line, &[" -", "/", "=", "|", ">", "$", "."]);
    if command && has_arguments && !line.ends_with('.') {
        points += 2;
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guess(text: &str) -> Language {
        Language::guess(text)
    }

    #[test]
    fn prose_and_empty_are_plain_text() {
        assert_eq!(guess(""), Language::PlainText);
        assert_eq!(guess("   \n\n"), Language::PlainText);
        assert_eq!(guess("Hola, ¿qué tal?"), Language::PlainText);
        assert_eq!(
            guess("Select the best option from the list.\nThen let me know."),
            Language::PlainText
        );
        assert_eq!(guess("git is great."), Language::PlainText);
        assert_eq!(guess("Nota: llamar mañana"), Language::PlainText);
        assert_eq!(guess("Hora: 10:00\nLugar: casa"), Language::PlainText);
        assert_eq!(
            guess("Lista de la compra:\n- pan\n- leche\n- huevos"),
            Language::PlainText
        );
    }

    #[test]
    fn json() {
        assert_eq!(
            guess("{\"nombre\": \"Lorca\", \"año\": 1898}"),
            Language::Json
        );
        assert_eq!(guess("[1, 2, {\"a\": null}]"), Language::Json);
        assert_eq!(guess("{\n  \"a\": [true, false]\n}\n"), Language::Json);
        // Sin comillas en las claves no es JSON (y tampoco otro lenguaje claro).
        assert_ne!(guess("{ a: 1 }"), Language::Json);
    }

    #[test]
    fn html() {
        assert_eq!(
            guess("<!DOCTYPE html>\n<html><body></body></html>"),
            Language::Html
        );
        assert_eq!(
            guess("<div class=\"x\">\n  <p>Hola</p>\n</div>"),
            Language::Html
        );
    }

    #[test]
    fn shebang() {
        assert_eq!(guess("#!/usr/bin/env python3\nprint(1)"), Language::Python);
    }

    #[test]
    fn sql() {
        assert_eq!(guess("SELECT * FROM users WHERE id = 1;"), Language::Sql);
        assert_eq!(
            guess("select nombre, edad\nfrom personas\nwhere edad > 30\norder by nombre;"),
            Language::Sql
        );
        assert_eq!(
            guess("CREATE TABLE obras (\n  id INTEGER PRIMARY KEY,\n  titulo TEXT\n);"),
            Language::Sql
        );
    }

    #[test]
    fn rust() {
        let code = "use std::collections::HashMap;\n\nfn main() {\n    let mut mapa = HashMap::new();\n    println!(\"{mapa:?}\");\n}\n";
        assert_eq!(guess(code), Language::Rust);
        assert_eq!(
            guess("#[derive(Debug)]\npub struct Punto {\n    x: f32,\n}"),
            Language::Rust
        );
    }

    #[test]
    fn python() {
        let code = "import os\nfrom pathlib import Path\n\ndef main():\n    for f in Path('.').iterdir():\n        print(f)\n\nif __name__ == '__main__':\n    main()\n";
        assert_eq!(guess(code), Language::Python);
        assert_eq!(
            guess("class Obra:\n    def __init__(self, titulo):\n        self.titulo = titulo"),
            Language::Python
        );
    }

    #[test]
    fn javascript_and_typescript() {
        let js = "const fs = require('fs');\n\nfunction leer(ruta) {\n  return fs.readFileSync(ruta, 'utf8');\n}\nconsole.log(leer('a.txt'));\n";
        assert_eq!(guess(js), Language::JavaScript);
        assert_eq!(
            guess("import React from 'react';\nexport default () => <div />;"),
            Language::JavaScript
        );
        let ts = "interface Obra {\n  titulo: string;\n  año: number;\n}\n\nexport const leer = (o: Obra): string => o.titulo;\n";
        assert_eq!(guess(ts), Language::TypeScript);
    }

    #[test]
    fn yaml() {
        let yaml = "name: CI\non:\n  push:\n    branches: [master]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n";
        assert_eq!(guess(yaml), Language::Yaml);
    }

    #[test]
    fn toml() {
        let toml =
            "[package]\nname = \"lorca\"\nversion = \"0.1.0\"\n\n[dependencies]\nropey = \"1.6\"\n";
        assert_eq!(guess(toml), Language::Toml);
        assert_eq!(guess("tema = \"Yerma\"\ntamaño = 14"), Language::Toml);
    }

    #[test]
    fn css() {
        let css = ".boton {\n  color: red;\n  padding: 4px 8px;\n}\n\n@media (max-width: 600px) {\n  .boton { display: none; }\n}\n";
        assert_eq!(guess(css), Language::Css);
    }

    #[test]
    fn bash() {
        assert_eq!(
            guess("cd /tmp && git clone https://github.com/x/y.git\ncargo build --release"),
            Language::Bash
        );
        let script =
            "for f in *.txt; do\n  echo \"$f\"\ndone\nif [ -d dir ]; then\n  rm -rf dir\nfi\n";
        assert_eq!(guess(script), Language::Bash);
    }

    #[test]
    fn markdown() {
        let md = "# Lorca\n\nEditor de texto.\n\n## Funciones\n\n- Pestañas\n- **Temas**\n\n```rust\nfn main() {}\n```\n\nMás en [la web](https://example.com).\n";
        assert_eq!(guess(md), Language::Markdown);
    }
}
