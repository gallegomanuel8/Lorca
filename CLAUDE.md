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
- `workspace`: `Workspace` (documentos por `DocumentId`, orden de pestañas, activa) y
  `Document` (editor, ruta, nombre «Sin título N», formato, lenguaje, `DiskStamp`).
  - `storage`: `data_dir()` (respeta `LORCA_DATA_DIR`), `write_atomic` (temporal + rename),
    `DiskStamp` (mtime + tamaño) para detectar cambios en disco.
  - `session`: `SessionStore` guarda `session.toml` y `backups/doc-N.txt` (solo documentos
    modificados; solo reescribe si cambió la revisión; borra respaldos huérfanos).
    `Workspace::restore` reabre de disco los no modificados y de respaldo los modificados
    (estos quedan con `forced_modified`). Formato versionado con `SESSION_VERSION`.
  - `config`: `Config` (`config.toml`), `ThemeMode` (auto/claro/oscuro). `theme = None`
    significa primer arranque.
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
  - Para probar sin tocar los datos reales: `LORCA_DATA_DIR=/tmp/lorca cargo run -p lorca`.

## Limitaciones conocidas (Fase 1)

- Columnas por carácter, no por grafema (emoji compuestos se recorren carácter a carácter).
- La barra de desplazamiento es solo indicador (no se arrastra).
- macOS: ⌘Q usa el menú por defecto de winit, que termina el proceso sin avisar a la app.
  Solo se pierde lo tecleado en los últimos ~300 ms. Solución prevista: menú nativo propio
  (crate `muda`) que enrute «Salir» por `Message::CloseRequested`.
- Dos instancias de Lorca a la vez comparten y sobrescriben la misma sesión.
- El historial de deshacer no se conserva entre sesiones.
- La búsqueda trabaja sobre una copia `String` del documento y se recalcula al editar
  (rápido gracias a la caché, pero no incremental).
- Los resultados de «Buscar todo» son una foto: si el documento cambia, el clic lleva a la
  línea guardada en vez de al rango exacto.
- Sin resaltado de sintaxis (Fase 2), ni búsqueda (Fase 3).
