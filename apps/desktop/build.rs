fn main() {
    set_build_number();
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

/// Monotonic number used by the auto-updater. CI sets `TWG_BUILD_NUMBER`;
/// local builds fall back to the git commit count.
fn set_build_number() {
    println!("cargo:rerun-if-env-changed=TWG_BUILD_NUMBER");
    let build = std::env::var("TWG_BUILD_NUMBER")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .or_else(git_build_number)
        .unwrap_or(0);
    println!("cargo:rustc-env=TWG_BUILD_NUMBER={build}");
}

fn git_build_number() -> Option<u64> {
    let output = std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}
