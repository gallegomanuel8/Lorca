# CLAUDE.md

Guía para Claude Code (y cualquier colaborador) en este repositorio.

## Proyecto

Lorca: editor de texto y código multiplataforma en Rust (iced + ropey). Se trabaja **por
fases** (ver README.md); no adelantes funcionalidades de fases futuras sin pedirlo.

## Comandos

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p lorca -- [ARCHIVOS...]
```

Los tres primeros deben pasar antes de cada commit. Un commit por paso completado, con
mensaje en español.

## Convenciones

- Comentarios y documentación en **español**; identificadores en **inglés**.
- Edición 2024. Dependencias siempre con `cargo add` (nunca inventar versiones).
- Lints del workspace (`Cargo.toml`): prohibidos `unwrap`, `expect`, `panic!`, `dbg!` y
  `todo!` fuera de los tests (`clippy.toml` los permite en tests). `unsafe` prohibido.
- Errores: un enum propio por crate con `thiserror`; `anyhow` solo en el binario.
- Los crates `core`, `syntax`, `search`, `formats` y `workspace` **no dependen de la GUI**
  y se prueban con `cargo test`. Toda lógica nueva va ahí con tests, no en `app`.

## Arquitectura

- `core`
  - `Buffer` (ropey): índices de **carácter**; el texto en memoria solo contiene **LF**.
    Ropey está compilado sin `unicode_lines`/`cr_lines`, así que solo `\n` separa líneas.
  - `TextFormat` (codificación + `LineEnding`) guarda el formato del archivo;
    `load_bytes`/`save_bytes` convierten entre bytes en disco y texto normalizado.
  - `EditorState` = buffer + `Selection` (ancla/cabeza) + `History`. Toda edición pasa por
    él y se registra como `Transaction` de `Edit::{Insert, Delete}`.
  - Agrupación de deshacer: ediciones consecutivas del mismo `EditKind` (Word, Space,
    Backspace, DeleteForward) y contiguas se fusionan. Cualquier cambio de selección
    llama a `History::seal()`. Sin temporizadores.
  - `History::revision()` identifica el estado del texto; `Document::is_modified` lo
    compara con la revisión guardada (deshacer hasta lo guardado = sin cambios).
  - `display`: columnas visuales (tabuladores, caracteres anchos con `unicode-width`).
- `syntax`
  - `Language`: `id`/`from_id` (sesión), `from_name` (alias de bloques de código Markdown),
    `detect(path, primera línea)` = extensión/nombre → shebang → texto plano.
  - `highlight(language, &str)` → `Highlights` (tramos `StyledSpan` por línea, columnas de
    carácter, sin solapes) con `tree-sitter-highlight`. Recalcula el documento entero (no
    incremental). `CAPTURES` mapea nombres de captura a `HighlightKind`; añadir ahí los
    nombres nuevos que traigan las gramáticas.
  - `Grammar`: configuraciones compiladas una vez (`OnceLock`) por gramática. TypeScript
    usa la gramática TSX con las consultas de JS + JSX + TS concatenadas.
  - Markdown: las inyecciones del bloque necesitan `injection.include-children` para
    `inline` y `code_fence_content` (`markdown_block_injections`); sin eso el texto
    inyectado llega troceado.
  - `examples/bench_highlight.rs` mide rendimiento (≈35 ms para 6 600 líneas de Rust en
    release).
- `search`
  - `SearchQuery::new(patrón, SearchOptions)`: todos los modos se compilan a `regex`
    (Normal = `regex::escape`, Extendido = `unescape_extended` + escape, Regex tal cual),
    siempre multilínea. CRLF/CR del patrón se convierten en LF (el buffer solo tiene LF).
  - Posiciones en **caracteres**. «Palabra completa» se comprueba mirando los caracteres
    vecinos (el crate `regex` no tiene lookaround).
  - `replacement()` / `replace_all()` expanden `$1`, `${n}` y `\1` (`regex_template`).
  - `next_match` / `previous_match` / `match_index` trabajan sobre rangos ordenados.
- `formats`: `format_json`/`minify_json` (serde_json con `preserve_order`), `format_sql`
  (sqlformat), `change_case`, `markdown_to_html` (pulldown-cmark; para exportar). La vista
  previa en pantalla usa el widget `markdown` de iced (que también usa pulldown-cmark).
- `core` (multicursor): `Cursors` = selecciones ordenadas y fusionadas con una principal;
  `selection()` es la principal y `selections()` todas. Toda edición pasa por
  `EditorState::edit_each` (un `Change` por cursor, una transacción). El historial guarda
  `Cursors` antes/después. `version()` cambia con cada edición (clave de cachés);
  `revision()` solo sirve para saber si coincide con lo guardado.
- `search`: también `search_folder` (crate `ignore`, respeta `.gitignore`, sin binarios
  ni > 5 MB) y `build_hits` (filas de resultados, compartidas con «Buscar todo»).
- `formats`: `markdown_to_pdf` con `genpdf` y fuentes del sistema (`PdfFonts::system`:
  Arial/Courier en macOS y Windows, DejaVu en Linux). Ejemplo `md2pdf`. El PDF incrusta
  las fuentes completas (≈6 MB).
- `workspace`: `Workspace` (documentos por `DocumentId`, orden de pestañas, activa) y
  `Document` (editor, ruta, nombre «Sin título N», formato, lenguaje, `DiskStamp`).
  - `storage`: `data_dir()` (respeta `LORCA_DATA_DIR`), `write_atomic` (temporal + rename),
    `DiskStamp` (mtime + tamaño) para detectar cambios en disco.
  - `session`: `SessionStore` guarda `session.toml` y `backups/doc-N.txt` (solo documentos
    modificados; solo reescribe si cambió la revisión; borra respaldos huérfanos).
    `Workspace::restore` reabre de disco los no modificados y de respaldo los modificados
    (estos quedan con `forced_modified`). Formato versionado con `SESSION_VERSION`.
  - `config`: `Config` (`config.toml`), `ThemeMode` (auto/claro/oscuro;
    `ThemeMode::appearance`). `theme = None` significa primer arranque. También
    `light_theme`/`dark_theme`, `font_size`, `tab_width`, `insert_spaces` (con métodos que
    los limitan a rangos válidos) y `LspConfig` (`[lsp]`, servidores por `Language::id`).
  - `theme`: `ThemeSpec` (TOML: `[palette]` de iced, `[editor]` opcional, `[syntax]` con
    claves `HighlightKind::name`), `Rgba` (`#rrggbb[aa]`), `ThemeSet` (incluidos con
    `include_str!` desde `crates/workspace/themes/` + `<datos>/themes/*.toml`;
    `resolve(nombre, aspecto)` cae al primero del aspecto; `create_personal` copia uno).
