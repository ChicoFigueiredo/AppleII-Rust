//! Report writers: per-case manifests, `junit.xml` and `summary.md`.

use crate::manifest::{now_utc, TestResult};
use crate::runner::Outcome;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// Counts per result.
pub fn counts(outcomes: &[Outcome]) -> BTreeMap<TestResult, usize> {
    let mut m: BTreeMap<TestResult, usize> = TestResult::ALL.iter().map(|r| (*r, 0)).collect();
    for o in outcomes {
        *m.entry(o.manifest.result).or_default() += 1;
    }
    m
}

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// JUnit XML. FAIL is a `<failure>`; BLOCKED is an `<error>` (never a silent skip); UNVERIFIED,
/// NOT_APPLICABLE and IMPLEMENTED are `<skipped>` with the state in the message.
pub fn junit_xml(outcomes: &[Outcome]) -> String {
    let c = counts(outcomes);
    let failures = c[&TestResult::Fail];
    let errors = c[&TestResult::Blocked];
    let skipped =
        c[&TestResult::Unverified] + c[&TestResult::NotApplicable] + c[&TestResult::Implemented];
    let total_time: f64 = outcomes.iter().map(|o| o.seconds).sum();
    let ts = outcomes
        .first()
        .map_or_else(now_utc, |o| o.manifest.timestamp_utc.clone());
    let head = format!(
        "tests=\"{}\" failures=\"{failures}\" errors=\"{errors}\" skipped=\"{skipped}\" time=\"{total_time:.3}\"",
        outcomes.len()
    );
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str(&format!("<testsuites name=\"conformance\" {head}>\n"));
    x.push_str(&format!(
        "  <testsuite name=\"apple2-conformance\" {head} timestamp=\"{}\">\n",
        xml_escape(&ts)
    ));
    for o in outcomes {
        let m = &o.manifest;
        x.push_str(&format!(
            "    <testcase classname=\"{}\" name=\"{}\" time=\"{:.3}\"",
            xml_escape(&m.user_story),
            xml_escape(&m.test_id),
            o.seconds
        ));
        let detail = format!(
            "expected: {}\nactual: {}\ndiff_artifact: {}\nmanifest: {}.manifest.json",
            m.expected,
            m.actual,
            m.diff_artifact.as_deref().unwrap_or("-"),
            m.test_id
        );
        match m.result {
            TestResult::Pass => x.push_str("/>\n"),
            r => {
                let (tag, extra) = match r {
                    TestResult::Fail => ("failure", ""),
                    TestResult::Blocked => ("error", ""),
                    _ => ("skipped", ""),
                };
                x.push_str(&format!(
                    ">\n      <{tag} message=\"{}: {}\" type=\"{}\">{}</{tag}>{extra}\n    </testcase>\n",
                    r,
                    xml_escape(&m.actual),
                    r,
                    xml_escape(&detail)
                ));
            }
        }
    }
    x.push_str("  </testsuite>\n</testsuites>\n");
    x
}

/// Markdown summary with the counts per result and one line per case.
pub fn summary_md(outcomes: &[Outcome]) -> String {
    let c = counts(outcomes);
    let mut s = String::from("# Resumo da conformidade\n\n");
    s.push_str(&format!("Casos executados: {}\n\n", outcomes.len()));
    s.push_str("| Resultado | Casos |\n|---|---|\n");
    for r in TestResult::ALL {
        s.push_str(&format!("| {r} | {} |\n", c[&r]));
    }
    s.push_str("\n| Caso | História | Modelo | Resultado | Detalhe |\n|---|---|---|---|---|\n");
    for o in outcomes {
        let m = &o.manifest;
        let detail: String = m.actual.replace('|', "\\|").replace('\n', " ");
        let detail = if detail.chars().count() > 160 {
            format!("{}...", detail.chars().take(160).collect::<String>())
        } else {
            detail
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            m.test_id, m.user_story, m.machine_model, m.result, detail
        ));
    }
    let regressions: Vec<&str> = outcomes
        .iter()
        .filter(|o| o.manifest.expected.contains("[REGRESSÃO"))
        .map(|o| o.manifest.test_id.as_str())
        .collect();
    if !regressions.is_empty() {
        s.push_str(&format!(
            "\nCasos de regressão (a referência é saída anterior da Máquina Rust, não o AppleWin): {}\n",
            regressions.join(", ")
        ));
    }
    s
}

