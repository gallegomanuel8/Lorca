//! Actualizaciones automáticas con `cargo-packager-updater`: comprueba si hay una versión
//! nueva publicada en GitHub (el `latest.json` de la última release), la descarga, verifica
//! su firma con la clave pública del proyecto y la instala.
//!
//! - macOS: sustituye el `.app` y Lorca se vuelve a abrir.
//! - Windows: ejecuta el instalador sin preguntas (`cargo-packager-updater` cierra Lorca).
//! - Linux: sustituye el AppImage (con el `.deb` solo se avisa).
//!
//! Las llamadas de red son bloqueantes: se hacen en un hilo aparte.

use std::path::PathBuf;
use std::sync::Arc;

use cargo_packager_updater::{Config, Update, UpdaterBuilder, semver::Version, url};
use iced::futures::Stream;
use iced::futures::channel::{mpsc, oneshot};

/// Manifiesto de la última versión publicada (la release más reciente de GitHub).
const ENDPOINT: &str =
    "https://github.com/gallegomanuel8/Lorca/releases/latest/download/latest.json";
/// Página de una versión publicada.
const RELEASE_PAGE: &str = "https://github.com/gallegomanuel8/Lorca/releases/tag/v";
/// Clave pública con la que se verifican las actualizaciones.
const PUBLIC_KEY: &str = include_str!("../../../assets/actualizaciones/clave-publica.txt");

/// Versión nueva disponible.
#[derive(Debug, Clone)]
pub struct Available {
    pub version: String,
    /// Se puede instalar sola (si no, solo se ofrece abrir la página de la versión).
    pub installable: bool,
    update: Arc<Update>,
}

impl Available {
    /// Página de la versión en GitHub (novedades y descargas).
    pub fn page(&self) -> String {
        format!("{RELEASE_PAGE}{}", self.version.trim_start_matches('v'))
    }
}

/// Progreso de la descarga e instalación.
#[derive(Debug, Clone)]
pub enum Progress {
    /// Fracción descargada (`None` si no se conoce el tamaño).
    Downloading(Option<f32>),
    /// Descargada y verificada: se está instalando.
    Installing,
    /// Instalada: hay que reiniciar.
    Installed,
    Failed(String),
}

fn updater() -> Result<cargo_packager_updater::Updater, String> {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
    let endpoint = ENDPOINT
        .parse()
        .map_err(|e: url::ParseError| e.to_string())?;
    let config = Config {
        endpoints: vec![endpoint],
        pubkey: PUBLIC_KEY.trim().to_owned(),
        windows: None,
    };
    UpdaterBuilder::new(current, config)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

/// En Linux solo se instala solo el AppImage (con el `.deb` el ejecutable es del sistema).
fn installable() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

/// Comprueba si hay una versión nueva. `Ok(None)`: Lorca está al día.
pub async fn check() -> Result<Option<Available>, String> {
    let (sender, receiver) = oneshot::channel();
    std::thread::spawn(move || {
        let result = updater().and_then(|updater| updater.check().map_err(|e| e.to_string()));
        let _ = sender.send(result);
    });
    let update = receiver
        .await
        .map_err(|_| "se interrumpió la comprobación".to_owned())??;
    Ok(update.map(|update| Available {
        version: update.version.clone(),
        installable: installable(),
        update: Arc::new(update),
    }))
}

/// Descarga, verifica e instala `available`, informando del progreso. En Windows el
/// instalador cierra Lorca al empezar (la sesión se guarda antes de llamar a esto).
pub fn install(available: Available) -> impl Stream<Item = Progress> {
    let (sender, receiver) = mpsc::unbounded();
    std::thread::spawn(move || {
        let update = available.update;
        let progress = sender.clone();
        let downloaded = std::sync::atomic::AtomicUsize::new(0);
        let bytes = update.download_extended(
            |chunk, total| {
                let done =
                    downloaded.fetch_add(chunk, std::sync::atomic::Ordering::Relaxed) + chunk;
                let fraction = total
                    .filter(|total| *total > 0)
                    .map(|total| (done as f32 / total as f32).min(1.0));
                let _ = progress.unbounded_send(Progress::Downloading(fraction));
            },
            || {},
        );
        let result = bytes.and_then(|bytes| {
            let _ = sender.unbounded_send(Progress::Installing);
            update.install(bytes)
        });
        let _ = sender.unbounded_send(match result {
            Ok(()) => Progress::Installed,
            Err(error) => Progress::Failed(error.to_string()),
        });
    });
    receiver
}

/// Vuelve a abrir Lorca un momento después de que esta instancia termine (la nueva
/// encontraría aún a esta como instancia principal y se cerraría).
pub fn relaunch() {
    let Some(target) = relaunch_target() else {
        return;
    };
    let command = if cfg!(target_os = "macos") {
        "sleep 1; open -n \"$0\""
    } else {
        "sleep 1; \"$0\" >/dev/null 2>&1 &"
    };
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .arg(target)
        .spawn();
}

/// Lo que hay que abrir para reiniciar: el `.app` en macOS, el AppImage en Linux.
fn relaunch_target() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let executable = std::env::current_exe().ok()?;
        // Lorca.app/Contents/MacOS/lorca → Lorca.app
        executable.ancestors().nth(3).map(PathBuf::from)
    } else {
        std::env::var_os("APPIMAGE").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_is_valid() {
        assert!(PUBLIC_KEY.trim().len() > 40);
        assert!(ENDPOINT.parse::<url::Url>().is_ok());
    }
}
