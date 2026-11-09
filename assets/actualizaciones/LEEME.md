# Actualizaciones automáticas

`clave-publica.txt` es la clave pública (minisign) con la que Lorca comprueba que una
actualización la ha firmado el proyecto. Es pública por diseño: solo sirve para verificar.

La clave privada y su contraseña están en los secretos del repositorio
(`CARGO_PACKAGER_SIGN_PRIVATE_KEY` y `CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD`); con ellas
cargo-packager firma los paquetes en `release.yml`, que además publica `latest.json`.

Si se pierde la clave privada hay que generar otra (`cargo packager signer generate`),
sustituir este archivo y los secretos, y las versiones ya instaladas no podrán actualizarse
solas a la siguiente: habrá que instalarla a mano una vez.
