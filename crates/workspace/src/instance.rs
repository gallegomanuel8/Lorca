//! Instancia única: si Lorca ya está abierto, una segunda ejecución le pasa los archivos y
//! termina («Abrir con Lorca» abre en la ventana existente).
//!
//! La instancia principal escucha en un puerto TCP de `127.0.0.1` y guarda el puerto y una
//! clave aleatoria en `<datos>/instance`. Las siguientes leen ese archivo, se conectan y
//! envían la clave y las rutas absolutas separadas por `\0` (una ruta no puede contener
//! ese carácter). La principal contesta `ok` cuando las acepta; si no hay respuesta
//! (archivo obsoleto de una ejecución anterior, puerto ocupado por otro programa…) la
//! nueva ejecución pasa a ser la principal.

use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::storage::write_atomic;

/// Nombre del archivo con el puerto y la clave, dentro de la carpeta de datos.
pub const INSTANCE_FILE: &str = "instance";

/// Tiempo máximo para conectar, enviar y recibir la respuesta.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Respuesta de la instancia principal cuando acepta los archivos.
const ACK: &[u8] = b"ok";
/// Tamaño máximo de un mensaje (protege de conexiones ajenas que no terminan).
const MAX_MESSAGE: u64 = 1 << 20;

/// Resultado de intentar arrancar.
#[derive(Debug)]
pub enum Launch {
    /// No había otra instancia: esta es la principal y debe atender a las siguientes.
    Primary(InstanceServer),
    /// Otra instancia ha recibido los archivos: esta debe terminar.
    Forwarded,
    /// No se pudo escuchar (sin carpeta de datos, sin red local…): se sigue sin instancia
    /// única.
    Standalone,
}

/// Arranca como instancia principal o, si ya hay una, le pasa `paths` (relativos a la
/// carpeta actual) y devuelve [`Launch::Forwarded`].
pub fn claim(data_dir: &Path, paths: &[PathBuf]) -> Launch {
    let file = data_dir.join(INSTANCE_FILE);
    if forward(&file, paths) {
        return Launch::Forwarded;
    }
    match InstanceServer::bind(&file) {
        Some(server) => Launch::Primary(server),
        None => Launch::Standalone,
    }
}

/// Intenta pasar `paths` a la instancia anotada en `file`. `true` si la aceptó.
fn forward(file: &Path, paths: &[PathBuf]) -> bool {
    let Ok(contents) = std::fs::read_to_string(file) else {
        return false;
    };
    let Some((port, token)) = parse_instance_file(&contents) else {
        return false;
    };
    let Some(message) = encode(token, paths) else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    if stream.write_all(&message).is_err() || stream.shutdown(std::net::Shutdown::Write).is_err() {
        return false;
    }
    let mut reply = Vec::new();
    let _ = stream.take(ACK.len() as u64).read_to_end(&mut reply);
    reply == ACK
}

/// Mensaje: clave, salto de línea y rutas absolutas terminadas en `\0`. `None` si alguna
/// ruta no es UTF-8 (entonces se abre una instancia nueva en vez de perderla).
fn encode(token: &str, paths: &[PathBuf]) -> Option<Vec<u8>> {
    let mut message = format!("{token}\n").into_bytes();
    for path in paths {
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        message.extend_from_slice(absolute.to_str()?.as_bytes());
        message.push(0);
    }
    Some(message)
}

/// Rutas de un mensaje si la clave coincide.
fn decode(message: &[u8], token: &str) -> Option<Vec<PathBuf>> {
    let text = std::str::from_utf8(message).ok()?;
    let (received, rest) = text.split_once('\n')?;
    if received != token {
        return None;
    }
    Some(
        rest.split_terminator('\0')
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .collect(),
    )
}

/// Archivo `instance`: puerto en la primera línea y clave en la segunda.
fn parse_instance_file(contents: &str) -> Option<(u16, &str)> {
    let mut lines = contents.lines();
    let port = lines.next()?.trim().parse().ok()?;
    let token = lines.next()?.trim();
    (!token.is_empty()).then_some((port, token))
}

/// Clave aleatoria (sin dependencias: `RandomState` se siembra con datos del sistema).
fn random_token() -> String {
    (0..2)
        .map(|_| format!("{:016x}", RandomState::new().build_hasher().finish()))
        .collect()
}

