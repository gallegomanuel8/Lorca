//! «Abrir con» y doble clic en el Finder (solo macOS).
//!
//! macOS no pasa los archivos como argumentos: envía a la aplicación el evento «abrir
//! documentos», que AppKit traduce en `-[delegado application:openURLs:]`. iced (winit) no
//! implementa ese método, así que macOS decía que Lorca «no puede abrir archivos en ese
//! formato». Aquí se añade el método en tiempo de ejecución y las rutas se entregan como
//! las de otra ejecución (`instance::deliver`): se abren en pestañas y la ventana pasa al
//! frente.
//!
//! Es el **único módulo con `unsafe`** del proyecto (el resto lo tiene prohibido): hace
//! falta para hablar con el runtime de Objective-C.
//!
//! El método se añade a `NSObject` antes de arrancar la aplicación: así lo tiene ya el
//! delegado que crea winit cuando llega el evento del arranque (si se abre Lorca con un
//! archivo). Solo AppKit lo llama, y solo sobre el delegado de la aplicación.

#![allow(unsafe_code)]

use std::path::PathBuf;

use objc2::ffi;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;
use objc2_foundation::{NSArray, NSURL};

/// Implementación de `application:openURLs:` (`v@:@@`: no devuelve nada; recibe el
/// receptor, el selector, la aplicación y la lista de URL).
unsafe extern "C-unwind" fn open_urls(
    _this: *mut AnyObject,
    _cmd: Sel,
    _application: *mut AnyObject,
    urls: *const NSArray<NSURL>,
) {
    // SAFETY: AppKit pasa un `NSArray<NSURL>` válido mientras dura la llamada (o nulo).
    let Some(urls) = (unsafe { urls.as_ref() }) else {
        return;
    };
    let paths: Vec<PathBuf> = urls
        .iter()
        .filter_map(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .collect();
    if !paths.is_empty() {
        crate::instance::deliver(paths);
    }
}

/// Añade `application:openURLs:` a `NSObject`. Llamar una vez, antes de arrancar iced.
pub fn install() {
    let Some(class) = AnyClass::get(c"NSObject") else {
        return;
    };
    type OpenUrls =
        unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *const NSArray<NSURL>);
    // SAFETY: el runtime de Objective-C guarda las implementaciones como `Imp` y las llama
    // con la firma que dice la codificación `v@:@@`, que es la de `open_urls`.
    let imp = unsafe { std::mem::transmute::<OpenUrls, Imp>(open_urls) };
    // SAFETY: `class` es una clase registrada, el selector es válido y la codificación es
    // una cadena C estática. Si `NSObject` ya tuviera el método, no se cambia nada.
    unsafe {
        ffi::class_addMethod(
            std::ptr::from_ref(class).cast_mut(),
            sel!(application:openURLs:),
            imp,
            c"v@:@@".as_ptr(),
        );
    }
}
