// Embeds icon, version info and a DPI-aware manifest into apple2-rust.exe (Windows hosts only).
// The resource compiler is only a build-dependency on Windows; elsewhere this is a no-op.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/apple2-rust.manifest");
    println!("cargo:rerun-if-changed=../../packaging/icons/apple2-rust.ico");
    #[cfg(windows)]
    embed();
    gui_subsystem();
}

// Windows release builds: `apple2-rust` is a GUI-subsystem program (no console window on double
// click); `apple2-rust-console` stays in the console subsystem, so the shell waits for it and its
// output can be redirected (#107). Same condition as the old `windows_subsystem` attribute.
fn gui_subsystem() {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if var("CARGO_CFG_TARGET_OS") != "windows"
        || std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some()
    {
        return;
    }
    if var("CARGO_CFG_TARGET_ENV") == "msvc" {
        println!("cargo:rustc-link-arg-bin=apple2-rust=/SUBSYSTEM:WINDOWS");
        println!("cargo:rustc-link-arg-bin=apple2-rust=/ENTRY:mainCRTStartup");
    } else {
        println!("cargo:rustc-link-arg-bin=apple2-rust=-Wl,--subsystem,windows");
    }
}

#[cfg(windows)]
fn embed() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../packaging/icons/apple2-rust.ico")
        .set_manifest_file("../../packaging/windows/apple2-rust.manifest")
        .set("ProductName", "AppleII-Rust")
        .set(
            "FileDescription",
            "AppleII-Rust - Apple II emulator (AppleWin port)",
        )
        .set("InternalName", "apple2-rust")
        .set("OriginalFilename", "apple2-rust.exe")
        .set("LegalCopyright", "GPL-2.0-or-later. Based on AppleWin.")
        .set("CompanyName", "AppleII-Rust contributors");
    if let Err(e) = res.compile() {
        // never break the build over cosmetics
        println!("cargo:warning=could not embed Windows resources: {e}");
    }
}
