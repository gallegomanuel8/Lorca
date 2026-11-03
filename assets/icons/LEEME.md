# Iconos de Lorca

Copia esta carpeta en tu proyecto como `assets/icons/`.

| Archivo | Uso |
|---|---|
| `lorca.ico` | Windows (ejecutable e instalador). Incluye 16, 24, 32, 48, 64, 128 y 256 px |
| `lorca.icns` | macOS (bundle `.app` / `.dmg`). Con el margen de la plantilla de Apple |
| `png/lorca-*.png` | Linux (AppImage, .deb) y el icono de la ventana en tiempo de ejecución |
| `png/lorca-macos-1024.png` | Original 1024 px con margen macOS, por si regeneras el `.icns` |
| `macos/Lorca.icon` | Origen del icono de macOS en formato de Icon Composer (capa `retrato.png` sobre fondo negro) |
| `macos/generar-icns.sh` | Regenera `lorca.icns` desde `Lorca.icon` (necesita Xcode 26 o posterior) |

## macOS 26 y posteriores

macOS dibuja dentro de un cuadro claro los iconos `.icns` que no tienen su forma
(«squircle»). Por eso `lorca.icns` no se hace a mano: `macos/generar-icns.sh` compila
`Lorca.icon` con `actool`, pide al sistema que lo dibuje a 1024 px (ya con la forma y la
sombra de Apple) y monta el `.icns` con todos los tamaños. Tras cambiar el retrato, vuelve a
ejecutarlo.

## 1. Icono de la ventana (iced)

Activa la feature `image` de iced en `crates/app/Cargo.toml`:

```toml
iced = { version = "*", features = ["image"] }  # usa la versión que ya tengas
```

Y al crear la aplicación:

```rust
let icon = iced::window::icon::from_file_data(
    include_bytes!("../../../assets/icons/png/lorca-256.png"),
    None,
).ok();

// en la configuración de la ventana:
iced::window::Settings { icon, ..Default::default() }
```

## 2. Icono del ejecutable en Windows

```toml
# crates/app/Cargo.toml
[target.'cfg(windows)'.build-dependencies]
winresource = "*"
```

```rust
// crates/app/build.rs
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icons/lorca.ico");
        res.compile().expect("no se pudo incrustar el icono");
    }
}
```

## 3. Instaladores (cargo-packager)

```toml
# crates/app/Cargo.toml
[package.metadata.packager]
product-name = "Lorca"
identifier = "dev.lorca.editor"
icons = [
  "../../assets/icons/png/lorca-32.png",
  "../../assets/icons/png/lorca-128.png",
  "../../assets/icons/png/lorca-256.png",
  "../../assets/icons/png/lorca-512.png",
  "../../assets/icons/lorca.icns",
  "../../assets/icons/lorca.ico",
]
```

## Nota sobre tamaños pequeños

A 16 y 32 px el retrato se percibe como una mancha clara sobre negro. Si quieres más
nitidez en la barra de tareas, sustituye `png/lorca-16.png` y `png/lorca-32.png`
por una versión simplificada (por ejemplo, solo el tupé y un ojo) y regenera el `.ico`.
