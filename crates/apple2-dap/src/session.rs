//! The adapter state machine: DAP requests in, responses and events out.
//!
//! One [`Session`] owns one headless [`Machine`]. It is single-threaded: the binary feeds
//! requests with [`Session::handle`] and, while [`Session::is_running`], calls
//! [`Session::run_slice`] between polls of its input so that `pause` is honoured.

use crate::base64;
use crate::protocol::{error_response, event, parse_address, response};
use apple2_core::{AccessKind, AccessObserver, Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::Debugger;
use apple2_storage::disk2::Disk2Card;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

const THREAD_ID: i64 = 1;
/// Instructions executed between two polls of the input while running.
const SLICE: u32 = 20_000;
/// Largest `readMemory`/`disassemble` request served.
const MAX_BYTES: usize = 0x10000;
const MAX_FRAMES: usize = 32;

const VAR_REGISTERS: i64 = 1;
const VAR_FLAGS: i64 = 2;

/// Debugger commands that `evaluate` may run: they only read or format emulator state.
/// Nothing here (or anywhere in the adapter) starts a process or touches host files: the
/// debugger is created with `NoHost`, which refuses every file access.
const EVAL_COMMANDS: [&str; 10] = [
    "CALC", "U", "MD", "MDB", "SYM", "HELP", "BPL", "WL", "ZPL", "VER",
];

/// Bus accesses the data breakpoints watch.
#[derive(Default)]
struct Watch {
    /// Per address: bit 0 read, bit 1 write.
    map: Vec<u8>,
    hit: Option<(u16, AccessKind)>,
}

struct Obs(Rc<RefCell<Watch>>);

impl AccessObserver for Obs {
    fn access(&mut self, addr: u16, _value: u8, kind: AccessKind) {
        let mut w = self.0.borrow_mut();
        let bit = match kind {
            AccessKind::Read => 1,
            AccessKind::Write => 2,
        };
        if w.hit.is_none() && w.map.get(addr as usize).is_some_and(|f| f & bit != 0) {
            w.hit = Some((addr, kind));
        }
    }
}

enum RunKind {
    Continue,
    /// Step over a JSR: stop back at `target` with the stack at `sp`.
    Next {
        target: u16,
        sp: u8,
    },
    /// Run until the return address is reached with the stack above `sp`.
    Out {
        ret: u16,
        sp: u8,
    },
}

struct Run {
    kind: RunKind,
    /// The first instruction runs even on a breakpoint (we are resuming from it).
    first: bool,
}

pub struct Session {
    seq: i64,
    out: Vec<Value>,
    machine: Option<Machine>,
    dbg: Debugger,
    watch: Rc<RefCell<Watch>>,
    bp_source: BTreeSet<u16>,
    bp_instr: BTreeSet<u16>,
    data_bps: Vec<(u16, u8)>,
    next_bp_id: i64,
    run: Option<Run>,
    stop_on_entry: bool,
    exit: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_model(s: &str) -> Option<Model> {
    let k: String = s
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' ' | '/' | ']' | '['))
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match k.as_str() {
        "apple2" | "ii" => Model::Apple2,
        "apple2plus" | "iiplus" | "ii+" | "apple2+" => Model::Apple2Plus,
        "apple2jplus" | "jplus" => Model::Apple2JPlus,
        "apple2e" | "iie" => Model::Apple2e,
        "apple2eenhanced" | "apple2ee" | "enhanced" | "iieenhanced" => Model::Apple2eEnhanced,
        "pravets82" => Model::Pravets82,
        "pravets8m" => Model::Pravets8M,
        "pravets8a" => Model::Pravets8A,
        "base64a" => Model::Base64A,
        "tk3000e" | "tk3000" => Model::Tk3000e,
        _ => return None,
    })
}

fn hex16(a: u16) -> String {
    format!("0x{a:04X}")
}

/// Reference + byte offset -> address (None when outside the 64K space).
fn address_of(reference: &Value, offset: i64) -> Option<u16> {
    let base = match reference {
        Value::String(s) => parse_address(s)? as i64,
        Value::Number(n) => n.as_i64()?,
        _ => return None,
    };
    u16::try_from(base.checked_add(offset)?).ok()
}

