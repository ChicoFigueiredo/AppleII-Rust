//! Run manifest (plan section 17): one JSON document per executed test, and a small validator for
//! the JSON Schema in `tests/conformance/manifests/schema.json` (just the keywords that schema
//! uses: `type`, `required`, `properties`, `additionalProperties`, `enum`, `minLength`, `pattern`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// The schema, embedded so the binary validates what it writes.
pub const SCHEMA_JSON: &str = include_str!("../../../tests/conformance/manifests/schema.json");

/// Verification state (GLOSSARY: Estado de verificação).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TestResult {
    Pass,
    Fail,
    Unverified,
    NotApplicable,
    Blocked,
    Implemented,
}

impl TestResult {
    pub const ALL: [TestResult; 6] = [
        TestResult::Pass,
        TestResult::Fail,
        TestResult::Unverified,
        TestResult::NotApplicable,
        TestResult::Blocked,
        TestResult::Implemented,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TestResult::Pass => "PASS",
            TestResult::Fail => "FAIL",
            TestResult::Unverified => "UNVERIFIED",
            TestResult::NotApplicable => "NOT_APPLICABLE",
            TestResult::Blocked => "BLOCKED",
            TestResult::Implemented => "IMPLEMENTED",
        }
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The manifest, with the fields of plan section 17 in that order. Fields without a known value
/// serialize as `null` (never omitted).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub test_id: String,
    pub user_story: String,
    pub acceptance_criterion: String,
    pub applewin_sha: Option<String>,
    pub rust_sha: String,
    pub fixture_sha256: Option<String>,
    pub rom_sha256: Option<String>,
    pub machine_model: String,
    pub cpu_model: String,
    pub cards_slots: String,
    pub host_os: String,
    pub host_arch: String,
    pub toolchain: String,
    pub input_trace: Option<String>,
    pub command: String,
    pub expected: String,
    pub actual: String,
    pub diff_artifact: Option<String>,
    pub result: TestResult,
    pub timestamp_utc: String,
    pub reviewer: Option<String>,
}

impl Manifest {
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("manifest serializes") + "\n"
    }

    /// Serializes and validates against the embedded schema.
    pub fn validated_json(&self) -> Result<String, Vec<String>> {
        let v = serde_json::to_value(self).expect("manifest serializes");
        validate_against_schema(&v)?;
        Ok(self.to_json_pretty())
    }
}

/// Validates `doc` against the embedded schema; errors are `path: message`.
pub fn validate_against_schema(doc: &Value) -> Result<(), Vec<String>> {
    let schema: Value = serde_json::from_str(SCHEMA_JSON).expect("embedded schema is valid JSON");
    validate(doc, &schema)
}

/// Validates `doc` against `schema` (subset described in the module docs).
pub fn validate(doc: &Value, schema: &Value) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    check(doc, schema, "$", &mut errs);
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_matches(v: &Value, t: &str) -> bool {
    let n = type_name(v);
    n == t || (t == "number" && n == "integer")
}

fn check(doc: &Value, schema: &Value, path: &str, errs: &mut Vec<String>) {
    if let Some(t) = schema.get("type") {
        let allowed: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if !allowed.iter().any(|t| type_matches(doc, t)) {
            errs.push(format!(
                "{path}: type {} not in {allowed:?}",
                type_name(doc)
            ));
            return;
        }
    }
    if let Some(Value::Array(options)) = schema.get("enum") {
        if !options.contains(doc) {
            errs.push(format!("{path}: {doc} not in enum {options:?}"));
        }
    }
    if let Value::String(s) = doc {
        if let Some(min) = schema.get("minLength").and_then(Value::as_u64) {
            if (s.chars().count() as u64) < min {
                errs.push(format!("{path}: shorter than {min}"));
            }
        }
        if let Some(p) = schema.get("pattern").and_then(Value::as_str) {
            match mini_regex_match(p, s) {
                Ok(true) => {}
                Ok(false) => errs.push(format!("{path}: `{s}` does not match {p}")),
                Err(e) => errs.push(format!("{path}: unsupported pattern {p}: {e}")),
            }
        }
    }
    if let Value::Object(obj) = doc {
        let props = schema.get("properties").and_then(Value::as_object);
        if let Some(Value::Array(req)) = schema.get("required") {
            for r in req.iter().filter_map(Value::as_str) {
                if !obj.contains_key(r) {
                    errs.push(format!("{path}: missing required field `{r}`"));
                }
            }
        }
        for (k, v) in obj {
            match props.and_then(|p| p.get(k)) {
                Some(sub) => check(v, sub, &format!("{path}.{k}"), errs),
                None => {
                    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                        errs.push(format!("{path}: unexpected field `{k}`"));
                    }
                }
            }
        }
    }
}

