//! Automated DAP client: starts `applewin-dap`, talks to it over stdio and checks breakpoints,
//! stepping, registers, memory, consistency with the native debugger and the shutdown.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_dap::{base64, protocol};
use apple2_debugger::Debugger;
use serde_json::{json, Value};
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

static NEXT_CLIENT: AtomicUsize = AtomicUsize::new(0);

const TIMEOUT: Duration = Duration::from_secs(20);

/// ```text
/// 0300 LDX #$05     0302 JSR $0310   0305 INX   0306 STX $10   0308 JMP $0308
/// 0310 DEX          0311 BNE $0310   0313 LDA #$42   0315 RTS
/// ```
const PROGRAM: [(u16, &[u8]); 2] = [
    (
        0x0300,
        &[
            0xA2, 0x05, 0x20, 0x10, 0x03, 0xE8, 0x86, 0x10, 0x4C, 0x08, 0x03,
        ],
    ),
    (0x0310, &[0xCA, 0xD0, 0xFD, 0xA9, 0x42, 0x60]),
];

struct Client {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    seq: i64,
    id: usize,
    events: Vec<Value>,
}

impl Client {
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_applewin-dap"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn applewin-dap");
        let stdin = child.stdin.take().unwrap();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(Some(m)) = protocol::read_message(&mut out) {
                if tx.send(m).is_err() {
                    break;
                }
            }
        });
        Client {
            child,
            stdin,
            rx,
            seq: 0,
            id: NEXT_CLIENT.fetch_add(1, Ordering::Relaxed),
            events: Vec::new(),
        }
    }

    fn send(&mut self, command: &str, arguments: Value) -> i64 {
        self.seq += 1;
        let msg = json!({"seq": self.seq, "type": "request", "command": command,
            "arguments": arguments});
        protocol::write_message(&mut self.stdin, &msg).unwrap();
        self.seq
    }

    fn next_message(&mut self) -> Value {
        self.rx.recv_timeout(TIMEOUT).expect("adapter went silent")
    }

    /// Sends a request and returns its response (events seen meanwhile are queued).
    fn request(&mut self, command: &str, arguments: Value) -> Value {
        let seq = self.send(command, arguments);
        loop {
            let m = self.next_message();
            match m["type"].as_str() {
                Some("response") if m["request_seq"] == seq => return m,
                Some("event") => self.events.push(m),
                _ => {}
            }
        }
    }

    fn ok(&mut self, command: &str, arguments: Value) -> Value {
        let r = self.request(command, arguments);
        assert_eq!(r["success"], true, "{command} failed: {r}");
        r["body"].clone()
    }

    fn wait_event(&mut self, name: &str) -> Value {
        if let Some(i) = self.events.iter().position(|e| e["event"] == name) {
            return self.events.remove(i);
        }
        loop {
            let m = self.next_message();
            if m["type"] == "event" {
                if m["event"] == name {
                    return m;
                }
                self.events.push(m);
            }
        }
    }

    fn registers(&mut self) -> Registers {
        let vars = self.ok("variables", json!({"variablesReference": 1}))["variables"].clone();
        let get = |n: &str| -> u32 {
            let v = vars
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == n)
                .unwrap()["value"]
                .as_str()
                .unwrap()
                .to_string();
            u32::from_str_radix(v.trim_start_matches('$'), 16).unwrap()
        };
        Registers {
            a: get("A") as u8,
            x: get("X") as u8,
            y: get("Y") as u8,
            sp: get("SP") as u8,
            pc: get("PC") as u16,
            p: get("P") as u8,
        }
    }

    /// Writes the test program to disk, initializes and launches stopped on entry.
    fn launch_program(&mut self) {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
        let mut args = json!({"model": "apple2plus", "pc": 768, "stopOnEntry": true, "load": []});
        for (addr, bytes) in PROGRAM {
            // One file per client and address: tests run in parallel inside one process, and a
            // shared name let one test truncate the program while another adapter loaded it.
            let p = dir.join(format!(
                "dap-prog-{addr:04x}-{}-{}.bin",
                std::process::id(),
                self.id
            ));
            std::fs::write(&p, bytes).unwrap();
            args["load"]
                .as_array_mut()
                .unwrap()
                .push(json!({"path": p.to_str().unwrap(), "address": addr}));
        }
        let caps = self.ok("initialize", json!({"adapterID": "applewin"}));
        assert_eq!(caps["supportsReadMemoryRequest"], true);
        self.ok("launch", args);
        self.wait_event("initialized");
        self.ok("configurationDone", json!({}));
        let stop = self.wait_event("stopped");
        assert_eq!(stop["body"]["reason"], "entry");
    }

    fn finish(mut self) {
        let r = self.request("disconnect", json!({}));
        assert_eq!(r["success"], true);
        self.wait_event("terminated");
        let status = wait_exit(&mut self.child);
        assert!(status.success(), "adapter exit status {status:?}");
    }
}