impl Session {
    pub fn new() -> Self {
        Session {
            seq: 0,
            out: Vec::new(),
            machine: None,
            dbg: Debugger::new(),
            watch: Rc::new(RefCell::new(Watch::default())),
            bp_source: BTreeSet::new(),
            bp_instr: BTreeSet::new(),
            data_bps: Vec::new(),
            next_bp_id: 1,
            run: None,
            stop_on_entry: true,
            exit: false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    /// Messages produced since the last call, in order.
    pub fn take_output(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.out)
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn push_event(&mut self, name: &str, body: Value) {
        let s = self.next_seq();
        self.out.push(event(s, name, body));
    }

    fn stopped(&mut self, reason: &str, description: &str) {
        self.run = None;
        self.push_event(
            "stopped",
            json!({
                "reason": reason, "description": description,
                "threadId": THREAD_ID, "allThreadsStopped": true,
            }),
        );
    }

    // ---- requests ----

    pub fn handle(&mut self, req: &Value) {
        let cmd = req["command"].as_str().unwrap_or("");
        let args = &req["arguments"];
        let result = self.dispatch(cmd, args);
        let s = self.next_seq();
        match result {
            Ok(body) => self.out.push(response(s, req, body)),
            Err(msg) => self.out.push(error_response(s, req, &msg)),
        }
        // Events that must follow the response.
        match cmd {
            "initialize" => self.push_event("initialized", json!({})),
            "configurationDone" => {
                if self.machine.is_some() {
                    if self.stop_on_entry {
                        self.stopped("entry", "Stopped on entry");
                    } else {
                        self.run = Some(Run {
                            kind: RunKind::Continue,
                            first: true,
                        });
                    }
                }
            }
            "disconnect" | "terminate" => {
                self.push_event("terminated", json!({}));
                self.exit = true;
            }
            _ => {}
        }
    }

    fn m(&self) -> Result<&Machine, String> {
        self.machine
            .as_ref()
            .ok_or_else(|| "no machine: send launch or attach first".to_string())
    }

    fn dispatch(&mut self, cmd: &str, args: &Value) -> Result<Value, String> {
        match cmd {
            "initialize" => Ok(json!({
                "supportsConfigurationDoneRequest": true,
                "supportsInstructionBreakpoints": true,
                "supportsDataBreakpoints": true,
                "supportsReadMemoryRequest": true,
                "supportsWriteMemoryRequest": true,
                "supportsDisassembleRequest": true,
                "supportsSteppingGranularity": true,
                "supportsEvaluateForHovers": true,
                "supportsTerminateRequest": true,
                "exceptionBreakpointFilters": [],
            })),
            "launch" | "attach" => self.launch(args, cmd == "attach"),
            "configurationDone" | "setExceptionBreakpoints" => Ok(json!({})),
            "disconnect" | "terminate" => Ok(json!({})),
            "threads" => Ok(json!({"threads": [{"id": THREAD_ID, "name": "6502"}]})),
            "setBreakpoints" => self.set_breakpoints(args),
            "setInstructionBreakpoints" => self.set_instruction_breakpoints(args),
            "dataBreakpointInfo" => self.data_breakpoint_info(args),
            "setDataBreakpoints" => self.set_data_breakpoints(args),
            "pause" => {
                if self.run.is_some() {
                    self.stopped("pause", "Paused");
                }
                Ok(json!({}))
            }
            "continue" => self.resume(RunKind::Continue),
            "next" => self.next(),
            "stepIn" => self.step_in(),
            "stepOut" => self.step_out(),
            "stackTrace" => self.stack_trace(args),
            "scopes" => Ok(json!({"scopes": [
                {"name": "Registers", "variablesReference": VAR_REGISTERS, "expensive": false},
                {"name": "Flags", "variablesReference": VAR_FLAGS, "expensive": false},
            ]})),
            "variables" => self.variables(args),
            "readMemory" => self.read_memory(args),
            "writeMemory" => self.write_memory(args),
            "disassemble" => self.disassemble(args),
            "evaluate" => self.evaluate(args),
            other => Err(format!("unsupported request '{other}'")),
        }
    }

    fn launch(&mut self, args: &Value, attach: bool) -> Result<Value, String> {
        if attach && self.machine.is_some() {
            return Ok(json!({}));
        }
        let model = match args["model"].as_str() {
            Some(s) => parse_model(s).ok_or_else(|| format!("unknown model '{s}'"))?,
            None => Model::Apple2eEnhanced,
        };
        let mut cfg = MachineConfig::for_model(model);
        if let Some(p) = args["rom"].as_str() {
            cfg.custom_rom = Some(std::fs::read(p).map_err(|e| format!("rom {p}: {e}"))?);
        }
        let mut m = Machine::try_new(cfg).map_err(|e| format!("machine: {e:?}"))?;
        if let Some(disks) = args["disks"].as_array() {
            if !disks.is_empty() {
                m.insert_card(6, Box::new(Disk2Card::new()));
                for (i, d) in disks.iter().take(2).enumerate() {
                    let p = d.as_str().ok_or("disks must be a list of paths")?;
                    m.card_mut::<Disk2Card>(6)
                        .ok_or("no Disk II")?
                        .insert_path(i, std::path::Path::new(p), false)
                        .map_err(|e| format!("disk {p}: {e:?}"))?;
                }
            }
        }
        m.power_cycle();
        for item in args["load"].as_array().into_iter().flatten() {
            let path = item["path"].as_str().ok_or("load.path is required")?;
            let at = match &item["address"] {
                Value::String(s) => parse_address(s).ok_or("bad load.address")?,
                Value::Number(n) => n.as_u64().ok_or("bad load.address")? as u32,
                _ => 0x0800,
            };
            let bytes = std::fs::read(path).map_err(|e| format!("load {path}: {e}"))?;
            if at as usize + bytes.len() > 0x10000 {
                return Err(format!("{path} does not fit at ${at:04X}"));
            }
            for (i, b) in bytes.iter().enumerate() {
                m.poke(at as u16 + i as u16, *b);
            }
        }
        if let Some(pc) = args.get("pc").filter(|v| !v.is_null()) {
            let a = match pc {
                Value::String(s) => parse_address(s),
                Value::Number(n) => n.as_u64().map(|n| n as u32),
                _ => None,
            };
            m.cpu_mut().pc = u16::try_from(a.ok_or("bad pc")?).map_err(|_| "pc out of range")?;
        }
        self.stop_on_entry = args["stopOnEntry"].as_bool().unwrap_or(!attach);
        self.dbg = Debugger::new();
        self.dbg.initialize(&mut m);
        self.dbg.console.clear();
        self.machine = Some(m);
        self.install_watch();
        Ok(json!({}))
    }

    // ---- breakpoints ----

    fn next_id(&mut self) -> i64 {
        self.next_bp_id += 1;
        self.next_bp_id - 1
    }

    fn set_breakpoints(&mut self, args: &Value) -> Result<Value, String> {
        let src = &args["source"];
        let name = src["path"].as_str().or(src["name"].as_str()).unwrap_or("");
        let by_address = name == "memory" || name.starts_with("memory:");
        let mut lines: Vec<i64> = args["breakpoints"]
            .as_array()
            .map(|a| a.iter().filter_map(|b| b["line"].as_i64()).collect())
            .unwrap_or_default();
        if lines.is_empty() {
            lines = args["lines"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
        }
        self.bp_source.clear();
        let mut out = Vec::new();
        for l in lines {
            let id = self.next_id();
            match u16::try_from(l) {
                Ok(a) if by_address => {
                    self.bp_source.insert(a);
                    out.push(json!({"id": id, "verified": true, "line": l,
                        "instructionReference": hex16(a)}));
                }
                _ => out.push(json!({"id": id, "verified": false, "line": l,
                    "message": "only address breakpoints are supported: use source 'memory' \
                                (line = address) or instruction breakpoints"})),
            }
        }
        Ok(json!({"breakpoints": out}))
    }

    fn set_instruction_breakpoints(&mut self, args: &Value) -> Result<Value, String> {
        self.bp_instr.clear();
        let mut out = Vec::new();
        for b in args["breakpoints"].as_array().into_iter().flatten() {
            let id = self.next_id();
            let off = b["offset"].as_i64().unwrap_or(0);
            match address_of(&b["instructionReference"], off) {
                Some(a) => {
                    self.bp_instr.insert(a);
                    out.push(json!({"id": id, "verified": true,
                        "instructionReference": hex16(a)}));
                }
                None => out.push(json!({"id": id, "verified": false,
                    "message": "bad instructionReference"})),
            }
        }
        Ok(json!({"breakpoints": out}))
    }

    fn data_breakpoint_info(&mut self, args: &Value) -> Result<Value, String> {
        let name = args["name"].as_str().unwrap_or("");
        match parse_address(name).and_then(|a| u16::try_from(a).ok()) {
            Some(a) => Ok(json!({
                "dataId": format!("{a:04X}"),
                "description": format!("${a:04X}"),
                "accessTypes": ["read", "write", "readWrite"],
                "canPersist": false,
            })),
            None => Ok(json!({"dataId": null, "description": "not an address"})),
        }
    }

    fn set_data_breakpoints(&mut self, args: &Value) -> Result<Value, String> {
        self.data_bps.clear();
        let mut out = Vec::new();
        for b in args["breakpoints"].as_array().into_iter().flatten() {
            let id = self.next_id();
            let addr = b["dataId"]
                .as_str()
                .and_then(|s| u16::from_str_radix(s.trim_start_matches(['$', '0', 'x']), 16).ok())
                .or_else(|| {
                    b["dataId"]
                        .as_str()
                        .and_then(|s| u16::from_str_radix(s, 16).ok())
                });
            let access = match b["accessType"].as_str().unwrap_or("write") {
                "read" => 1,
                "write" => 2,
                _ => 3,
            };
            match addr {
                Some(a) => {
                    self.data_bps.push((a, access));
                    out.push(json!({"id": id, "verified": true}));
                }
                None => out.push(json!({"id": id, "verified": false, "message": "bad dataId"})),
            }
        }
        self.install_watch();
        Ok(json!({"breakpoints": out}))
    }

    /// Rebuilds the watch map and installs the bus observer only if a data breakpoint exists.
    fn install_watch(&mut self) {
        {
            let mut w = self.watch.borrow_mut();
            w.map = vec![0; 0x10000];
            w.hit = None;
            for (a, acc) in &self.data_bps {
                w.map[*a as usize] |= acc;
            }
        }
        let on = !self.data_bps.is_empty();
        let obs: Option<Box<dyn AccessObserver>> =
            on.then(|| Box::new(Obs(self.watch.clone())) as _);
        if let Some(m) = self.machine.as_mut() {
            m.set_access_observer(obs);
        }
    }

    // ---- execution ----

    fn resume(&mut self, kind: RunKind) -> Result<Value, String> {
        self.m()?;
        self.run = Some(Run { kind, first: true });
        Ok(json!({"allThreadsContinued": true}))
    }

    fn step_in(&mut self) -> Result<Value, String> {
        self.m()?;
        self.single_step();
        self.stopped("step", "Step");
        Ok(json!({}))
    }

    fn single_step(&mut self) {
        if let Some(m) = self.machine.as_mut() {
            self.watch.borrow_mut().hit = None;
            m.step();
        }
    }

    fn next(&mut self) -> Result<Value, String> {
        let m = self.m()?;
        let pc = m.cpu().pc;
        if m.peek(pc) == 0x20 {
            let sp = m.cpu().sp;
            self.resume(RunKind::Next {
                target: pc.wrapping_add(3),
                sp,
            })
        } else {
            self.step_in()
        }
    }

    fn step_out(&mut self) -> Result<Value, String> {
        let m = self.m()?;
        let ret = Debugger::stack_return_address(m);
        let sp = m.cpu().sp;
        self.resume(RunKind::Out { ret, sp })
    }

    /// Runs up to [`SLICE`] instructions of the current run; emits `stopped` when it ends.
    pub fn run_slice(&mut self) {
        let Some(mut run) = self.run.take() else {
            return;
        };
        let Some(m) = self.machine.as_mut() else {
            return;
        };
        let watch = self.watch.clone();
        let start = m.cycles();
        let mut stop: Option<(&str, String)> = None;
        for _ in 0..SLICE {
            let pc = m.cpu().pc;
            if !run.first && (self.bp_source.contains(&pc) || self.bp_instr.contains(&pc)) {
                stop = Some(("breakpoint", format!("Breakpoint at ${pc:04X}")));
                break;
            }
            run.first = false;
            if m.cpu().stopped {
                stop = Some(("pause", "CPU stopped (STP)".to_string()));
                break;
            }
            watch.borrow_mut().hit = None;
            m.step();
            if let Some((a, k)) = watch.borrow_mut().hit.take() {
                let what = if k == AccessKind::Read {
                    "read"
                } else {
                    "write"
                };
                stop = Some((
                    "data breakpoint",
                    format!("Data breakpoint: {what} ${a:04X}"),
                ));
                break;
            }
            let (pc, sp) = (m.cpu().pc, m.cpu().sp);
            let done = match run.kind {
                RunKind::Continue => false,
                RunKind::Next { target, sp: s } => pc == target && sp == s,
                RunKind::Out { ret, sp: s } => pc == ret && sp > s,
            };
            if done {
                stop = Some(("step", "Step".to_string()));
                break;
            }
        }
        let cycles = m.cycles() - start;
        m.update_cards(cycles);
        match stop {
            Some((reason, text)) => self.stopped(reason, &text),
            None => self.run = Some(run),
        }
    }

    // ---- inspection ----

    fn stack_trace(&mut self, args: &Value) -> Result<Value, String> {
        let m = self.m()?;
        let (pc, sp) = (m.cpu().pc, m.cpu().sp);
        let line = self.dbg.disasm_line(m, pc);
        let mut frames = vec![json!({
            "id": 1, "name": format!("${pc:04X}  {}", self.insn_text(m, pc)),
            "line": 0, "column": 0, "instructionPointerReference": hex16(pc),
            "presentationHint": "normal", "moduleId": "6502", "_bytes": line.bytes,
        })];
        // Walk the stack looking for JSR return addresses (a JSR three bytes before).
        let mut i = sp as u16 + 1;
        while i < 0xFF && frames.len() < MAX_FRAMES {
            let ret =
                ((m.peek(0x100 + i + 1) as u16) << 8 | m.peek(0x100 + i) as u16).wrapping_add(1);
            let call = ret.wrapping_sub(3);
            if m.peek(call) == 0x20 {
                frames.push(json!({
                    "id": frames.len() + 1,
                    "name": format!("${call:04X}  {}", self.insn_text(m, call)),
                    "line": 0, "column": 0, "instructionPointerReference": hex16(call),
                }));
                i += 2;
            } else {
                i += 1;
            }
        }
        for f in frames.iter_mut() {
            f.as_object_mut().map(|o| o.remove("_bytes"));
        }
        let start = args["startFrame"].as_u64().unwrap_or(0) as usize;
        let levels = args["levels"]
            .as_u64()
            .filter(|l| *l > 0)
            .map_or(usize::MAX, |l| l as usize);
        let total = frames.len();
        let page: Vec<Value> = frames.into_iter().skip(start).take(levels).collect();
        Ok(json!({"stackFrames": page, "totalFrames": total}))
    }

    fn variables(&mut self, args: &Value) -> Result<Value, String> {
        let m = self.m()?;
        let c = m.cpu();
        let var = |name: &str, value: String, ty: &str| json!({"name": name, "value": value, "type": ty, "variablesReference": 0});
        let vars = match args["variablesReference"].as_i64().unwrap_or(0) {
            VAR_REGISTERS => vec![
                var("A", format!("${:02X}", c.a), "u8"),
                var("X", format!("${:02X}", c.x), "u8"),
                var("Y", format!("${:02X}", c.y), "u8"),
                var("SP", format!("${:02X}", c.sp), "u8"),
                var("PC", format!("${:04X}", c.pc), "u16"),
                var("P", format!("${:02X}", c.p), "u8"),
                var("Cycles", m.cycles().to_string(), "u64"),
            ],
            VAR_FLAGS => ['N', 'V', '-', 'B', 'D', 'I', 'Z', 'C']
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let bit = 7 - i;
                    var(&n.to_string(), ((c.p >> bit) & 1).to_string(), "bit")
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(json!({"variables": vars}))
    }

    fn read_memory(&mut self, args: &Value) -> Result<Value, String> {
        let m = self.m()?;
        let count = (args["count"].as_u64().unwrap_or(0) as usize).min(MAX_BYTES);
        let off = args["offset"].as_i64().unwrap_or(0);
        let start = address_of(&args["memoryReference"], off).ok_or("bad memoryReference")?;
        let n = count.min(0x10000 - start as usize);
        let data: Vec<u8> = (0..n).map(|i| m.peek(start + i as u16)).collect();
        Ok(json!({
            "address": hex16(start),
            "data": base64::encode(&data),
            "unreadableBytes": count - n,
        }))
    }

    fn write_memory(&mut self, args: &Value) -> Result<Value, String> {
        let data = base64::decode(args["data"].as_str().ok_or("data is required")?)
            .ok_or("data is not valid base64")?;
        let off = args["offset"].as_i64().unwrap_or(0);
        let start = address_of(&args["memoryReference"], off).ok_or("bad memoryReference")?;
        if start as usize + data.len() > 0x10000 {
            return Err("write goes past $FFFF".into());
        }
        let m = self.machine.as_mut().ok_or("no machine")?;
        for (i, b) in data.iter().enumerate() {
            m.poke(start + i as u16, *b);
        }
        Ok(json!({"bytesWritten": data.len(), "offset": 0}))
    }

    /// "LDA #$00" for the instruction at `addr`.
    fn insn_text(&self, m: &Machine, addr: u16) -> String {
        let mut l = self.dbg.disasm_line(m, addr);
        l.address.clear();
        l.opcodes.clear();
        let t = self.dbg.format_trace_disasm(&l);
        t.trim_start_matches(':').trim().to_string()
    }

    fn disassemble(&mut self, args: &Value) -> Result<Value, String> {
        let m = self.m()?;
        let off = args["offset"].as_i64().unwrap_or(0);
        let anchor = address_of(&args["memoryReference"], off).ok_or("bad memoryReference")?;
        let io = args["instructionOffset"].as_i64().unwrap_or(0);
        let count = (args["instructionCount"].as_u64().unwrap_or(0) as usize).min(4096);
        let mut addrs: Vec<u16> = Vec::new();
        let mut a = anchor;
        if io < 0 {
            let want = (-io) as usize;
            let mut found = None;
            'outer: for back in (want..=want * 3).rev() {
                let Some(mut s) = anchor
                    .checked_sub(back as u16)
                    .filter(|_| back <= anchor as usize)
                else {
                    continue;
                };
                let mut seen = Vec::new();
                while s < anchor {
                    seen.push(s);
                    s = s.wrapping_add(self.dbg.instr_len(m, s).max(1) as u16);
                }
                if s == anchor && seen.len() >= want {
                    found = Some(seen[seen.len() - want..].to_vec());
                    break 'outer;
                }
            }
            match found {
                Some(v) => addrs = v,
                None => {
                    // No clean decode: show single bytes before the anchor.
                    for k in (1..=want as u32).rev() {
                        if anchor as u32 >= k {
                            addrs.push(anchor - k as u16);
                        }
                    }
                }
            }
        }
        let before = addrs.len();
        let after = if io > 0 {
            // skip `io` instructions forward
            for _ in 0..io {
                a = a.wrapping_add(self.dbg.instr_len(m, a).max(1) as u16);
            }
            count
        } else {
            count.saturating_sub(before)
        };
        let mut a2 = if io > 0 { a } else { anchor };
        for _ in 0..after {
            addrs.push(a2);
            let step = self.dbg.instr_len(m, a2).max(1) as u16;
            let next = a2.wrapping_add(step);
            if next < a2 {
                break;
            }
            a2 = next;
        }
        let instrs: Vec<Value> = addrs
            .into_iter()
            .take(count.max(before))
            .map(|a| {
                let l = self.dbg.disasm_line(m, a);
                json!({
                    "address": hex16(a),
                    "instructionBytes": l.opcodes.trim(),
                    "instruction": self.insn_text(m, a),
                })
            })
            .collect();
        Ok(json!({"instructions": instrs}))
    }

    fn evaluate(&mut self, args: &Value) -> Result<Value, String> {
        let expr = args["expression"]
            .as_str()
            .ok_or("expression is required")?
            .trim();
        if expr.is_empty() {
            return Err("empty expression".into());
        }
        let first = expr.split_whitespace().next().unwrap_or("");
        let verb = first.to_ascii_uppercase();
        // `300:EA 60` style memory entry is a command too. Everything else that is not on the
        // list (G, T, P, RTS, file commands...) is parsed as an expression for CALC.
        let hex_colon = first
            .split_once(':')
            .is_some_and(|(a, _)| !a.is_empty() && a.chars().all(|c| c.is_ascii_hexdigit()));
        let line = if EVAL_COMMANDS.contains(&verb.as_str()) || hex_colon {
            expr.to_string()
        } else {
            format!("CALC {expr}")
        };
        let m = self
            .machine
            .as_mut()
            .ok_or("no machine: send launch or attach first")?;
        self.dbg.console.clear();
        self.dbg.execute(m, &line);
        let text: Vec<String> = self.dbg.console.lines().map(line_string).collect();
        Ok(json!({"result": text.join("\n").trim(), "variablesReference": 0}))
    }
}