/// Matches the regex subset the schema uses: optional `^`/`$`, literal characters, `[...]`
/// classes with ranges, and `{n}` repetition. Anything else is an error.
fn mini_regex_match(pat: &str, text: &str) -> Result<bool, String> {
    enum Atom {
        Lit(char),
        Class(Vec<(char, char)>),
    }
    let p: Vec<char> = pat.chars().collect();
    let mut i = 0;
    let anchored_start = p.first() == Some(&'^');
    if anchored_start {
        i = 1;
    }
    let mut atoms: Vec<(Atom, usize)> = Vec::new();
    let mut anchored_end = false;
    while i < p.len() {
        let atom = match p[i] {
            '$' if i + 1 == p.len() => {
                anchored_end = true;
                break;
            }
            '[' => {
                let end = p[i..]
                    .iter()
                    .position(|&c| c == ']')
                    .ok_or("unterminated class")?
                    + i;
                let body = &p[i + 1..end];
                let mut ranges = Vec::new();
                let mut j = 0;
                while j < body.len() {
                    if j + 2 < body.len() && body[j + 1] == '-' {
                        ranges.push((body[j], body[j + 2]));
                        j += 3;
                    } else {
                        ranges.push((body[j], body[j]));
                        j += 1;
                    }
                }
                i = end + 1;
                Atom::Class(ranges)
            }
            '\\' => {
                i += 2;
                Atom::Lit(*p.get(i - 1).ok_or("dangling escape")?)
            }
            c if "(){}|*+?.".contains(c) => return Err(format!("`{c}` not supported")),
            c => {
                i += 1;
                Atom::Lit(c)
            }
        };
        let mut count = 1;
        if p.get(i) == Some(&'{') {
            let end = p[i..]
                .iter()
                .position(|&c| c == '}')
                .ok_or("unterminated repetition")?
                + i;
            let n: String = p[i + 1..end].iter().collect();
            count = n.parse().map_err(|_| "bad repetition")?;
            i = end + 1;
        }
        atoms.push((atom, count));
    }
    let t: Vec<char> = text.chars().collect();
    let mut pos = 0;
    for (atom, count) in &atoms {
        for _ in 0..*count {
            let Some(&c) = t.get(pos) else {
                return Ok(false);
            };
            let ok = match atom {
                Atom::Lit(l) => *l == c,
                Atom::Class(r) => r.iter().any(|&(a, b)| a <= c && c <= b),
            };
            if !ok {
                return Ok(false);
            }
            pos += 1;
        }
    }
    Ok(!anchored_end || pos == t.len())
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a Unix time in seconds.
pub fn format_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Current time, `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_utc(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub fn sample() -> Manifest {
        Manifest {
            test_id: "t-1".into(),
            user_story: "HU-0101".into(),
            acceptance_criterion: "CA-1".into(),
            applewin_sha: None,
            rust_sha: "abc123".into(),
            fixture_sha256: None,
            rom_sha256: Some("0".repeat(64)),
            machine_model: "apple2e-enhanced".into(),
            cpu_model: "65C02".into(),
            cards_slots: "none".into(),
            host_os: "linux".into(),
            host_arch: "x86_64".into(),
            toolchain: "rustc 1.90.0".into(),
            input_trace: None,
            command: "conformance run --case t-1".into(),
            expected: "x".into(),
            actual: "y".into(),
            diff_artifact: None,
            result: TestResult::Pass,
            timestamp_utc: "2026-10-20T12:00:00Z".into(),
            reviewer: None,
        }
    }

    #[test]
    fn serializes_exactly_the_schema_fields_in_order() {
        let json = sample().to_json_pretty();
        let v: Value = serde_json::from_str(&json).unwrap();
        let schema: Value = serde_json::from_str(SCHEMA_JSON).unwrap();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(required.len(), 21);
        // serde_json sorts keys alphabetically in `Value`; compare as sets, then check textual order.
        let mut a = keys.clone();
        a.sort();
        let mut b = required.clone();
        b.sort();
        assert_eq!(a, b);
        let mut last = 0;
        for r in &required {
            let at = json.find(&format!("\"{r}\":")).unwrap();
            assert!(at >= last, "{r} out of order");
            last = at;
        }
        assert!(json.contains("\"applewin_sha\": null"));
        assert!(json.contains("\"result\": \"PASS\""));
    }

    #[test]
    fn schema_required_matches_the_plan() {
        let schema: Value = serde_json::from_str(SCHEMA_JSON).unwrap();
        let req: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert_eq!(
            req,
            "test_id user_story acceptance_criterion applewin_sha rust_sha fixture_sha256 rom_sha256 machine_model cpu_model cards_slots host_os host_arch toolchain input_trace command expected actual diff_artifact result timestamp_utc reviewer"
                .split(' ')
                .collect::<Vec<_>>()
        );
        let en: Vec<&str> = schema["properties"]["result"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let all: Vec<&str> = TestResult::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(en, all);
    }

    #[test]
    fn valid_manifest_passes_the_schema_for_every_result() {
        for r in TestResult::ALL {
            let mut m = sample();
            m.result = r;
            assert!(m.validated_json().is_ok(), "{r}");
        }
    }

    #[test]
    fn schema_rejects_missing_extra_and_bad_values() {
        let good = serde_json::to_value(sample()).unwrap();
        for k in good.as_object().unwrap().keys() {
            let mut v = good.clone();
            v.as_object_mut().unwrap().remove(k);
            let e = validate_against_schema(&v).unwrap_err();
            assert!(e[0].contains(k), "{k}: {e:?}");
        }
        let mut v = good.clone();
        v["result"] = json!("OK");
        assert!(validate_against_schema(&v).is_err());
        v["result"] = json!("pass");
        assert!(validate_against_schema(&v).is_err());
        let mut v = good.clone();
        v["extra"] = json!(1);
        assert!(validate_against_schema(&v).is_err());
        let mut v = good.clone();
        v["rom_sha256"] = json!("XYZ");
        assert!(validate_against_schema(&v).is_err());
        let mut v = good.clone();
        v["timestamp_utc"] = json!("2026-10-20 12:00:00");
        assert!(validate_against_schema(&v).is_err());
        let mut v = good.clone();
        v["test_id"] = json!(5);
        assert!(validate_against_schema(&v).is_err());
        let mut v = good;
        v["test_id"] = json!("");
        assert!(validate_against_schema(&v).is_err());
    }

    #[test]
    fn manifest_round_trips_and_rejects_unknown_result() {
        let m = sample();
        let back: Manifest = serde_json::from_str(&m.to_json_pretty()).unwrap();
        assert_eq!(back, m);
        let bad = m.to_json_pretty().replace("\"PASS\"", "\"MAYBE\"");
        assert!(serde_json::from_str::<Manifest>(&bad).is_err());
    }

    #[test]
    fn utc_formatting() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_781_956_800), "2026-06-20T12:00:00Z");
        assert!(validate_against_schema_ts(&now_utc()));
    }

    fn validate_against_schema_ts(ts: &str) -> bool {
        let mut v = serde_json::to_value(sample()).unwrap();
        v["timestamp_utc"] = json!(ts);
        validate_against_schema(&v).is_ok()
    }
}
