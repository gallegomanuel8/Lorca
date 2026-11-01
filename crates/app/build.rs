//! Incrusta el icono en el ejecutable de Windows. En el resto de sistemas no hace nada:
//! macOS lo toma del bundle `.app` y Linux del paquete.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../assets/icons/lorca.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/icons/lorca.ico");
        // Sin icono el programa funciona igual: se avisa en vez de fallar la compilación.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=no se pudo incrustar el icono: {error}");
        }
    }
}