- `lsp` (Fase 6): cliente LSP sin GUI.
  - `Client::spawn` (proceso) o `Client::connect` (cualquier `Read`/`Write`; las pruebas
    usan `std::io::pipe` y un servidor falso). Hilo de escritura que retiene todo hasta la
    respuesta a `initialize` y luego envía `initialized`; hilo de lectura que empareja
    respuestas por id (`Pending`), contesta las peticiones del servidor y entrega `Event`
    por una función de la aplicación.
  - Sincronización siempre completa (`didChange` con todo el texto). Se negocia
    `positionEncodings` utf-32/utf-16; `PositionEncoding::{to_server, to_chars}` convierte
    columnas.
  - `servers`: servidores conocidos por `Language`, búsqueda en `PATH` + carpetas
    habituales (las apps del Finder no heredan el `PATH`), `find_root` (marcador más
    cercano; en Rust el `Cargo.toml` más externo sin salir del repositorio).
  - `snippet::expand` quita la sintaxis de snippet (se anuncia `snippetSupport: false`,
    pero algunos servidores los mandan igual).
- `app`
  - `app.rs`: estado `Lorca`, `Message`, `update`, `view`. Flujos de guardar/cerrar/salir
    encadenados con `Then` (`Nothing`, `CloseTab`, `ContinueQuit`).
  - `editor/`: widget propio (`iced::advanced::Widget`) que **solo lee** `EditorState` y
    publica `EditorAction`; `apply_action` las aplica al núcleo. El desplazamiento vive en
    `DocView` (por documento, en la app), no en el núcleo.
  - Rejilla monoespaciada: ancho de celda medido de `Font::MONOSPACE`.
  - `editor/keymap.rs`: teclas de edición que captura el widget. `shortcuts.rs`: atajos
    globales vía `keyboard::listen()` (solo reciben eventos **no capturados**, así que el
    widget no debe capturar Cmd+S, Cmd+Z, Ctrl+Tab…).
  - `dialogs.rs`: diálogos rfd asíncronos (`Task::perform`); `dialog_open` evita abrir dos.
  - Sesión: cualquier mensaje que la afecte marca `session_dirty`; un `time::every`
    (ejecutor smol) la guarda 300 ms tras el último cambio o cada 2 s como máximo, y al
    cerrar la ventana. Cerrar la app no pregunta; si no se puede guardar la sesión, vuelve
    al flujo de confirmación por documento (`quit_queue`). ⌘W sigue preguntando.
  - Tema: `resolve_theme(ThemeMode, system Mode)`; el modo del sistema llega con
    `system::theme()` al arrancar y `system::theme_changes()`. `welcome.rs` es la
    pantalla del primer arranque.
  - Resaltado: `schedule_highlight` (al final de cada `update`) lanza `Task::perform` con
    una copia del texto si la revisión o el lenguaje del documento activo cambiaron y no
    hay otro cálculo en marcha; `Message::Highlighted` guarda el resultado en
    `HighlightState`. El widget pinta con el último resultado aunque vaya una revisión por
    detrás. Colores en `editor/palette.rs` (provisional hasta los temas de la Fase 6).
  - `Document::refresh_language()` se llama tras cada edición (detecta shebangs al
    teclear); `set_language` lo fija a mano y se guarda en la sesión.
  - Búsqueda: `find.rs` (estado `FindState`, vistas de la barra, «Ir a línea» y panel de
    resultados) y `find_actions.rs` (submódulo de `app` con las acciones). Las
    coincidencias se cachean por (documento, revisión, patrón, opciones, ámbito) en
    `MatchCache`; `refresh_search` se llama al final de cada `update`. «Marcar» guarda la
    consulta por documento y se reevalúa en vivo. El widget recibe `Decorations`.
  - Foco: `Focus` (Editor/FindBar/GoTo). El editor ignora teclas ya capturadas
    (`shell.is_event_captured()`). Los atajos usan `event::listen_with` (también ven
    eventos capturados: Escape, F2, F3…).
  - `key_filter.rs`: en macOS el `text_input` de iced escribe aunque se pulse ⌘ (⌘G → «g»,
    ⌥⌘F → «ƒ»); `filter_command_keys` envuelve los campos de la barra para evitarlo.
  - Saltos (buscar, ir a línea, resultados, marcadores) usan `DocView::jump_to_cursor`,
    que deja contexto; al cambiar el tamaño del editor se mantiene visible el cursor.
  - Pantalla dividida: `Workspace::panes()` (2–4 ids, el activo siempre dentro) es el
    modelo; `panes.rs` (submódulo de `app`) lo refleja en un `pane_grid` con anchos iguales
    (`PaneLayout`) y lo sincroniza con `sync_panes` tras cada `update`. Cada documento
    tiene su `DocView` con su propio `Viewport`; los mensajes del editor llevan el
    `DocumentId` (`Message::Editor(id, acción)`). El resaltado se lanza para todos los
    documentos visibles.
  - Menús contextuales: `context_menu.rs` dibuja el menú con `stack` + `pin` sobre un
    fondo que lo cierra al hacer clic fuera; `context_area` es un envoltorio que publica
    el clic derecho con su posición. Los elementos y acciones están en `panes.rs`
    (`menu_items`, `handle_context`). Textos de atajos por plataforma en
    `shortcut_labels.rs`.
  - Cerrar varias pestañas usa `close_queue` con `Then::CloseAndContinue`.
  - Fase 4: `ViewOptions` (espacios, finales de línea, guías) llega al widget desde
    `Config`; las marcas se dibujan con quads (exactas en la rejilla) y las etiquetas
    LF/CRLF con texto. Los menús de la barra de estado usan `click_area` y los destinos
    `MenuTarget::{Encoding, LineEnding, View}`. Formatear/mayúsculas usan
    `replace_ranges` (una transacción). La vista previa (`previews`, `preview_cache` por
    revisión) se muestra junto al editor en `editor_for`.
  - Atajos y teclado en macOS: el editor no escribe nunca con ⌘ pulsado (⌥⌘L daría «¬») y
    solo trata ⌘C/X/V/A sin Mayús (⇧⌘V es la vista previa).
  - Fase 5: `palette.rs` (búsqueda difusa, `palette_commands` en `panes.rs`),
    `watcher.rs` (notify, vigila carpetas; `Message::FileChanged`), `menu.rs` (muda en
    macOS, instalado con `Task::done(Message::InstallMenu)`; en Windows y Linux `bar` +
    `BAR`, desplegables como `MenuTarget::MenuBar` y `ContextAction::MenuCommand(id)`),
    arrastre de pestañas con
    `mouse_area` + `Message::MouseReleased` global.
  - Fase 6:
    - `theme.rs`: `ActiveTheme` (tema de iced + `EditorColors`), reconstruido en
      `refresh_theme` al final de cada `update` solo si cambió el aspecto o el nombre.
      `editor/palette.rs`: `EditorColors::new(spec, theme)` resuelve los colores del editor
      y del resaltado; el widget los recibe con `.colors(...)`.
    - `Metrics` (tamaño de letra, alto de línea, tabulador) viaja en `ViewOptions` y en
      `Viewport`; el widget vuelve a medir la celda si cambia el tamaño.
    - `watcher::Watched` vigila archivos y carpetas: `config.toml` y `themes/` se recargan
      solos (`reload_config`, `reload_themes`).
    - `lsp_actions.rs` (submódulo de `app`): `LspState`. `sync_lsp` (al final de cada
      `update`) abre/actualiza/cierra documentos en los servidores; un servidor por
      (orden, raíz); las órdenes que fallan van a `broken` hasta «LSP: Reiniciar
      servidores». Antes de cada petición se sincroniza el documento para que el servidor
      vea el último texto. Diagnósticos convertidos a caracteres por (versión,
      generación) en `refresh_diagnostics`.
    - `lsp_view.rs`: suscripción `events()` (manda primero `LspConnected` con el canal
      que usan los clientes) y desplegables (`stack` + `pin`) colocados con el punto que
      publica el widget (`EditorAction::CursorPoint`). Con el desplegable abierto el widget
      envía ↑/↓/Intro/Tab como `EditorAction::Completion`.
  - Probar la interfaz con osascript/clics simulados **roba el foco** al usuario: avisar
    antes y no hacerlo mientras pueda estar usando el ordenador.
  - Para probar sin tocar los datos reales: `LORCA_DATA_DIR=/tmp/lorca cargo run -p lorca`.

