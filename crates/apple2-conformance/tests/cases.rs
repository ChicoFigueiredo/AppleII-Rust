//! Shipped cases pass, and the failure paths (FAIL with diff artifact, BLOCKED with cause) work.

use apple2_conformance::case::Case;
use apple2_conformance::manifest::{validate_against_schema, TestResult};
use apple2_conformance::report::write_report;
use apple2_conformance::runner::{load_cases, run_case, Env};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("conf-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn case(toml: &str) -> Case {
    let c: Case = toml::from_str(toml).unwrap();
    c.validate().unwrap();
    c
}

/// An `Env` whose conformance root is a scratch dir (workspace = root/../..).
fn env_in(root: &Path) -> Env {
    let mut e = Env::detect(None);
    e.root = root.to_path_buf();
    e.workspace = root.join("../..");
    e
}

const HEAD: &str = r#"
id = "c1"
story = "HU-0101"
criterion = "CA"
model = "apple2plus"
[run]
cycles = 3000
"#;

#[test]
fn every_shipped_case_passes_and_its_manifest_validates() {
    let env = Env::detect(None);
    let cases = load_cases(&env.root.join("cases")).unwrap();
    assert!(cases.len() >= 5);
    let mut outcomes = Vec::new();
    for c in &cases {
        let o = run_case(&env, c);
        // A case may document a finding it is expected to fail on (`known_divergence`), and a
        // fixture-based case is BLOCKED, never PASS, where the corpus cache is absent.
        let fixture_missing = c
            .fixture
            .as_ref()
            .is_some_and(|f| !env.workspace.join(&f.path).exists());
        let expected_fail =
            o.manifest.result == TestResult::Fail && c.oracle.known_divergence.is_some();
        let expected_blocked = o.manifest.result == TestResult::Blocked && fixture_missing;
        assert!(
            o.manifest.result == TestResult::Pass || expected_fail || expected_blocked,
            "{}: {:?} {}",
            c.id,
            o.manifest.result,
            o.manifest.actual
        );
        validate_against_schema(&serde_json::to_value(&o.manifest).unwrap()).unwrap();
        assert!(o.manifest.rom_sha256.is_some(), "{}", c.id);
        outcomes.push(o);
    }
    let dir = scratch("shipped");
    write_report(&dir, &outcomes).unwrap();
    assert!(dir.join("junit.xml").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_self_trace_case_is_flagged_as_regression_not_applewin() {
    let env = Env::detect(None);
    let cases = load_cases(&env.root.join("cases")).unwrap();
    let c = cases
        .iter()
        .find(|c| c.id == "trace-iiplus-boot-selfref")
        .unwrap();
    assert_eq!(c.oracle.authority.as_str(), "regression");
    let o = run_case(&env, c);
    assert!(o.manifest.expected.contains("NÃO é oráculo do AppleWin"));
    assert!(o.manifest.applewin_sha.is_none());
}

#[test]
fn missing_fixture_is_blocked_with_the_cause() {
    let root = scratch("fixture").join("a/b");
    std::fs::create_dir_all(&root).unwrap();
    let c = case(&format!(
        r#"{HEAD}
[fixture]
path = "target/conformance-cache/applewin-test/v2/Memory/Saturn128.aws.yaml"
sha256 = "{}"
[oracle]
kind = "state-hash"
authority = "fixture"
source = "t"
sha256 = "{}"
"#,
        "1".repeat(64),
        "2".repeat(64)
    ));
    let o = run_case(&env_in(&root), &c);
    assert_eq!(o.manifest.result, TestResult::Blocked);
    assert!(
        o.manifest.actual.contains("fixture ausente"),
        "{}",
        o.manifest.actual
    );
    assert!(o.manifest.actual.contains("Saturn128.aws.yaml"));
    assert_eq!(o.manifest.fixture_sha256, Some("1".repeat(64)));
}

#[test]
fn fixture_with_wrong_hash_is_blocked() {
    let root = scratch("fixture-hash").join("a/b");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("../../f.bin"), b"abc").unwrap();
    let c = case(&format!(
        r#"{HEAD}
[fixture]
path = "f.bin"
sha256 = "{}"
[oracle]
kind = "state-hash"
authority = "fixture"
source = "t"
sha256 = "{}"
"#,
        "1".repeat(64),
        "2".repeat(64)
    ));
    let o = run_case(&env_in(&root), &c);
    assert_eq!(o.manifest.result, TestResult::Blocked);
    assert!(
        o.manifest.actual.contains("ba7816bf"),
        "{}",
        o.manifest.actual
    );
}

#[test]
fn wrong_hash_fails_with_an_actual_dump() {
    let c = case(&format!(
        r#"{HEAD}
[oracle]
kind = "text-hash"
authority = "known-result"
source = "t"
sha256 = "{}"
"#,
        "3".repeat(64)
    ));
    let o = run_case(&Env::detect(None), &c);
    assert_eq!(o.manifest.result, TestResult::Fail);
    assert_eq!(o.manifest.diff_artifact.as_deref(), Some("c1.actual.txt"));
    assert!(o
        .artifacts
        .iter()
        .any(|(n, b)| n == "c1.actual.txt" && b.len() >= 24));
    assert!(o.manifest.actual.contains("esperado 3333"));
}

#[test]
fn failing_framebuffer_writes_a_png() {
    let c = case(&format!(
        r#"
id = "fb"
story = "HU-0101"
criterion = "CA"
model = "apple2e"
[run]
frames = 3
[oracle]
kind = "framebuffer-hash"
authority = "regression"
source = "t"
sha256 = "{}"
"#,
        "4".repeat(64)
    ));
    let o = run_case(&Env::detect(None), &c);
    assert_eq!(o.manifest.result, TestResult::Fail);
    let (_, png) = o
        .artifacts
        .iter()
        .find(|(n, _)| n == "fb.actual.png")
        .unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn failing_expression_is_reported() {
    let c = case(&format!(
        r#"{HEAD}
[oracle]
kind = "expr"
authority = "known-result"
source = "t"
expect = ["pc >= 0x0000", "a == 0x42"]
"#
    ));
    let o = run_case(&Env::detect(None), &c);
    assert_eq!(o.manifest.result, TestResult::Fail);
    assert!(o.manifest.actual.contains("a == 0x42"));
    assert!(!o.manifest.actual.contains("pc >="));
}

#[test]
fn trace_divergence_produces_a_diff_artifact_and_ignore_works() {
    let env = Env::detect(None);
    let root = scratch("trace");
    std::fs::create_dir_all(root.join("reference-traces")).unwrap();
    // A reference whose cycles are all shifted, and one with a corrupted register.
    let refs = std::fs::read_to_string(
        env.root
            .join("reference-traces/apple2plus-boot-selfref.csv"),
    )
    .unwrap();
    let mut shifted = String::new();
    let mut corrupted = String::new();
    let mut n = 0;
    for l in refs.lines() {
        if l.starts_with('#') || l.starts_with("cycle") {
            shifted.push_str(&format!("{l}\n"));
            corrupted.push_str(&format!("{l}\n"));
            continue;
        }
        let cols: Vec<&str> = l.split(',').collect();
        shifted.push_str(&format!(
            "{},{}\n",
            cols[0].parse::<u64>().unwrap() + 1,
            cols[1..].join(",")
        ));
        n += 1;
        if n == 100 {
            corrupted.push_str(&format!(
                "{},{},{},77,{}\n",
                cols[0],
                cols[1],
                cols[2],
                cols[4..].join(",")
            ));
        } else {
            corrupted.push_str(&format!("{l}\n"));
        }
    }
    std::fs::write(root.join("reference-traces/shifted.csv"), shifted).unwrap();
    std::fs::write(root.join("reference-traces/corrupt.csv"), corrupted).unwrap();
    let mk = |file: &str, ignore: &str| {
        case(&format!(
            r#"
id = "t"
story = "HU-0101"
criterion = "CA"
model = "apple2plus"
[run]
cycles = 6000
[oracle]
kind = "trace"
authority = "regression"
source = "t"
trace = "reference-traces/{file}"
ignore = [{ignore}]
context_lines = 3
"#
        ))
    };
    let e = env_in(&root);
    let o = run_case(&e, &mk("shifted.csv", ""));
    assert_eq!(o.manifest.result, TestResult::Fail);
    assert!(o.manifest.actual.contains("cycle"), "{}", o.manifest.actual);
    assert_eq!(
        run_case(&e, &mk("shifted.csv", "\"cycle\""))
            .manifest
            .result,
        TestResult::Pass
    );

    let o = run_case(&e, &mk("corrupt.csv", "\"cycle\""));
    assert_eq!(o.manifest.result, TestResult::Fail);
    assert_eq!(
        o.manifest.diff_artifact.as_deref(),
        Some("t.trace-diff.txt")
    );
    let (_, diff) = o
        .artifacts
        .iter()
        .find(|(n, _)| n == "t.trace-diff.txt")
        .unwrap();
    let diff = String::from_utf8_lossy(diff);
    assert!(diff.contains("instrução 100"), "{diff}");
    assert!(diff.contains("a esperado 77"), "{diff}");
    assert!(o.artifacts.iter().any(|(n, _)| n == "t.actual.csv"));
    // Exactly three context rows precede the "-" line.
    let before: Vec<&str> = diff
        .lines()
        .skip_while(|l| !l.starts_with("# cycle"))
        .skip(1)
        .take_while(|l| !l.starts_with("- "))
        .collect();
    assert_eq!(before.len(), 3, "{diff}");

    let missing = run_case(&e, &mk("nope.csv", ""));
    assert_eq!(missing.manifest.result, TestResult::Blocked);
    assert!(missing
        .manifest
        .actual
        .contains("trace de referência ausente"));
}

#[test]
fn key_refused_by_the_model_is_a_fail_not_a_skip() {
    let c = case(&format!(
        r#"{HEAD}
[[input]]
at_cycle = 100
key = 200
[oracle]
kind = "state-hash"
authority = "determinism"
source = "t"
sha256 = "{}"
"#,
        "5".repeat(64)
    ));
    let o = run_case(&Env::detect(None), &c);
    // Either the model took the key (then the hash differs) or refused it: never PASS.
    assert_eq!(o.manifest.result, TestResult::Fail, "{}", o.manifest.actual);
}

#[test]
fn different_machine_states_give_different_hashes() {
    // Two different cases must not collide on the same hash (sanity for the hash itself).
    let mk = |frames: u32| {
        case(&format!(
            r#"
id = "d{frames}"
story = "HU-0101"
criterion = "CA"
model = "apple2e"
[run]
frames = {frames}
repeat = 3
[oracle]
kind = "state-hash"
authority = "determinism"
source = "t"
"#
        ))
    };
    let env = Env::detect(None);
    let (a, b) = (run_case(&env, &mk(5)), run_case(&env, &mk(6)));
    assert_eq!(a.manifest.result, TestResult::Pass);
    assert_eq!(b.manifest.result, TestResult::Pass);
    assert_ne!(a.manifest.actual, b.manifest.actual);
}