fn wait_exit(child: &mut Child) -> std::process::ExitStatus {
    for _ in 0..200 {
        if let Some(s) = child.try_wait().unwrap() {
            return s;
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.kill().ok();
    panic!("the adapter did not exit after disconnect");
}

#[derive(Debug, PartialEq, Eq)]
struct Registers {
    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    pc: u16,
    p: u8,
}

fn native_machine() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    m.power_cycle();
    for (addr, bytes) in PROGRAM {
        for (i, b) in bytes.iter().enumerate() {
            m.poke(addr + i as u16, *b);
        }
    }
    m.cpu_mut().pc = 0x0300;
    m
}

fn native_regs(m: &Machine) -> Registers {
    let c = m.cpu();
    Registers {
        a: c.a,
        x: c.x,
        y: c.y,
        sp: c.sp,
        pc: c.pc,
        p: c.p,
    }
}

#[test]
fn instruction_breakpoint_step_registers_memory_and_shutdown() {
    let mut c = Client::start();
    c.launch_program();
    let r = c.registers();
    assert_eq!(r.pc, 0x0300);

    // breakpoint after the subroutine
    let b = c.ok(
        "setInstructionBreakpoints",
        json!({"breakpoints": [{"instructionReference": "0x0305"}]}),
    );
    assert_eq!(b["breakpoints"][0]["verified"], true);
    c.ok("continue", json!({"threadId": 1}));
    let stop = c.wait_event("stopped");
    assert_eq!(stop["body"]["reason"], "breakpoint");
    let r = c.registers();
    assert_eq!((r.pc, r.a, r.x), (0x0305, 0x42, 0x00));

    // step in executes INX
    c.ok("stepIn", json!({"threadId": 1}));
    assert_eq!(c.wait_event("stopped")["body"]["reason"], "step");
    let r = c.registers();
    assert_eq!((r.pc, r.x), (0x0306, 0x01));

    // memory: read the program, write, read back
    let mem = c.ok(
        "readMemory",
        json!({"memoryReference": "0x0300", "count": 3}),
    );
    assert_eq!(
        base64::decode(mem["data"].as_str().unwrap()).unwrap(),
        [0xA2, 0x05, 0x20]
    );
    c.ok(
        "writeMemory",
        json!({"memoryReference": "0x0400", "data": base64::encode(&[1, 2, 3, 4])}),
    );
    let mem = c.ok(
        "readMemory",
        json!({"memoryReference": "0x0400", "offset": 1, "count": 2}),
    );
    assert_eq!(
        base64::decode(mem["data"].as_str().unwrap()).unwrap(),
        [2, 3]
    );

    // disassembly with the debugger's disassembler
    let d = c.ok(
        "disassemble",
        json!({"memoryReference": "0x0300", "instructionCount": 3}),
    );
    let ins = d["instructions"].as_array().unwrap();
    assert_eq!(ins.len(), 3);
    assert_eq!(ins[0]["instruction"], "LDX #$05");
    assert_eq!(ins[0]["instructionBytes"], "A2 05");
    assert_eq!(ins[1]["instruction"], "JSR $0310");
    assert_eq!(ins[2]["address"], "0x0305");

    // data breakpoint on the write to $10, then the stack frame view
    let info = c.ok("dataBreakpointInfo", json!({"name": "0x0010"}));
    c.ok(
        "setDataBreakpoints",
        json!({"breakpoints": [{"dataId": info["dataId"], "accessType": "write"}]}),
    );
    c.ok("continue", json!({"threadId": 1}));
    let stop = c.wait_event("stopped");
    assert_eq!(stop["body"]["reason"], "data breakpoint", "{stop}");
    assert_eq!(c.registers().pc, 0x0308);
    let st = c.ok("stackTrace", json!({"threadId": 1}));
    assert_eq!(
        st["stackFrames"][0]["instructionPointerReference"],
        "0x0308"
    );

    // evaluate: debugger expressions only; a command that would run code is just an expression
    let e = c.ok("evaluate", json!({"expression": "1+2", "context": "repl"}));
    assert!(!e["result"].as_str().unwrap().is_empty());
    let pc_before = c.registers().pc;
    let e = c.request("evaluate", json!({"expression": "G", "context": "repl"}));
    assert_eq!(
        c.registers().pc,
        pc_before,
        "evaluate must not run the machine: {e}"
    );
    let e = c.ok(
        "evaluate",
        json!({"expression": "400:EE", "context": "repl"}),
    );
    let _ = e;
    let mem = c.ok(
        "readMemory",
        json!({"memoryReference": "0x0400", "count": 1}),
    );
    assert_eq!(
        base64::decode(mem["data"].as_str().unwrap()).unwrap(),
        [0xEE]
    );

    c.finish();
}

#[test]
fn step_over_and_step_out() {
    let mut c = Client::start();
    c.launch_program();
    // LDX; then `next` on the JSR runs the whole subroutine
    c.ok("next", json!({"threadId": 1}));
    c.wait_event("stopped");
    assert_eq!(c.registers().pc, 0x0302);
    c.ok("next", json!({"threadId": 1}));
    assert_eq!(c.wait_event("stopped")["body"]["reason"], "step");
    let r = c.registers();
    assert_eq!((r.pc, r.a, r.x), (0x0305, 0x42, 0));

    // go back to the start via a new session state: step into the subroutine and out of it
    let mut c2 = Client::start();
    c2.launch_program();
    c2.ok("stepIn", json!({"threadId": 1}));
    c2.wait_event("stopped");
    c2.ok("stepIn", json!({"threadId": 1}));
    c2.wait_event("stopped");
    assert_eq!(c2.registers().pc, 0x0310);
    let st = c2.ok("stackTrace", json!({"threadId": 1}));
    assert_eq!(st["totalFrames"], 2);
    assert_eq!(
        st["stackFrames"][1]["instructionPointerReference"],
        "0x0302"
    );
    c2.ok("stepOut", json!({"threadId": 1}));
    c2.wait_event("stopped");
    assert_eq!(c2.registers().pc, 0x0305);
    c.finish();
    c2.finish();
}

#[test]
fn pause_stops_a_running_machine() {
    let mut c = Client::start();
    c.launch_program();
    c.ok("continue", json!({"threadId": 1}));
    thread::sleep(Duration::from_millis(100));
    c.ok("pause", json!({"threadId": 1}));
    assert_eq!(c.wait_event("stopped")["body"]["reason"], "pause");
    assert_eq!(c.registers().pc, 0x0308); // the program ends in `JMP $0308`
    c.finish();
}

#[test]
fn source_breakpoints_by_address_and_errors() {
    let mut c = Client::start();
    // requests before launch fail cleanly
    let r = c.request("readMemory", json!({"memoryReference": "0x0", "count": 1}));
    assert_eq!(r["success"], false);
    c.launch_program();
    let b = c.ok(
        "setBreakpoints",
        json!({"source": {"path": "memory"}, "breakpoints": [{"line": 0x0306}]}),
    );
    assert_eq!(b["breakpoints"][0]["verified"], true);
    let b = c.ok(
        "setBreakpoints",
        json!({"source": {"path": "game.s"}, "breakpoints": [{"line": 3}]}),
    );
    assert_eq!(b["breakpoints"][0]["verified"], false);
    c.ok(
        "setBreakpoints",
        json!({"source": {"path": "memory"}, "breakpoints": [{"line": 0x0306}]}),
    );
    c.ok("continue", json!({"threadId": 1}));
    assert_eq!(c.wait_event("stopped")["body"]["reason"], "breakpoint");
    assert_eq!(c.registers().pc, 0x0306);
    let r = c.request("bogus", json!({}));
    assert_eq!(r["success"], false);
    c.finish();
}

#[test]
fn same_steps_as_the_native_debugger() {
    let mut c = Client::start();
    c.launch_program();

    let mut m = native_machine();
    let mut dbg = Debugger::new();
    dbg.initialize(&mut m);
    dbg.enter(&mut m);
    assert_eq!(c.registers(), native_regs(&m), "initial state");

    // stepIn x2, next (over the JSR is not at pc now), stepIn x3, stepOut
    for i in 0..2 {
        c.ok("stepIn", json!({"threadId": 1}));
        c.wait_event("stopped");
        dbg.trace_n(&mut m, 1);
        assert_eq!(c.registers(), native_regs(&m), "stepIn #{i}");
    }
    // at $0310 inside the subroutine: three more steps
    for i in 0..3 {
        c.ok("stepIn", json!({"threadId": 1}));
        c.wait_event("stopped");
        dbg.trace_n(&mut m, 1);
        assert_eq!(c.registers(), native_regs(&m), "stepIn (sub) #{i}");
    }
    // step out of the subroutine (native: RTS command then the stepping loop)
    c.ok("stepOut", json!({"threadId": 1}));
    c.wait_event("stopped");
    dbg.execute(&mut m, "RTS");
    dbg.run(&mut m, 1_000_000);
    assert_eq!(c.registers(), native_regs(&m), "stepOut");
    assert_eq!(native_regs(&m).pc, 0x0305);

    // and the same over a JSR: restart both machines
    let mut c2 = Client::start();
    c2.launch_program();
    let mut m2 = native_machine();
    let mut dbg2 = Debugger::new();
    dbg2.initialize(&mut m2);
    dbg2.enter(&mut m2);
    for _ in 0..2 {
        c2.ok("next", json!({"threadId": 1}));
        c2.wait_event("stopped");
        dbg2.step_over_n(&mut m2, 1);
        assert_eq!(c2.registers(), native_regs(&m2), "next");
    }
    c.finish();
    c2.finish();
}
