//! Windows release builds of `apple2-rust.exe` use the GUI subsystem (no console window when
//! double-clicked). To keep `--help`, `--headless-frames ... --dump-text` and error messages
//! visible when the `.exe` is launched from cmd/PowerShell, attach to the parent console and
//! point stdout/stderr at it.
//! Limits: the shell does not wait for a GUI-subsystem program, and cmd's `> file` and `| more`
//! do not reach it (a GUI process does not inherit the handles cmd redirects), so the output goes
//! to the console instead. Redirection and pipes need `apple2-rust.com`, the console-subsystem
//! twin in the package (`apple2-rust-console.exe` in `target/`), which `apple2-rust` typed without
//! an extension finds first (#107). In that binary `AttachConsole` fails and this is a no-op.

#[cfg(all(windows, not(debug_assertions)))]
#[allow(unsafe_code)]
pub fn attach_parent_console() {
    use std::os::windows::io::IntoRawHandle;
    type Handle = *mut core::ffi::c_void;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
        fn GetStdHandle(n: u32) -> Handle;
        fn SetStdHandle(n: u32, h: Handle) -> i32;
    }
    // SAFETY: plain kernel32 calls; handles passed to SetStdHandle are leaked on purpose
    // (they must stay open for the lifetime of the process).
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return; // no parent console (double-click): stay silent
        }
        for std_id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(std_id);
            // null / INVALID_HANDLE_VALUE: not redirected, so reopen the console screen buffer
            if h.is_null() || h as isize == -1 {
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                    SetStdHandle(std_id, f.into_raw_handle());
                }
            }
        }
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
pub fn attach_parent_console() {}
