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

### Datos de usuario

| Sistema | Carpeta |
|---|---|
| macOS | `~/Library/Application Support/Lorca/` |
| Windows | `%APPDATA%\Lorca\` |
| Linux | `~/.config/lorca/` |

Contiene `config.toml` (preferencias), `session.toml` (pestañas) y `backups/` (texto sin
guardar). La variable de entorno `LORCA_DATA_DIR` permite usar otra carpeta.

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
| Palabra anterior / siguiente | ⌥← / ⌥→ | Ctrl+← / Ctrl+→ |
| Inicio / fin de línea | ⌘← / ⌘→ | Inicio / Fin |

## Compilar y ejecutar

Requisitos: Rust estable (1.85 o posterior, edición 2024).

En Linux hacen falta además las bibliotecas de desarrollo de X11/Wayland y
`xdg-desktop-portal` para los diálogos nativos.

```bash
cargo run --release                     # abre un documento nuevo
cargo run --release -- notas.md main.rs # abre archivos en pestañas
cargo test --workspace                  # pruebas
```

## Arquitectura

Workspace de Cargo:

| Crate | Responsabilidad |
|---|---|
| `crates/core` (`lorca-core`) | Buffer (ropey), selección, ediciones, deshacer/rehacer, codificaciones y finales de línea. Sin GUI. |
| `crates/syntax` (`lorca-syntax`) | Detección de lenguaje y resaltado con tree-sitter. |
| `crates/search` (`lorca-search`) | Buscar y reemplazar: modos, opciones, navegación y sustitución. |
| `crates/formats` (`lorca-formats`) | Preview de Markdown y formateadores SQL/JSON (Fase 4). |
| `crates/workspace` (`lorca-workspace`) | Documentos abiertos, pestañas, guardado, sesión y configuración. |
| `crates/app` (`lorca`) | Binario con la GUI en iced: dibuja y traduce eventos a órdenes. |

## Hoja de ruta

1. **MVP** ✅: workspace, núcleo de edición, pestañas, guardar/abrir, atajos.
   - **1.5** ✅: sesión persistente con cambios sin guardar y tema claro/oscuro/automático.
2. **Lenguajes** ✅: detección por extensión y shebang, selector manual en la barra de
   estado, resaltado de sintaxis.
3. **Búsqueda** ✅: barra buscar/reemplazar con modos Normal/Extendido/Regex, resaltado de
   coincidencias, siguiente/anterior, reemplazar todo, buscar todo con panel de
   resultados, marcar y marcadores de línea; ir a línea.
4. **Formato**: caracteres invisibles, guías de indentación, conversión LF/CRLF, otras
   codificaciones, preview de Markdown en panel dividido, formatear SQL y JSON.
5. **Productividad**: multicursor, paleta de órdenes (⌘⇧P), búsqueda en carpeta,
   reordenar pestañas arrastrando, recarga si el archivo cambia en disco.
6. **Avanzado**: cliente LSP, temas («Yerma», «Bodas de sangre», «Romancero gitano»…),
   configuración en TOML, empaquetado y CI para los tres sistemas.

## Licencia

MIT o Apache-2.0, a tu elección.
