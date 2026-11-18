# Lorca

<p align="center">
  <img src="assets/icons/retrato/lorca-256.png" width="128" alt="Retrato de Federico García Lorca dibujado con caracteres de código">
</p>

<p align="center"><em>Verde que te quiero verde.</em></p>

Lorca es un editor de texto y código de escritorio para macOS, Windows y Linux, escrito en
Rust con [iced](https://iced.rs). El nombre es un homenaje a Federico García Lorca: sus
temas de color llevan los títulos de sus obras.

Pensado para tomar notas y para programar. Las notas se escriben **viendo el formato**
(títulos, listas de tareas, imágenes…) aunque por debajo sean Markdown. El código tiene
resaltado de sintaxis, autocompletado y diagnósticos con servidores de lenguaje (LSP),
búsqueda al estilo Notepad++, multicursor y pantalla dividida. Y la sesión nunca pierde
lo que escribiste.

![Lorca con el tema Romancero gitano editando un programa en Rust](docs/capturas/principal.png)

## Funciones

### Notas con formato

Los documentos de **texto con formato** (Markdown, `.md`) se editan como en un procesador
de textos: las marcas (`**`, `#`, `- [ ]`…) no se ven y el texto se ajusta a la ventana.
El archivo sigue siendo Markdown, así que se abre igual en cualquier otro programa.

- **Barra de formato** como la del Bloc de notas de Windows 11: títulos, listas (viñetas,
  numeradas y **de tareas**), negrita, cursiva, tachado, vínculos, imágenes, tablas y
  borrar formato. Los botones se ven pulsados con el formato del cursor o de la selección.
- **Listas de tareas** con casillas que se marcan con un clic; Intro continúa la lista.
- **Imágenes**: se insertan con el botón, pegando o arrastrando archivos, y se ven en el
  texto. Se mueven arrastrándolas y se cambia su tamaño con el tirador de la esquina. Las
  grandes se guardan reducidas en una carpeta `<nombre>_imagenes/` junto al documento.
- **Bloques de código** con el resaltado de su lenguaje.
- **Pegar desde una web** o un procesador de textos conserva negritas, listas, títulos y
  vínculos.
- **Escribir Markdown** en una nota nueva (`# `, `- `, `**texto**`…) la convierte en texto
  con formato. El **texto plano** se ve tal cual; darle formato lo convierte.
- **Recuento de palabras** en la barra de estado (y de caracteres en su ayuda).
- **Exportar a PDF**, con las imágenes, y vista previa de Markdown junto al editor (⇧⌘V).
- Si prefieres ver las marcas: «Ver → Mostrar marcas de Markdown».

![Una nota con títulos, lista de tareas, lista numerada e imagen, con el tema Mariana Pineda](docs/capturas/texto-con-formato.png)

### Programar con ayuda del servidor de lenguaje

Lorca habla el *Language Server Protocol*: con un servidor instalado (rust-analyzer,
pyright, typescript-language-server…) ofrece **autocompletado** mientras escribes,
**información** del símbolo con ⌘I, **ir a la definición** con F12 o ⌘+clic y
**diagnósticos** subrayados, con el mensaje en la barra de estado y F8 para recorrerlos.

![Autocompletado de rust-analyzer: campos y métodos de una estructura con su tipo](docs/capturas/autocompletado.png)

![Información de un método mostrada con ⌘I](docs/capturas/informacion.png)

![Error de sintaxis subrayado y su mensaje en la barra de estado, con el tema Bodas de sangre](docs/capturas/diagnosticos.png)

### Edición

- **Pestañas** con indicador ● de cambios, reordenables arrastrando y con botón «+»;
  selección de varias con ⌘/Ctrl+clic para cerrarlas, guardarlas o verlas juntas. Las que
  no se han guardado llevan el comienzo de su texto, como en el Bloc de notas.
- **Pantalla dividida** de 2 a 4 documentos lado a lado (`⌘\`), cada uno con su cursor.
- **Multicursor**: ⌥+clic, ⌘D para la siguiente aparición, ⌥⌘↑/↓ en columna.
- **Deshacer por transacciones** (una palabra se deshace de una vez), números de línea,
  guías de indentación, espacios y finales de línea visibles.
- **Zoom por pestaña** con ⌘+/⌘−, ⌘+rueda o pellizcando el touchpad.
- **Paleta de órdenes** (⇧⌘P) con búsqueda difusa sobre todas las acciones.
- **Menú contextual** en el texto y las pestañas; **barra de menús** nativa en macOS y
  dibujada en la ventana en Windows y Linux.

![Pantalla dividida: código en Rust a la izquierda y un poema con formato a la derecha, con el tema Yerma](docs/capturas/dividida.png)

![Paleta de órdenes filtrando los temas](docs/capturas/paleta.png)

### Lenguajes

- **Resaltado** con tree-sitter para Rust, Python, JavaScript, TypeScript/TSX, JSON,
  YAML, TOML, HTML, CSS, Bash, SQL y Markdown (incluidos los bloques de código).
- **Detección** por extensión, nombre de archivo y shebang (y por el contenido al pegar
  en un documento sin lenguaje); selector manual en la barra de estado.
- **Formatear** JSON y SQL (⌥⌘L), minificar JSON, MAYÚSCULAS / minúsculas / Tipo Título.

### Búsqueda

- Barra de **buscar y reemplazar** con coincidencias resaltadas y contador, modos
  **Normal**, **Extendido** (`\n`, `\t`, `\xHH`…) y **Regex** con grupos (`$1`).
- **Buscar todo** en el documento, en todos los abiertos o **en una carpeta** (⇧⌘F,
  respeta `.gitignore`), con panel de resultados clicables.
- **Marcar** coincidencias y **marcadores** de línea (⌘F2, F2); **ir a línea** (⌃G).

![Barra de búsqueda con las coincidencias de «luna» resaltadas](docs/capturas/busqueda.png)

### Archivos y sesión

- Al cerrar y volver a abrir, Lorca restaura **pestañas, cursores, pantalla dividida y
  cambios sin guardar**, sin preguntar ni tocar los archivos originales.
- Abre y guarda UTF-8 (con o sin BOM), UTF-16 y Windows-1252, y conserva LF o CRLF; ambos
  se cambian desde la barra de estado.
- **Recarga** los archivos que otro programa cambia (pregunta si tienes cambios).
- **«Abrir con Lorca»** desde el Finder de macOS y el Explorador de Windows (si Lorca ya
  está abierto, el archivo se abre en una pestaña nueva).
- **Actualizaciones automáticas**: avisa cuando hay una versión nueva y, con un clic, la
  descarga, comprueba su firma, la instala y vuelve a abrir Lorca. También desde «Ayuda →
  Buscar actualizaciones…».

### Temas con nombres de obras de Lorca

Cinco oscuros —*Romancero gitano*, *Yerma*, *Bodas de sangre*, *Poeta en Nueva York* y
*La bola negra*— y cinco claros —*Mariana Pineda*, *La casa de Bernarda Alba*, *Doña
Rosita la soltera*, *La bola negra* y *Blanco y negro*—, más *Negro y blanco* (oscuro).
Los dos últimos no tienen ningún color. Se elige uno claro y otro oscuro, y el modo
automático sigue al sistema. Se cambian desde la barra de estado o la paleta, y puedes
crear los tuyos.

La primera vez, Lorca pregunta cómo quieres verlo y enseña un resumen de lo que puede
hacer (también en «Ayuda → Lo que puedes hacer con Lorca»).

![Pantalla de bienvenida: modo automático, claro (Mariana Pineda) u oscuro (Romancero gitano)](docs/capturas/bienvenida.png)

## Instalación

Cada versión deja sus paquetes en
[releases](https://github.com/gallegomanuel8/Lorca/releases):

| Sistema | Archivo |
|---|---|
| macOS (Apple Silicon) | `.dmg`, o el `.zip` portable con el `.app` |
| Windows | instalador `.exe` (en español) o `.msi` |
| Debian / Ubuntu | `.deb` |
| Fedora / openSUSE | `.rpm` |
| Otras distribuciones Linux | AppImage |

- El instalador `.exe` de Windows ofrece añadir «Abrir con Lorca» al menú contextual de
  los archivos y ofrecer Lorca en «Abrir con» de los archivos de texto.
- La app de macOS va firmada ad hoc, sin certificado de desarrollador: la primera vez
  macOS dirá que no puede verificar al desarrollador; ve a Ajustes del Sistema →
  Privacidad y seguridad y pulsa «Abrir igualmente».
- Lorca se actualiza solo (en Linux, solo el AppImage; con `.deb` o `.rpm`, instala el
  paquete nuevo).

Para el autocompletado y los diagnósticos instala el servidor de tu lenguaje, por
ejemplo `brew install rust-analyzer` o `rustup component add rust-analyzer`.

## Atajos

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
| Negrita · cursiva · vínculo (texto con formato) | ⌘B · ⌘I · ⌘K | Ctrl+B · Ctrl+I · Ctrl+K |
| Lista con viñetas · numerada | ⇧⌘8 · ⇧⌘7 | Ctrl+Mayús+8 · Ctrl+Mayús+7 |
| minúsculas · MAYÚSCULAS | ⌘U · ⇧⌘U | Ctrl+U · Ctrl+Mayús+U |
| Formatear JSON/SQL | ⌥⌘L | Ctrl+Alt+L |
| Tamaño de letra mayor · menor · normal | ⌘+ · ⌘− · ⌘0 | Ctrl++ · Ctrl+− · Ctrl+0 |
| Ajustes (`config.toml`) | ⌘, | Ctrl+, |
| Autocompletar | ⌃Espacio | Ctrl+Espacio |
| Información del símbolo (código) | ⌘I | Ctrl+I |
| Ir a la definición | F12 o ⌘+clic | F12 o Ctrl+clic |
| Siguiente / anterior problema | F8 / ⇧F8 | F8 / Mayús+F8 |

En macOS, ⌃Espacio suele estar asignado a «Seleccionar la fuente de entrada anterior»;
si no llega a Lorca, quítalo en Ajustes del Sistema → Teclado → Atajos, o usa el menú
Código.

## Configuración

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
show_markdown_markers = false  # ver las marcas de Markdown en vez del formato
wrap_markdown = true           # ajuste de línea al ver las marcas
check_updates = true           # buscar actualizaciones al arrancar

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

## Datos de usuario

| Sistema | Carpeta |
|---|---|
| macOS | `~/Library/Application Support/Lorca/` |
| Windows | `%APPDATA%\Lorca\` |
| Linux | `~/.config/lorca/` |

Contiene `config.toml` (preferencias), `themes/` (temas propios), `session.toml`
(pestañas), `backups/` (texto sin guardar) e `imagenes/` (las de las notas que aún no
se han guardado; al guardar pasan a la carpeta del documento). La variable de entorno `LORCA_DATA_DIR` permite usar otra carpeta.

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
cargo build --release && cargo generate-rpm -p crates/app   # .rpm
```

Los paquetes quedan en `target/packages/`. La app de macOS se firma ad hoc (la release usa
un certificado Developer ID y la notariza si el repositorio tiene los secretos de Apple;
ver `CLAUDE.md`). Los iconos están en `assets/icons/`.

### Integración continua

`.github/workflows/ci.yml` comprueba formato, clippy y tests en macOS, Windows y Linux en
cada push. Al publicar una etiqueta (`git tag v0.2.5 && git push origin v0.2.5`),
`release.yml` genera y firma los paquetes de los tres sistemas, los adjunta a una release
en borrador y crea el `latest.json` de las actualizaciones, que se ofrecen cuando la
release se publica.

## Arquitectura

Workspace de Cargo:

| Crate | Responsabilidad |
|---|---|
| `crates/core` (`lorca-core`) | Buffer (ropey), selección, ediciones, deshacer/rehacer, codificaciones y finales de línea. Sin GUI. |
| `crates/syntax` (`lorca-syntax`) | Detección de lenguaje y resaltado con tree-sitter. |
| `crates/search` (`lorca-search`) | Buscar y reemplazar: modos, opciones, navegación y sustitución. |
| `crates/formats` (`lorca-formats`) | Formato de Markdown (negrita, listas, tablas, imágenes), análisis del texto con formato, HTML a Markdown, formateadores JSON/SQL, mayúsculas, exportación a HTML y PDF. |
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
7. **Notas** ✅: texto con formato editado viendo el formato, barra de formato, imágenes,
   listas de tareas, bloques de código, pegar desde la web, recuento de palabras,
   actualizaciones automáticas y bienvenida con resumen de funciones.

## Licencia

MIT o Apache-2.0, a tu elección.
