//! Disk boot cases (trace-checkpoints): the checkpoint file round-trips, a tampered or wrong run
//! is a FAIL, a missing fixture is a BLOCKED with the cause. Uses the free DOS 3.3 image only.

use apple2_conformance::case::Case;
use apple2_conformance::disk::{self, Checkpoints, Failure};
use apple2_conformance::manifest::TestResult;
use apple2_conformance::runner::{machine_config, run_case, Env};

const CKPT: &str =
    include_str!("../../../tests/conformance/reference-traces/disk/disk-dos33-boot-apple2plus.ckpt");
const DOS: &str = "assets/disks/DOS 3.3 System Master - 680-0210-A.dsk";

fn dos_bytes() -> Vec<u8> {
    std::fs::read(format!("{}/../../{DOS}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn case_cfg() -> apple2_core::MachineConfig {
    apple2_core::MachineConfig::for_model(apple2_core::Model::Apple2Plus)
}

/// The shipped file cut after `rows` instructions (the hash chain stays valid for the prefix).
fn prefix(rows: u64) -> Checkpoints {
    let mut c = Checkpoints::parse(CKPT).unwrap();
    c.checks.retain(|k| k.row < rows);
    c.syncs.retain(|s| s.row < rows);
    c
}

#[test]
fn checkpoint_file_round_trips() {
    let c = Checkpoints::parse(CKPT).unwrap();
    assert_eq!(c.meta("format"), Some(disk::FORMAT));
    assert_eq!(Checkpoints::parse(&c.to_text()).unwrap().checks, c.checks);
    assert!(c.rows() > 2_000_000);
    assert!(Checkpoints::parse("ck,0,0,0000,00,00,00,00,00,0\n").is_err());
}

#[test]
fn the_machine_agrees_with_the_oracle_on_the_first_300k_instructions() {
    let ck = prefix(300_000);
    let mut m = disk::power_on(case_cfg(), DOS, dos_bytes(), 0).unwrap();
    disk::check(&mut m, &ck).unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn a_tampered_checkpoint_is_a_fail() {
    let mut ck = prefix(60_000);
    let mid = ck.checks.len() / 2;
    ck.checks[mid].hash ^= 1;
    let mut m = disk::power_on(case_cfg(), DOS, dos_bytes(), 0).unwrap();
    match disk::check(&mut m, &ck) {
        Err(Failure::Fail(msg, _)) => assert!(msg.contains("divergência"), "{msg}"),
        other => panic!("expected FAIL, got {other:?}"),
    }
}

#[test]
fn a_different_ram_pattern_is_detected_by_the_boot_trace() {
    // memclear 2 fills RAM with $FF: the early boot reads it, so the chain breaks.
    let ck = prefix(60_000);
    let mut m = disk::power_on(case_cfg(), DOS, dos_bytes(), 2).unwrap();
    assert!(matches!(
        disk::check(&mut m, &ck),
        Err(Failure::Fail(..))
    ));
}

#[test]
fn a_missing_fixture_is_blocked_with_the_cause() {
    let toml = r#"
id = "d1"
story = "HU-0301"
criterion = "CA"
model = "apple2plus"
[fixture]
path = "target/conformance-cache/does-not-exist.woz"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
[run]
cycles = 1000
[oracle]
kind = "trace-checkpoints"
authority = "applewin"
source = "x"
trace = "reference-traces/disk/disk-dos33-boot-apple2plus.ckpt"
"#;
    let c: Case = toml::from_str(toml).unwrap();
    c.validate().unwrap();
    let _ = machine_config(&c).unwrap();
    let o = run_case(&Env::detect(None), &c);
    assert_eq!(o.manifest.result, TestResult::Blocked);
    assert!(o.manifest.actual.contains("fixture ausente"));
}

#[test]
fn a_trace_checkpoints_case_needs_a_fixture() {
    let toml = r#"
id = "d2"
story = "HU-0301"
criterion = "CA"
model = "apple2plus"
[run]
cycles = 1000
[oracle]
kind = "trace-checkpoints"
authority = "applewin"
source = "x"
trace = "a.ckpt"
"#;
    let c: Case = toml::from_str(toml).unwrap();
    assert!(c.validate().is_err());
}