## Empaquetado y CI

- `cargo packager --release` desde `crates/app` (configuración en su `Cargo.toml`);
  salida en `target/packages/`. Iconos en `assets/icons/` (ver su `LEEME.md`):
  `lorca.icns` para el bundle, `lorca.ico` incrustado en el `.exe` por `build.rs`
  (`winresource`, solo Windows) y `png/lorca-256.png` como icono de la ventana
  (feature `image` de iced). Si `create-dmg` falla con «Recurso ocupado», queda montado
  un `/Volumes/Lorca` de un intento anterior: `hdiutil detach`.
- Instalador de Windows (NSIS, en español): plantilla propia `crates/app/windows/installer.nsi`
  (copia de la de cargo-packager; cambios marcados con «Lorca:»). Añade una página de
  componentes con «Abrir con Lorca» en el menú contextual de los archivos
  (`HKCUSoftwareClasses*shellLorca`; en Windows 11, en «Mostrar más opciones»).
- `release.yml` adjunta también un portable de macOS: el `.app` comprimido con `ditto`
  (`Lorca_<versión>_<arquitectura>-portable.zip`).
- `.github/workflows/ci.yml` (fmt, clippy, tests en los tres sistemas) y `release.yml`
  (paquetes al publicar una etiqueta `v*`).
