// Embeds icon, version info and a DPI-aware manifest into applewin.exe (Windows hosts only).
// The resource compiler is only a build-dependency on Windows; elsewhere this is a no-op.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/applewin.manifest");
    println!("cargo:rerun-if-changed=../../packaging/icons/applewin.ico");
    #[cfg(windows)]
    embed();
}

#[cfg(windows)]
fn embed() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../packaging/icons/applewin.ico")
        .set_manifest_file("../../packaging/windows/applewin.manifest")
        .set("ProductName", "AppleII-Rust")
        .set("FileDescription", "AppleII-Rust - Apple II emulator (AppleWin port)")
        .set("InternalName", "applewin")
        .set("OriginalFilename", "applewin.exe")
        .set("LegalCopyright", "GPL-2.0-or-later. Based on AppleWin.")
        .set("CompanyName", "AppleII-Rust contributors");
    if let Err(e) = res.compile() {
        // never break the build over cosmetics
        println!("cargo:warning=could not embed Windows resources: {e}");
    }
}
