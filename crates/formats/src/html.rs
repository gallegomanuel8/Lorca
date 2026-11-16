//! Pegar HTML (copiado de una página web o de un procesador de textos) como Markdown, con
//! `htmd`: títulos, negrita, cursiva, tachado, listas, vínculos, tablas y código.

use htmd::element_handler::{HandlerResult, Handlers};
use htmd::options::{BulletListMarker, HrStyle, Options};
use htmd::{Element, HtmlToMarkdown};

/// Etiquetas con formato que el texto con formato sabe mostrar. Si el HTML no tiene
/// ninguna (p. ej. el que copian los editores de código: solo `div`, `span` y colores), se
/// pega el texto tal cual.
const FORMAT_TAGS: [&str; 17] = [
    "h1", "h2", "h3", "h4", "h5", "h6", "strong", "b", "em", "i", "ul", "ol", "a", "table", "del",
    "s", "strike",
];

/// Markdown equivalente a `html`, o `None` si no aporta formato (o no se pudo convertir).
pub fn html_to_markdown(html: &str) -> Option<String> {
    if !has_format_tags(html) {
        return None;
    }
    let converter = HtmlToMarkdown::builder()
        .skip_tags(vec![
            "script", "style", "head", "meta", "link", "img", "svg", "picture", "video", "audio",
            "iframe", "noscript", "button", "input", "form",
        ])
        // htmd no convierte el tachado.
        .add_handler(
            vec!["del", "s", "strike"],
            |handlers: &dyn Handlers, element: Element| {
                let inner = handlers.walk_children(element.node).content;
                let inner = inner.trim();
                Some(HandlerResult::from(if inner.is_empty() {
                    String::new()
                } else {
                    format!("~~{inner}~~")
                }))
            },
        )
        .options(Options {
            bullet_list_marker: BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ol_number_spacing: 1,
            hr_style: HrStyle::Dashes,
            ..Options::default()
        })
        .build();
    let markdown = converter.convert(html).ok()?;
    let markdown = tidy(&markdown);
    (!markdown.trim().is_empty()).then_some(markdown)
}

/// ¿Tiene `html` alguna etiqueta de [`FORMAT_TAGS`]?
fn has_format_tags(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    FORMAT_TAGS.iter().any(|tag| {
        let open = format!("<{tag}");
        lower.match_indices(&open).any(|(at, _)| {
            matches!(
                lower.as_bytes().get(at + open.len()),
                Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/')
            )
        })
    })
}

/// Cursiva con `*` (el texto con formato no reconoce `_`), sin espacios al final de las
/// líneas ni más de una línea en blanco seguida.
fn tidy(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut blank = 0;
    for line in markdown.trim().lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(&underscore_emphasis(line));
        out.push('\n');
    }
    out.trim_end().to_owned()
}

/// `_texto_` → `*texto*` cuando los guiones bajos rodean una palabra (no dentro de
/// `nombres_con_guiones` ni en el código entre comillas invertidas).
fn underscore_emphasis(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<char> = chars.clone();
    let mut in_code = false;
    let mut open: Option<usize> = None;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '`' => {
                in_code = !in_code;
                open = None;
            }
            '_' if !in_code => {
                let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
                let after = chars.get(i + 1).copied();
                let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
                match open {
                    None if !word(before) && after.is_some_and(|c| !c.is_whitespace()) => {
                        open = Some(i);
                    }
                    Some(start) if !word(after) && before.is_some_and(|c| !c.is_whitespace()) => {
                        if let Some(slot) = out.get_mut(start) {
                            *slot = '*';
                        }
                        if let Some(slot) = out.get_mut(i) {
                            *slot = '*';
                        }
                        open = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_web_formatting() {
        let html = "<meta charset='utf-8'><h2>Romance</h2><p>Verde <b>que</b> te <em>quiero</em> \
            <del>verde</del>, <a href=\"https://ejemplo.es\">viento</a>.</p>\
            <ul><li>uno</li><li>dos</li></ul><ol><li>tres</li></ol>\
            <img src=\"foto.png\"><script>alert(1)</script>";
        let markdown = html_to_markdown(html).unwrap();
        assert_eq!(
            markdown,
            "## Romance\n\nVerde **que** te *quiero* ~~verde~~, [viento](https://ejemplo.es).\n\n\
             - uno\n- dos\n\n1. tres"
        );
    }

    #[test]
    fn plain_html_is_pasted_as_text() {
        let html = "<div><span style=\"color:red\">let</span> x = a_b_c;</div>";
        assert_eq!(html_to_markdown(html), None);
        assert_eq!(html_to_markdown("<body>hola</body>"), None);
        // «<b» de «<br>» o «<body>» no cuenta como negrita.
        assert_eq!(html_to_markdown("uno<br>dos"), None);
    }

    #[test]
    fn underscores_become_asterisks_only_around_words() {
        assert_eq!(underscore_emphasis("_hola_ mundo"), "*hola* mundo");
        assert_eq!(underscore_emphasis("snake_case_name"), "snake_case_name");
        assert_eq!(underscore_emphasis("`_x_` y _y_"), "`_x_` y *y*");
    }
}
