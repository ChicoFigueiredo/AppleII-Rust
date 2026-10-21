//! DAP wire format: `Content-Length: N\r\n\r\n` followed by N bytes of JSON.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Reads one message; `Ok(None)` at a clean end of input.
pub fn read_message(r: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            if len.is_some() {
                break;
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().ok();
            }
        }
    }
    let mut body = vec![0u8; len.unwrap_or(0)];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_message(w: &mut impl Write, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(msg)?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()
}

/// A successful response.
pub fn response(seq: i64, req: &Value, body: Value) -> Value {
    json!({
        "seq": seq, "type": "response",
        "request_seq": req["seq"], "command": req["command"],
        "success": true, "body": body,
    })
}

/// A failed response.
pub fn error_response(seq: i64, req: &Value, message: &str) -> Value {
    json!({
        "seq": seq, "type": "response",
        "request_seq": req["seq"], "command": req["command"],
        "success": false, "message": message,
    })
}

pub fn event(seq: i64, name: &str, body: Value) -> Value {
    json!({ "seq": seq, "type": "event", "event": name, "body": body })
}

/// Parses `0x1F00`, `$1F00` or decimal.
pub fn parse_address(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else if let Some(h) = s.strip_prefix('$') {
        u32::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}
