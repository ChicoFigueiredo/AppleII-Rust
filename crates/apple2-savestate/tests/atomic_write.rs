//! AUD-C-022 / HU-0402: a gravação do save-state é atômica. Uma queda simulada (erro de escrita
//! injetado no meio do arquivo) não pode destruir o último save válido.

use apple2_savestate::{write_atomic, write_atomic_with};
use std::io::{self, Write};
use std::path::PathBuf;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("aws-atomic-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn leftovers(d: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn replaces_the_target_and_leaves_no_temporary() {
    let d = dir("ok");
    let t = d.join("s.aws.yaml");
    std::fs::write(&t, "OLD").unwrap();
    write_atomic(&t, b"NEW CONTENT").unwrap();
    assert_eq!(std::fs::read_to_string(&t).unwrap(), "NEW CONTENT");
    assert_eq!(leftovers(&d), vec!["s.aws.yaml".to_string()]);
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn creates_a_missing_target() {
    let d = dir("new");
    let t = d.join("n.aws.yaml");
    write_atomic(&t, b"X").unwrap();
    assert_eq!(std::fs::read(&t).unwrap(), b"X");
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_write_error_midway_keeps_the_last_valid_save() {
    let d = dir("crash");
    let t = d.join("s.aws.yaml");
    std::fs::write(&t, "LAST VALID SAVE").unwrap();
    // Hard link = the same inode as the old save: truncate-in-place would change it too.
    let twin = d.join("twin");
    std::fs::hard_link(&t, &twin).unwrap();
    let r = write_atomic_with(&t, |f| {
        f.write_all(b"HALF OF THE NEW FI")?; // the "crash" comes after a partial write
        Err(io::Error::other("disco cheio simulado"))
    });
    assert!(r.is_err());
    assert_eq!(std::fs::read_to_string(&t).unwrap(), "LAST VALID SAVE");
    assert_eq!(std::fs::read_to_string(&twin).unwrap(), "LAST VALID SAVE");
    let mut names = leftovers(&d);
    names.sort();
    assert_eq!(names, vec!["s.aws.yaml".to_string(), "twin".to_string()]);
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_failed_first_save_leaves_nothing_behind() {
    let d = dir("first");
    let t = d.join("s.aws.yaml");
    let r = write_atomic_with(&t, |_| Err(io::Error::other("falha")));
    assert!(r.is_err());
    assert!(leftovers(&d).is_empty());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn an_unwritable_directory_is_an_error_not_a_panic() {
    let d = dir("nodir");
    let t = d.join("missing").join("s.aws.yaml");
    assert!(write_atomic(&t, b"x").is_err());
    std::fs::remove_dir_all(d).unwrap();
}
