//! The `conformance` binary: trace, diff and run.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_conformance"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("conf-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn trace_then_diff_against_itself_and_a_modified_copy() {
    let d = scratch("trace");
    let a = d.join("a.csv");
    let out = bin()
        .args([
            "trace",
            "--model",
            "apple2e-enhanced",
            "--frames",
            "2",
            "--input",
            "frame:1:A\\r",
            "-o",
        ])
        .arg(&a)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&a).unwrap();
    assert!(text.starts_with("# format=apple2-trace-v1\n"));
    assert!(text.contains("# model=apple2e-enhanced\n"));
    assert!(text.contains("\ncycle,pc,op,a,x,y,sp,p\n"));
    assert!(text.lines().count() > 1000);

    let same = bin().arg("diff").arg(&a).arg(&a).output().unwrap();
    assert!(same.status.success());
    assert!(String::from_utf8_lossy(&same.stdout).contains("idênticos"));

    // Change the cycle of row 50 and the X of row 60.
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let first = lines.iter().position(|l| l.starts_with("cycle")).unwrap() + 1;
    let cols: Vec<String> = lines[first + 49].split(',').map(String::from).collect();
    lines[first + 49] = format!(
        "{},{}",
        cols[0].parse::<u64>().unwrap() + 7,
        cols[1..].join(",")
    );
    let b = d.join("b.csv");
    std::fs::write(&b, lines.join("\n") + "\n").unwrap();
    let diff = bin().arg("diff").arg(&a).arg(&b).output().unwrap();
    assert_eq!(diff.status.code(), Some(1));
    let so = String::from_utf8_lossy(&diff.stdout);
    assert!(so.contains("instrução 50"), "{so}");
    assert!(so.contains("cycle esperado"), "{so}");
    // Ignoring cycle makes them equal.
    let ign = bin()
        .arg("diff")
        .arg(&a)
        .arg(&b)
        .args(["--ignore", "cycle"])
        .output()
        .unwrap();
    assert!(ign.status.success());
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn bad_arguments_exit_with_a_usage_error() {
    assert_eq!(bin().arg("nope").output().unwrap().status.code(), Some(64));
    assert_eq!(
        bin()
            .args(["trace", "--model", "zx81", "--frames", "1", "-o", "x"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(64)
    );
    assert_eq!(
        bin()
            .args(["run", "--report", "x"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(64)
    );
}

#[test]
fn run_all_writes_manifests_junit_and_summary() {
    let d = scratch("run");
    let out = bin()
        .args(["run", "--all", "--report"])
        .arg(&d)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for f in [
        "junit.xml",
        "summary.md",
        "boot-iie-enhanced-text-hash.manifest.json",
        "determinism-iie-enhanced-x10.manifest.json",
    ] {
        assert!(d.join(f).exists(), "{f}");
    }
    let s = std::fs::read_to_string(d.join("summary.md")).unwrap();
    assert!(s.contains("| PASS | 5 |"), "{s}");
    assert!(s.contains("| FAIL | 0 |"));
    let one = scratch("run-one");
    let out = bin()
        .args(["run", "--case", "boot-iie-enhanced-prompt", "--report"])
        .arg(&one)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(one.join("boot-iie-enhanced-prompt.manifest.json").exists());
    assert!(!one
        .join("boot-iie-enhanced-text-hash.manifest.json")
        .exists());
    let bad = bin()
        .args(["run", "--case", "nao-existe", "--report"])
        .arg(&one)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(64));
    std::fs::remove_dir_all(d).unwrap();
    std::fs::remove_dir_all(one).unwrap();
}

#[test]
fn run_exits_2_when_blocked_and_1_when_failing() {
    let root = scratch("root");
    std::fs::create_dir_all(root.join("cases")).unwrap();
    std::fs::create_dir_all(root.join("reference-traces")).unwrap();
    // The workspace is root/../.., so a fixture path relative to it is missing.
    std::fs::write(
        root.join("cases/b.toml"),
        format!(
            "id=\"b\"\nstory=\"HU-0002\"\ncriterion=\"CA\"\nmodel=\"apple2e\"\n[fixture]\npath=\"missing.yaml\"\nsha256=\"{}\"\n[run]\nframes=1\n[oracle]\nkind=\"state-hash\"\nauthority=\"fixture\"\nsource=\"t\"\nsha256=\"{}\"\n",
            "1".repeat(64),
            "2".repeat(64)
        ),
    )
    .unwrap();
    let rep = scratch("rep-b");
    let out = bin()
        .args(["run", "--all", "--root"])
        .arg(&root)
        .arg("--report")
        .arg(&rep)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let j = std::fs::read_to_string(rep.join("junit.xml")).unwrap();
    assert!(
        j.contains("<error message=\"BLOCKED: BLOCKED: fixture ausente"),
        "{j}"
    );

    std::fs::write(
        root.join("cases/f.toml"),
        format!(
            "id=\"f\"\nstory=\"HU-0101\"\ncriterion=\"CA\"\nmodel=\"apple2e\"\n[run]\nframes=1\n[oracle]\nkind=\"text-hash\"\nauthority=\"known-result\"\nsource=\"t\"\nsha256=\"{}\"\n",
            "9".repeat(64)
        ),
    )
    .unwrap();
    let out = bin()
        .args(["run", "--all", "--root"])
        .arg(&root)
        .arg("--report")
        .arg(&rep)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(rep.join("f.actual.txt").exists());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(rep).unwrap();
}
