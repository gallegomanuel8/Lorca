# Lorca

> *Verde que te quiero verde.*

Lorca es un editor de texto y código de escritorio para Windows, macOS y Linux, escrito en
Rust. El nombre es un homenaje a Federico García Lorca.

Pensado para tomar notas y programar: resaltado de muchos lenguajes, búsqueda y sustitución
con expresiones regulares, visualización de espacios, finales de línea y codificaciones, y
gestión de documentos en pestañas.

## Estado

**Fase 1 (MVP) completada:**

- Pestañas horizontales con indicador ● de cambios sin guardar, botón de cerrar y clic
  central para cerrar.
- Editor con números de línea, fuente monoespaciada, selección con teclado y ratón
  (doble clic: palabra; triple clic: línea), portapapeles e IME.
- Deshacer/rehacer por transacciones: una palabra tecleada seguida se deshace de una vez.
- Barra de estado con línea:columna, lenguaje (por extensión), codificación y LF/CRLF.
- Abre UTF-8, UTF-8 con BOM, UTF-16 LE/BE y Windows-1252, y guarda conservando la
  codificación y los finales de línea originales.
- Confirmación al cerrar una pestaña con cambios sin guardar.

**Fase 1.5 completada:**

- **Sesión persistente**: al cerrar y volver a abrir, Lorca restaura las pestañas, la activa,
  el cursor, el desplazamiento y los cambios sin guardar (incluidos los «Sin título»). Los
  archivos originales no se tocan: el texto pendiente se guarda en copias de respaldo y la
  app sale sin preguntar. Avisa si un archivo cambió o desapareció mientras estaba cerrada.
- **Tema claro, oscuro o automático** (sigue al sistema en vivo). Se elige en la pantalla
  de bienvenida del primer arranque y se cambia desde la barra de estado.

**Fase 2 completada:**

