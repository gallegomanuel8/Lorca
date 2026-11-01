//! Textos de los atajos de teclado según la plataforma, para menús y avisos.

macro_rules! label {
    ($name:ident, $mac:literal, $other:literal) => {
        pub const $name: &str = if cfg!(target_os = "macos") {
            $mac
        } else {
            $other
        };
    };
}

label!(COMMAND, "⌘", "Ctrl");
label!(CUT, "⌘X", "Ctrl+X");
label!(COPY, "⌘C", "Ctrl+C");
label!(PASTE, "⌘V", "Ctrl+V");
label!(SELECT_ALL, "⌘A", "Ctrl+A");
label!(UNDO, "⌘Z", "Ctrl+Z");
label!(REDO, "⇧⌘Z", "Ctrl+Mayús+Z");
label!(FIND, "⌘F", "Ctrl+F");
label!(BOOKMARK, "⌘F2", "Ctrl+F2");
label!(GOTO, "⌃G", "Ctrl+G");
label!(CLOSE, "⌘W", "Ctrl+W");
label!(SPLIT, "⌘\\", "Ctrl+\\");

/// Texto de «mostrar el archivo» en el gestor de archivos del sistema.
pub const REVEAL: &str = if cfg!(target_os = "macos") {
    "Mostrar en Finder"
} else if cfg!(windows) {
    "Mostrar en el Explorador"
} else {
    "Abrir la carpeta"
};
