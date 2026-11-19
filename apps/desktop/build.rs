fn main() {
    let icon = "../../assets/icons/twg-desktop.ico";
    println!("cargo:rerun-if-changed={icon}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon);
    resource.set("FileDescription", "Tactical Game 3D");
    resource.set("ProductName", "Tactical Game 3D");
    resource.set("OriginalFilename", "twg-desktop.exe");
    if let Err(error) = resource.compile() {
        panic!("failed to embed the Windows application icon: {error}");
    }
}
