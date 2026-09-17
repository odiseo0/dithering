fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/app.ico")
        .set_manifest_file("assets/app.manifest")
        .set("ProductName", "Dithering")
        .set("FileDescription", "Aplicación de dithering de imágenes")
        .set("OriginalFilename", "dithering.exe");
    resource.compile().expect("Windows resources must compile");
}
