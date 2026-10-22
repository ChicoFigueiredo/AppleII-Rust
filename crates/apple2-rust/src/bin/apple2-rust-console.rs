// Console-subsystem twin of src/main.rs: the Windows package ships it as apple2-rust.com, so
// `apple2-rust` typed in cmd/PowerShell can be redirected and piped (#107). See build.rs.

#[path = "../main.rs"]
mod app;

fn main() {
    app::main();
}
