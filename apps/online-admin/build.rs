fn main() {
    let icon = "../../assets/icons/twg-admin.ico";
    println!("cargo:rerun-if-changed={icon}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon);
    resource.set("FileDescription", "TWG Online Admin");
    resource.set("ProductName", "TWG Online Admin");
    resource.set("OriginalFilename", "twg-online-admin.exe");
    if let Err(error) = resource.compile() {
        panic!("failed to embed the Windows application icon: {error}");
    }
}