- **Resaltado de sintaxis** con tree-sitter para Markdown, SQL, Rust, Python, JavaScript,
  TypeScript (incluido TSX), JSON, YAML, TOML, HTML, CSS y Bash, con colores adaptados al
  tema claro u oscuro. Los bloques de código de Markdown (```` ```rust ````) y el JS/CSS
  dentro de HTML se resaltan con su propio lenguaje.
- **Detección del lenguaje** por extensión, nombre de archivo y shebang
  (`#!/usr/bin/env python3`), también mientras escribes en un documento sin título.
- **Selector manual de lenguaje** en la barra de estado; la elección se recuerda entre
  sesiones.
- El resaltado se calcula en segundo plano para no frenar la escritura en archivos grandes
  (por encima de 4 MB se muestra sin colores).

**Fase 3 completada** (búsqueda al estilo Notepad++):

- **Barra de búsqueda y sustitución** encima del texto, con las coincidencias resaltadas en
  vivo y contador «3 de 17».
- **Modos** Normal, **Extendido** (`\n`, `\t`, `\0`, `\xHH`, `\uHHHH`) y **Regex** (con
  «. = \n»); mayúsculas/minúsculas, palabra completa, **en selección** y dar la vuelta.
- **Reemplazar** y **Reemplazar todo** (se deshace de una vez). En modo regex, grupos con
  `$1`, `${nombre}` o `\1`.
- **Contar** y **Buscar todo** en el documento o en **todos los abiertos**, con un panel de
  resultados abajo: clic en una línea para ir a ella.
- **Marcar**: resalta todas las coincidencias y, opcionalmente, pone un **marcador** en cada
  línea. Los marcadores (punto en el margen) se mueven con el texto; se pueden recorrer,
  copiar o borrar sus líneas.
- **Ir a línea** (`línea` o `línea:columna`).

**Fase 3.5 completada:**

- **Menú contextual** (clic derecho) sobre el texto —cortar, copiar, pegar, deshacer, buscar,
  marcar, marcadores, ir a línea— y sobre las pestañas —cerrar, cerrar las demás o las de
  la derecha, copiar la ruta, mostrar en Finder/Explorador, dividir—.
- **Selección de varias pestañas** con ⌘/Ctrl+clic (o Mayús+clic para un rango): cerrar o
  guardar las seleccionadas.
- **Pantalla dividida**: con 2 a 4 pestañas seleccionadas, «Dividir pantalla» (⌘\) las
  muestra lado a lado (si hay más de 4, las 4 primeras elegidas). Cada panel tiene su
  cursor y desplazamiento, los separadores se arrastran y la división se recuerda entre
  sesiones.

**Fase 4 completada:**

- **Caracteres invisibles**: espacios (·), tabuladores (→) y etiquetas de fin de línea
  (LF/CRLF) como en Notepad++; **guías de indentación**. Se activan con «¶» en la barra de
  estado y se recuerdan en `config.toml`.
- **Finales de línea**: clic en «LF/CRLF» de la barra de estado para convertir al guardar.
- **Codificaciones**: clic en «UTF-8» para guardar con otra codificación (UTF-8, UTF-8 con
  BOM, UTF-16 LE/BE, Windows-1252) o reabrir el archivo interpretándolo con otra.
- **Formatear** JSON (conserva el orden de las claves) y SQL, del documento o de la
  selección; **minificar JSON**. Si el JSON no es válido, el cursor va al error.
- **MAYÚSCULAS / minúsculas / Tipo Título** de la selección.
- **Vista previa de Markdown** al lado del editor, actualizada en vivo (enlaces clicables).

**Fase 5 completada:**

- **Exportar a PDF** la vista previa de Markdown (botón en su cabecera o menú contextual),
  con las fuentes del sistema.
- **Multicursor**: ⌥+clic añade un cursor, ⌘D selecciona la palabra y añade la siguiente
  aparición, ⌥⌘↑/↓ añade cursores en columna, Esc vuelve a uno. Copiar y pegar reparten
  una línea por cursor.
- **Paleta de órdenes** (⇧⌘P) con búsqueda difusa sobre todas las acciones.
- **Buscar en carpeta** (⇧⌘F): respeta `.gitignore`, ignora binarios; resultados
  clicables en el panel.
- **Reordenar pestañas** arrastrándolas.
- **Recarga automática** si otro programa cambia un archivo abierto (pregunta si tienes
  cambios sin guardar; avisa si se borró).
- **Barra de menús nativa en macOS** (Archivo, Editar, Buscar, Ver, Ventana); ⌘Q guarda la
  sesión antes de salir.

**Fase 6 completada:**

- **Temas con nombres de obras de Lorca**: oscuros *Romancero gitano*, *Yerma*, *Bodas de
  sangre* y *Poeta en Nueva York*; claros *Mariana Pineda*, *La casa de Bernarda Alba* y
  *Doña Rosita la soltera*. Se elige un tema claro y otro oscuro; el modo automático
  alterna entre ellos. Selector en la barra de estado y en la paleta («Tema: …»).
- **Temas propios** en `themes/*.toml` (ver abajo); «Crear tema a partir del actual» copia
  el tema activo para editarlo, y se recarga al guardar.
- **Configuración en `config.toml`**: tamaño de letra (⌘+ / ⌘− / ⌘0), tabulador, espacios
  o tabuladores, invisibles, temas y servidores de lenguaje. ⌘, la abre; los cambios se
  aplican al guardar.
- **Cliente LSP**: diagnósticos subrayados (mensaje en la barra de estado, F8 para
  recorrerlos), autocompletado al escribir o con ⌃Espacio, información con ⌘I e ir a la
  definición con F12 o ⌘+clic. Menú **Código** en macOS.
- **Paquetes** `.app`/`.dmg`, `.msi`/`.exe` y `.deb`/AppImage con cargo-packager, y **CI**
  en GitHub Actions para los tres sistemas.

### Configuración

`config.toml` (en la carpeta de datos, abajo) se crea al cambiar cualquier preferencia.
Todos los campos son opcionales:

```toml
theme = "auto"                 # "auto", "light" o "dark"
light_theme = "Mariana Pineda"
dark_theme = "Romancero gitano"
font_size = 14                 # de 8 a 40
tab_width = 4                  # de 1 a 16
insert_spaces = true           # la tecla Tab inserta espacios (false: un tabulador)
show_whitespace = false
show_line_endings = false
indent_guides = true

[lsp]
enabled = true
auto_completion = true         # false: solo con ⌃Espacio

# Sustituye al servidor conocido de un lenguaje (rust, python, javascript,
# typescript, json, yaml, toml, html, css, bash, markdown).
[lsp.servers.python]
command = "pylsp"
args = []

[lsp.servers.markdown]
enabled = false                # ningún servidor para Markdown
```

Lorca guarda el archivo al cambiar preferencias desde la interfaz, así que **los
comentarios que añadas se pierden**.

### Temas propios

Un tema es un `.toml` en `themes/`, dentro de la carpeta de datos. Si se llama igual que
uno incluido, lo sustituye. Lo más fácil es partir de «Crear tema a partir del actual».

```toml
name = "Llanto por Ignacio"
appearance = "dark"            # "dark" o "light"
description = "A las cinco de la tarde"

[palette]                      # interfaz (botones, paneles, campos)
background = "#16120f"
text = "#e8dccb"
primary = "#c9a227"
success = "#7fa37a"
warning = "#e0a95a"
danger = "#d0443a"

[editor]                       # opcionales; se derivan de la paleta
selection = "#c9a22755"        # #rrggbbaa admite transparencia
cursor = "#f2d16b"
current_line = "#201a15"
# gutter, line_number, line_number_active, find, find_current, mark,
# bookmark, whitespace, guide, error, warning, info

[syntax]                       # un color o { color, bold, italic }
keyword = { color = "#c9a227", bold = true }
string = "#a8b87a"
comment = { color = "#76695b", italic = true }
# function, type, escape, number, constant, property, variable, operator,
# punctuation, tag, attribute, heading, emphasis, strong, link, code
```

### Servidores de lenguaje

Lorca usa el primer servidor instalado de esta lista para cada lenguaje (o el de
`[lsp.servers]`):

| Lenguaje | Servidor |
|---|---|
| Rust | `rust-analyzer` |
| Python | `pyright-langserver`, `basedpyright-langserver` o `pylsp` |
| JavaScript / TypeScript | `typescript-language-server` |
| JSON, HTML, CSS | `vscode-json-language-server`, `vscode-html-language-server`, `vscode-css-language-server` |
| YAML · TOML · Bash · Markdown | `yaml-language-server` · `taplo` · `bash-language-server` · `marksman` |

Se buscan en el `PATH` y en las carpetas habituales (`~/.cargo/bin`, `/opt/homebrew/bin`,
`/usr/local/bin`, `~/.local/bin`…), porque una app abierta desde el Finder no hereda el
`PATH` de la terminal. Solo funcionan con archivos guardados (el servidor necesita la
ruta). Por ejemplo, `brew install rust-analyzer` o `rustup component add rust-analyzer`.

### Datos de usuario

| Sistema | Carpeta |
|---|---|
| macOS | `~/Library/Application Support/Lorca/` |
| Windows | `%APPDATA%\Lorca\` |
| Linux | `~/.config/lorca/` |

Contiene `config.toml` (preferencias), `themes/` (temas propios), `session.toml`
(pestañas) y `backups/` (texto sin guardar). La variable de entorno `LORCA_DATA_DIR` permite usar otra carpeta.

### Atajos

| Acción | macOS | Windows / Linux |
|---|---|---|
| Nuevo | ⌘N | Ctrl+N |
| Abrir | ⌘O | Ctrl+O |
| Guardar / Guardar como | ⌘S / ⌘⇧S | Ctrl+S / Ctrl+Mayús+S |
| Cerrar pestaña | ⌘W | Ctrl+W |
| Deshacer / Rehacer | ⌘Z / ⌘⇧Z | Ctrl+Z / Ctrl+Mayús+Z (o Ctrl+Y) |
| Pestaña siguiente / anterior | ⌃Tab / ⌃⇧Tab | Ctrl+Tab / Ctrl+Mayús+Tab |
| Copiar / Cortar / Pegar / Seleccionar todo | ⌘C / ⌘X / ⌘V / ⌘A | Ctrl+C / X / V / A |
| Buscar / Reemplazar | ⌘F / ⌥⌘F | Ctrl+F / Ctrl+H |
| Siguiente / anterior | F3 / ⇧F3 (o ⌘G / ⇧⌘G) | F3 / Mayús+F3 |
| Buscar la palabra del cursor | ⌘F3 | Ctrl+F3 |
| Ir a línea | ⌃G | Ctrl+G |
| Poner/quitar marcador · siguiente · anterior | ⌘F2 · F2 · ⇧F2 | Ctrl+F2 · F2 · Mayús+F2 |
| Cerrar barra de búsqueda | Esc | Esc |
| Seleccionar varias pestañas | ⌘+clic / ⇧+clic | Ctrl+clic / Mayús+clic |
| Dividir pantalla / quitar división | ⌘\ (⌘ç en teclado español) | Ctrl+\ |
| Palabra anterior / siguiente | ⌥← / ⌥→ | Ctrl+← / Ctrl+→ |
| Inicio / fin de línea | ⌘← / ⌘→ | Inicio / Fin |
| Paleta de órdenes | ⇧⌘P | Ctrl+Mayús+P |
| Buscar en carpeta | ⇧⌘F | Ctrl+Mayús+F |
| Añadir cursor · siguiente aparición · en columna | ⌥+clic · ⌘D · ⌥⌘↑/↓ | Alt+clic · Ctrl+D · Ctrl+Alt+↑/↓ |
| Vista previa de Markdown | ⇧⌘V | Ctrl+Mayús+V |
| Formatear JSON/SQL | ⌥⌘L | Ctrl+Alt+L |
| Tamaño de letra mayor · menor · normal | ⌘+ · ⌘− · ⌘0 | Ctrl++ · Ctrl+− · Ctrl+0 |
| Ajustes (`config.toml`) | ⌘, | Ctrl+, |
| Autocompletar | ⌃Espacio | Ctrl+Espacio |
| Información del símbolo | ⌘I | Ctrl+I |
| Ir a la definición | F12 o ⌘+clic | F12 o Ctrl+clic |
| Siguiente / anterior problema | F8 / ⇧F8 | F8 / Mayús+F8 |

En macOS, ⌃Espacio suele estar asignado a «Seleccionar la fuente de entrada anterior»;
si no llega a Lorca, quítalo en Ajustes del Sistema → Teclado → Atajos, o usa el menú
Código.

## Compilar y ejecutar

Requisitos: Rust estable (1.85 o posterior, edición 2024).

En Linux hacen falta además las bibliotecas de desarrollo de X11/Wayland y
`xdg-desktop-portal` para los diálogos nativos.

```bash
cargo run --release                     # abre un documento nuevo
cargo run --release -- notas.md main.rs # abre archivos en pestañas
cargo test --workspace                  # pruebas
```

### Paquetes

Con [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)
(`cargo install cargo-packager --locked`):

```bash
cd crates/app
cargo packager --release                     # formatos por defecto de la plataforma
cargo packager --release --formats app,dmg   # macOS
cargo packager --release --formats nsis,wix  # Windows (.exe y .msi)
cargo packager --release --formats deb,appimage
```

Los paquetes quedan en `target/packages/`. No están firmados: en macOS, la primera vez
abre la app con clic derecho → Abrir. Los iconos están en `assets/icons/`.

### Integración continua

`.github/workflows/ci.yml` comprueba formato, clippy y tests en macOS, Windows y Linux en
cada push. Al publicar una etiqueta (`git tag v0.1.0 && git push --tags`),
`release.yml` genera los paquetes de los tres sistemas y los adjunta a una release en
borrador.

## Arquitectura

Workspace de Cargo:

| Crate | Responsabilidad |
|---|---|
| `crates/core` (`lorca-core`) | Buffer (ropey), selección, ediciones, deshacer/rehacer, codificaciones y finales de línea. Sin GUI. |
| `crates/syntax` (`lorca-syntax`) | Detección de lenguaje y resaltado con tree-sitter. |
| `crates/search` (`lorca-search`) | Buscar y reemplazar: modos, opciones, navegación y sustitución. |
| `crates/formats` (`lorca-formats`) | Formateadores JSON/SQL, mayúsculas, Markdown a HTML y a PDF. |
| `crates/workspace` (`lorca-workspace`) | Documentos abiertos, pestañas, guardado, sesión, configuración y temas. |
| `crates/lsp` (`lorca-lsp`) | Cliente del Language Server Protocol. Sin GUI. |
| `crates/app` (`lorca`) | Binario con la GUI en iced: dibuja y traduce eventos a órdenes. |

## Hoja de ruta

1. **MVP** ✅: workspace, núcleo de edición, pestañas, guardar/abrir, atajos.
   - **1.5** ✅: sesión persistente con cambios sin guardar y tema claro/oscuro/automático.
2. **Lenguajes** ✅: detección por extensión y shebang, selector manual en la barra de
   estado, resaltado de sintaxis.
3. **Búsqueda** ✅: barra buscar/reemplazar con modos Normal/Extendido/Regex, resaltado de
   coincidencias, siguiente/anterior, reemplazar todo, buscar todo con panel de
   resultados, marcar y marcadores de línea; ir a línea.
   - **3.5** ✅: menú contextual, selección de varias pestañas y pantalla dividida (2–4).
4. **Formato** ✅: caracteres invisibles, guías de indentación, conversión LF/CRLF, otras
   codificaciones, vista previa de Markdown, formatear SQL y JSON, mayúsculas/minúsculas.
5. **Productividad** ✅: multicursor, paleta de órdenes, búsqueda en carpeta, reordenar
   pestañas arrastrando, recarga si el archivo cambia en disco, menús nativos (macOS) y
   exportar Markdown a PDF.
6. **Avanzado** ✅: cliente LSP, temas («Yerma», «Bodas de sangre», «Romancero gitano»…),
   configuración en TOML, empaquetado y CI para los tres sistemas.

## Licencia

MIT o Apache-2.0, a tu elección.