/// Writes `<id>.manifest.json` for every outcome (validated against the schema), the artifacts,
/// `junit.xml` and `summary.md` into `dir`.
pub fn write_report(dir: &Path, outcomes: &[Outcome]) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for o in outcomes {
        let json = o.manifest.validated_json().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "manifest of {} violates the schema: {}",
                    o.manifest.test_id,
                    e.join("; ")
                ),
            )
        })?;
        std::fs::write(
            dir.join(format!("{}.manifest.json", o.manifest.test_id)),
            json,
        )?;
        for (name, bytes) in &o.artifacts {
            std::fs::write(dir.join(name), bytes)?;
        }
    }
    std::fs::write(dir.join("junit.xml"), junit_xml(outcomes))?;
    std::fs::write(dir.join("summary.md"), summary_md(outcomes))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    fn outcome(id: &str, r: TestResult, actual: &str) -> Outcome {
        Outcome {
            manifest: Manifest {
                test_id: id.into(),
                user_story: "HU-0101".into(),
                acceptance_criterion: "CA".into(),
                applewin_sha: None,
                rust_sha: "sha".into(),
                fixture_sha256: None,
                rom_sha256: None,
                machine_model: "apple2plus".into(),
                cpu_model: "6502".into(),
                cards_slots: "none".into(),
                host_os: "linux".into(),
                host_arch: "x86_64".into(),
                toolchain: "rustc".into(),
                input_trace: None,
                command: "c".into(),
                expected: "e".into(),
                actual: actual.into(),
                diff_artifact: (r == TestResult::Fail).then(|| format!("{id}.diff.txt")),
                result: r,
                timestamp_utc: "2026-10-20T12:00:00Z".into(),
                reviewer: None,
            },
            artifacts: vec![(format!("{id}.diff.txt"), b"d".to_vec())],
            seconds: 0.5,
        }
    }

    fn sample() -> Vec<Outcome> {
        vec![
            outcome("ok", TestResult::Pass, "fine"),
            outcome("bad", TestResult::Fail, "x < y & \"z\""),
            outcome("blk", TestResult::Blocked, "BLOCKED: fixture ausente"),
            outcome("unv", TestResult::Unverified, "-"),
        ]
    }

    #[test]
    fn counts_every_result() {
        let c = counts(&sample());
        assert_eq!(c[&TestResult::Pass], 1);
        assert_eq!(c[&TestResult::Fail], 1);
        assert_eq!(c[&TestResult::Blocked], 1);
        assert_eq!(c[&TestResult::Unverified], 1);
        assert_eq!(c[&TestResult::NotApplicable], 0);
    }

    #[test]
    fn junit_structure_and_escaping() {
        let x = junit_xml(&sample());
        assert!(x.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites "));
        assert!(x.contains("tests=\"4\" failures=\"1\" errors=\"1\" skipped=\"1\""));
        assert!(x.contains("<testcase classname=\"HU-0101\" name=\"ok\" time=\"0.500\"/>"));
        assert!(
            x.contains("<failure message=\"FAIL: x &lt; y &amp; &quot;z&quot;\" type=\"FAIL\">")
        );
        assert!(
            x.contains("<error message=\"BLOCKED: BLOCKED: fixture ausente\" type=\"BLOCKED\">")
        );
        assert!(x.contains("<skipped message=\"UNVERIFIED: -\" type=\"UNVERIFIED\">"));
        assert!(x.contains("diff_artifact: bad.diff.txt"));
        assert!(x.trim_end().ends_with("</testsuites>"));
        assert_eq!(
            x.matches("<testcase").count(),
            x.matches("</testcase>").count() + 1
        );
    }

    #[test]
    fn xml_escape_drops_control_characters() {
        assert_eq!(xml_escape("a\u{1}b<\n"), "ab&lt;\n");
    }

    #[test]
    fn summary_lists_counts_and_cases() {
        let s = summary_md(&sample());
        assert!(s.contains("| PASS | 1 |"));
        assert!(s.contains("| FAIL | 1 |"));
        assert!(s.contains("| NOT_APPLICABLE | 0 |"));
        assert!(s.contains("| bad | HU-0101 | apple2plus | FAIL |"));
        assert!(!s.contains("Casos de regressão"));
        let mut o = sample();
        o[0].manifest.expected = "x [REGRESSÃO: y]".into();
        assert!(summary_md(&o).contains("Casos de regressão"));
    }

    #[test]
    fn write_report_creates_all_files() {
        let dir = std::env::temp_dir().join(format!("conf-report-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_report(&dir, &sample()).unwrap();
        for f in [
            "junit.xml",
            "summary.md",
            "ok.manifest.json",
            "bad.manifest.json",
            "bad.diff.txt",
        ] {
            assert!(dir.join(f).exists(), "{f}");
        }
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("bad.manifest.json")).unwrap())
                .unwrap();
        assert_eq!(v["result"], "FAIL");
        assert_eq!(v["diff_artifact"], "bad.diff.txt");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_report_refuses_manifests_that_break_the_schema() {
        let mut o = sample();
        o[0].manifest.test_id.clear();
        let dir = std::env::temp_dir().join(format!("conf-report-bad-{}", std::process::id()));
        assert!(write_report(&dir, &o).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
