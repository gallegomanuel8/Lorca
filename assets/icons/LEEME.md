# Iconos de Lorca

Copia esta carpeta en tu proyecto como `assets/icons/`.

| Archivo | Uso |
|---|---|
| `lorca.icns` | Icono de la aplicación en macOS (bundle `.app` / `.dmg`) |
| `lorca.ico` | Windows (ejecutable e instalador). Incluye 16, 24, 32, 48, 64, 128 y 256 px |
| `png/lorca-*.png` | Linux (AppImage, .deb) y el icono de la ventana en tiempo de ejecución |
| `macos/Lorca.icon` | Origen del icono (Icon Composer): degradado marfil y dos capas, renglones (delante, sin translucidez para que no se vea la luna a través) y luna |
| `macos/dibujar-capas.swift` | Dibuja las capas de `Lorca.icon` |
| `macos/generar-icns.sh` | Genera `lorca.icns`, `lorca.ico` y `png/` desde `Lorca.icon` (Xcode 26 o posterior) |
| `retrato/` | Retrato de Lorca hecho con caracteres: el README y «Acerca de» (no es el icono de la app) |

El icono de la aplicación es una luna llena roja con cuatro renglones de código sangrados
que la cruzan como nubes (la luna de Lorca: redonda en *Bodas de sangre*, «luna roja» en
«Canción de jinete»), en los colores del tema *Mariana Pineda*: tinta negra y rojo sobre
marfil. Para cambiarlo: edita `dibujar-capas.swift` (o el degradado
de `icon.json`) y ejecuta, desde `macos/`:

```sh
swift dibujar-capas.swift && sh generar-icns.sh
```

## macOS 26 y posteriores

macOS dibuja dentro de un cuadro claro los iconos `.icns` que no tienen su forma
(«squircle»). Por eso los iconos no se hacen a mano: `generar-icns.sh` compila `Lorca.icon`
con `actool`, pide al sistema que lo dibuje a 1024 px (ya con la forma y la sombra de
Apple) y saca de ahí el `.icns`, el `.ico` y los PNG.

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
