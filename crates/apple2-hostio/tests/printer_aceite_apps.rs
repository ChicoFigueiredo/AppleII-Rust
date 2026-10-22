//! Acceptance with period programs (issue #104): AppleWorks, Print Shop and dBase II (CP/M on the
//! Z80 SoftCard). The discs are third-party software and never enter the repository: the owner
//! drops them in `target/conformance-cache/printer-apps/<program>/`. Without them each test ends
//! as BLOCKED (message on stderr, no failure). With them, the steps are manual and live in
//! `docs/printer/aceite.md`: these programs need menu navigation that this project does not script
//! (PARTIAL), so the test only reports that the disc is there.

use std::path::PathBuf;

fn cache_dir(program: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/conformance-cache/printer-apps")
        .join(program)
}

fn has_disc(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                [
                    ".dsk", ".do", ".po", ".woz", ".2mg", ".nib", ".hdv", ".zip", ".gz",
                ]
                .iter()
                .any(|x| n.ends_with(x))
            })
        })
        .unwrap_or(false)
}

fn check(program: &str) {
    let dir = cache_dir(program);
    if !has_disc(&dir) {
        eprintln!("BLOCKED: disco ausente em {}", dir.display());
        return;
    }
    eprintln!(
        "PARTIAL: disco de {program} presente em {}; o roteiro de aceite é manual (docs/printer/aceite.md)",
        dir.display()
    );
}

#[test]
fn appleworks_printer_acceptance_needs_the_disc() {
    check("appleworks");
}

#[test]
fn print_shop_printer_acceptance_needs_the_disc() {
    check("printshop");
}

#[test]
fn dbase_ii_cpm_lst_acceptance_needs_the_disc() {
    check("dbase2");
}