- Código solo para macOS: lo que no se use fuera necesita
  `#[cfg_attr(not(target_os = "macos"), allow(dead_code))]` o la CI de Windows/Linux falla.

## Limitaciones conocidas

- Columnas por carácter, no por grafema (emoji compuestos se recorren carácter a carácter).
- La barra de desplazamiento es solo indicador (no se arrastra).
- Windows y Linux no tienen barra de menús nativa (muda exige `unsafe` en Windows y GTK
  en Linux): se dibuja una con iced (`menu::bar`), sin teclas Alt ni paso de un menú a
  otro al pasar el ratón.
- Dos instancias de Lorca a la vez comparten y sobrescriben la misma sesión.
- La sesión solo guarda el cursor principal (no todos los del multicursor).
- El historial de deshacer no se conserva entre sesiones.
- La vista previa de Markdown no sincroniza el desplazamiento con el editor ni se guarda en
  la sesión.
- La búsqueda trabaja sobre una copia `String` del documento y se recalcula al editar
  (rápido gracias a la caché, pero no incremental).
- Los resultados de «Buscar todo» son una foto: si el documento cambia, el clic lleva a la
  línea guardada en vez de al rango exacto.
- LSP: sin información al pasar el ratón (solo ⌘I), sin renombrar, referencias, acciones de
  código ni formateo por servidor; los snippets se insertan como texto con el cursor en el
  primer hueco. Solo archivos guardados. Sincroniza el documento completo en cada cambio.
- Guardar preferencias desde la interfaz reescribe `config.toml` y borra sus comentarios.
- Los paquetes no están firmados ni notarizados. Abrir un archivo desde el Finder («Abrir
  con») no lo carga: iced no recibe ese evento de macOS.