/// Instancia principal: escucha a las siguientes ejecuciones.
#[derive(Debug)]
pub struct InstanceServer {
    listener: TcpListener,
    token: String,
}

impl InstanceServer {
    fn bind(file: &Path) -> Option<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        let token = random_token();
        write_atomic(file, format!("{port}\n{token}\n").as_bytes()).ok()?;
        Some(Self { listener, token })
    }

    /// Atiende las conexiones en un hilo propio y llama a `on_open` con las rutas de cada
    /// ejecución nueva (vacía si se lanzó sin archivos: basta con traer la ventana).
    pub fn serve(self, mut on_open: impl FnMut(Vec<PathBuf>) + Send + 'static) {
        std::thread::spawn(move || {
            for stream in self.listener.incoming() {
                let Ok(stream) = stream else { continue };
                if let Some(paths) = receive(stream, &self.token) {
                    on_open(paths);
                }
            }
        });
    }
}

/// Lee un mensaje, contesta `ok` si la clave es buena y devuelve sus rutas.
fn receive(mut stream: TcpStream, token: &str) -> Option<Vec<PathBuf>> {
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let mut message = Vec::new();
    (&stream).take(MAX_MESSAGE).read_to_end(&mut message).ok()?;
    let paths = decode(&message, token)?;
    stream.write_all(ACK).ok()?;
    Some(paths)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn message_round_trip() {
        let paths = vec![PathBuf::from("/tmp/a b.txt"), PathBuf::from("/tmp/ñ.rs")];
        let message = encode("clave", &paths).unwrap();
        let decoded = decode(&message, "clave").unwrap();
        assert_eq!(decoded.len(), 2);
        assert!(decoded[0].ends_with("a b.txt"));
        assert!(decoded[1].ends_with("ñ.rs"));
        assert!(decoded.iter().all(|p| p.is_absolute()));
        assert_eq!(decode(&message, "otra"), None);
        assert_eq!(decode(b"clave\n", "clave"), Some(Vec::new()));
    }

    #[test]
    fn instance_file_needs_port_and_token() {
        assert_eq!(parse_instance_file("4000\nabc\n"), Some((4000, "abc")));
        assert_eq!(parse_instance_file("4000\n"), None);
        assert_eq!(parse_instance_file("puerto\nabc\n"), None);
    }

    #[test]
    fn tokens_are_random() {
        assert_ne!(random_token(), random_token());
        assert_eq!(random_token().len(), 32);
    }

    #[test]
    fn second_launch_forwards_to_first() {
        let dir = tempfile::tempdir().unwrap();
        let Launch::Primary(server) = claim(dir.path(), &[]) else {
            panic!("la primera ejecución debe ser la principal");
        };
        let (sender, receiver) = mpsc::channel();
        server.serve(move |paths| {
            let _ = sender.send(paths);
        });
        let file = dir.path().join("notas.md");
        assert!(matches!(
            claim(dir.path(), std::slice::from_ref(&file)),
            Launch::Forwarded
        ));
        assert_eq!(receiver.recv_timeout(TIMEOUT).unwrap(), vec![file]);
    }

    #[test]
    fn stale_file_makes_a_new_primary() {
        let dir = tempfile::tempdir().unwrap();
        // Puerto de un proceso que ya no existe: nadie escucha.
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::fs::write(dir.path().join(INSTANCE_FILE), format!("{port}\nvieja\n")).unwrap();
        assert!(matches!(claim(dir.path(), &[]), Launch::Primary(_)));
        let contents = std::fs::read_to_string(dir.path().join(INSTANCE_FILE)).unwrap();
        assert!(!contents.contains("vieja"));
    }

    #[test]
    fn wrong_token_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let Launch::Primary(server) = claim(dir.path(), &[]) else {
            panic!("la primera ejecución debe ser la principal");
        };
        let (sender, receiver) = mpsc::channel();
        server.serve(move |paths| {
            let _ = sender.send(paths);
        });
        let file = dir.path().join(INSTANCE_FILE);
        let contents = std::fs::read_to_string(&file).unwrap();
        let port = contents.lines().next().unwrap();
        std::fs::write(&file, format!("{port}\nfalsa\n")).unwrap();
        assert!(!forward(&file, &[]));
        assert!(receiver.recv_timeout(Duration::from_millis(300)).is_err());
    }
}
