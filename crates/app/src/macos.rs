//! Integración con macOS que iced no ofrece: «Abrir con» y doble clic en el Finder, y el
//! pellizco del touchpad para el zoom.
//!
//! ## Abrir documentos
//!
//! macOS no pasa los archivos como argumentos: envía a la aplicación el evento «abrir
//! documentos», que AppKit traduce en `-[delegado application:openURLs:]`. iced (winit) no
//! implementa ese método, así que macOS decía que Lorca «no puede abrir archivos en ese
//! formato». Aquí se añade el método en tiempo de ejecución y las rutas se entregan como
//! las de otra ejecución (`instance::deliver`): se abren en pestañas y la ventana pasa al
//! frente.
//!
//! Es el **único módulo con `unsafe`** del proyecto (el resto lo tiene prohibido): hace
//! falta para hablar con el runtime de Objective-C y con AppKit.
//!
//! El método se añade a `NSObject` antes de arrancar la aplicación: así lo tiene ya el
//! delegado que crea winit cuando llega el evento del arranque (si se abre Lorca con un
//! archivo). Solo AppKit lo llama, y solo sobre el delegado de la aplicación.
//!
//! ## Pellizco
//!
//! winit detecta el gesto de pellizcar (`PinchGesture`), pero iced lo descarta. Un
//! monitor local de eventos de AppKit recibe los eventos de ampliación de la ventana, los
//! convierte en pasos de tamaño de letra y los manda a la aplicación por una suscripción
//! (`Message::Zoom`).

#![allow(unsafe_code)]

use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::{Mutex, Once, OnceLock};

use block2::RcBlock;
use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::{SinkExt, StreamExt};
use objc2::ffi;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;
use objc2_app_kit::{NSEvent, NSEventMask};
use objc2_foundation::{NSArray, NSURL};

use crate::app::Message;

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
pub fn install_open_documents() {
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

// --- Pellizco ------------------------------------------------------------------------

/// Ampliación acumulada (fracción del tamaño) que equivale a un punto de letra.
const MAGNIFY_PER_STEP: f64 = 0.06;

/// Pasos de zoom pendientes de entregar a la aplicación.
static PINCH: OnceLock<UnboundedSender<i8>> = OnceLock::new();
static PINCH_RECEIVER: Mutex<Option<UnboundedReceiver<i8>>> = Mutex::new(None);

/// Canal de los pasos de zoom, creado en el primer uso. La suscripción puede arrancar
/// antes de que se instale el monitor (o al revés): los dos lo piden aquí, así que el
/// receptor siempre existe cuando la suscripción lo busca.
fn pinch_sender() -> &'static UnboundedSender<i8> {
    PINCH.get_or_init(|| {
        let (sender, receiver) = unbounded();
        if let Ok(mut slot) = PINCH_RECEIVER.lock() {
            *slot = Some(receiver);
        }
        sender
    })
}
/// Ampliación acumulada que aún no llega a un paso.
static PINCH_PENDING: Mutex<f64> = Mutex::new(0.0);

/// Convierte la ampliación de un evento de pellizco en pasos de letra, guardando el resto
/// en `pending`. Abrir los dedos (positivo) agranda.
fn pinch_steps(pending: &mut f64, magnification: f64) -> i8 {
    if !magnification.is_finite() {
        return 0;
    }
    *pending += magnification / MAGNIFY_PER_STEP;
    let steps = pending.trunc();
    *pending -= steps;
    steps.clamp(-8.0, 8.0) as i8
}

/// Empieza a escuchar el pellizco del touchpad. Llamar en el hilo principal con la
/// aplicación ya arrancada (se hace al instalar el menú); solo actúa la primera vez.
pub fn install_pinch_zoom() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let sender = pinch_sender().clone();
        let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit pasa un evento válido durante la llamada.
            let magnification = unsafe { event.as_ref() }.magnification();
            let steps = PINCH_PENDING
                .lock()
                .map_or(0, |mut pending| pinch_steps(&mut pending, magnification));
            if steps != 0 {
                let _ = sender.unbounded_send(steps);
            }
            // Se devuelve el mismo evento: el resto de la aplicación lo sigue recibiendo.
            event.as_ptr()
        });
        // SAFETY: el bloque devuelve el puntero del evento que recibe, que es válido.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::Magnify, &handler)
        };
        // El monitor dura lo que la aplicación: no se quita nunca.
        std::mem::forget(monitor);
    });
}

/// `Message::Zoom` con los pasos del pellizco.
pub fn pinch_events() -> Subscription<Message> {
    Subscription::run(|| {
        iced::stream::channel(8, async |mut output| {
            pinch_sender();
            let receiver = PINCH_RECEIVER.lock().ok().and_then(|mut slot| slot.take());
            let Some(mut receiver) = receiver else {
                return;
            };
            while let Some(steps) = receiver.next().await {
                if output.send(Message::Zoom(steps)).await.is_err() {
                    break;
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinch_accumulates_into_font_steps() {
        let mut pending = 0.0;
        assert_eq!(pinch_steps(&mut pending, 0.03), 0);
        assert_eq!(pinch_steps(&mut pending, 0.04), 1);
        assert_eq!(pinch_steps(&mut pending, -0.13), -2);
        assert_eq!(pinch_steps(&mut pending, f64::NAN), 0);
    }
}
