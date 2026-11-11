//! Incrusta los iconos en el ejecutable de Windows: el de la aplicación (ID 1) y el de los
//! documentos (ID 2, que el instalador usa como `lorca.exe,-2`). En el resto de sistemas no
//! hace nada: macOS los toma del bundle `.app` y Linux del paquete.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../assets/icons/lorca.ico");
        println!("cargo:rerun-if-changed=../../assets/icons/documento/lorca-documento.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/icons/lorca.ico");
        resource.set_icon_with_id("../../assets/icons/documento/lorca-documento.ico", "2");
        // Sin icono el programa funciona igual: se avisa en vez de fallar la compilación.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=no se pudo incrustar el icono: {error}");
        }
    }
}
