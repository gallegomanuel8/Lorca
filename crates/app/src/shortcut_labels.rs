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
label!(FORMAT, "⌥⌘L", "Ctrl+Alt+L");
label!(UPPER, "⇧⌘U", "Ctrl+Mayús+U");
label!(LOWER, "⌘U", "Ctrl+U");
label!(PREVIEW, "⇧⌘V", "Ctrl+Mayús+V");
label!(NEW, "⌘N", "Ctrl+N");
label!(OPEN, "⌘O", "Ctrl+O");
label!(SAVE, "⌘S", "Ctrl+S");
label!(SAVE_AS, "⇧⌘S", "Ctrl+Mayús+S");
label!(REPLACE, "⌥⌘F", "Ctrl+H");
label!(FIND_IN_FOLDER, "⇧⌘F", "Ctrl+Mayús+F");
label!(ZOOM_IN, "⌘+", "Ctrl++");
label!(ZOOM_OUT, "⌘−", "Ctrl+−");
label!(ZOOM_RESET, "⌘0", "Ctrl+0");
label!(SETTINGS, "⌘,", "Ctrl+,");
label!(COMPLETE, "⌃Espacio", "Ctrl+Espacio");
label!(HOVER, "⌘I", "Ctrl+I");
label!(BOLD, "⌘B", "Ctrl+B");
label!(ITALIC, "⌘I", "Ctrl+I");
label!(BULLETS, "⇧⌘8", "Ctrl+Mayús+8");
label!(NUMBERED, "⇧⌘7", "Ctrl+Mayús+7");
label!(PALETTE, "⇧⌘P", "Ctrl+Mayús+P");
label!(FIND_NEXT, "⌘G", "F3");
label!(FIND_PREVIOUS, "⇧⌘G", "Mayús+F3");
label!(QUIT, "⌘Q", "Alt+F4");
pub const DEFINITION: &str = "F12";
pub const NEXT_PROBLEM: &str = "F8";
pub const PREVIOUS_PROBLEM: &str = "Mayús+F8";

/// Texto de «mostrar el archivo» en el gestor de archivos del sistema.
pub const REVEAL: &str = if cfg!(target_os = "macos") {
    "Mostrar en Finder"
} else if cfg!(windows) {
    "Mostrar en el Explorador"
} else {
    "Abrir la carpeta"
};
