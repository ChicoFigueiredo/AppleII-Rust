use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::host::MemHost;
use apple2_debugger::{Debugger, RunMode};
#[test]
fn scratch() {
    let script = std::fs::read_to_string("/tmp/claude-1000/-mnt-d-Chico-AppleRust/5f7e6844-b02c-4374-9708-0b0683d12e34/scratchpad/probe.txt").unwrap();
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut d = Debugger::new();
    d.set_host(Box::new(MemHost::default()));
    d.initialize(&mut m);
    d.enter(&mut m);
    for l in script.lines() {
        if l.is_empty() {
            continue;
        }
        d.console.clear();
        if l == "~" {
            for _ in 0..2000 {
                if d.mode != RunMode::Stepping {
                    break;
                }
                d.run(&mut m, 5000);
            }
        } else if let Some(r) = l.strip_prefix("poke ") {
            let v: Vec<u32> = r
                .split_whitespace()
                .map(|x| u32::from_str_radix(x, 16).unwrap())
                .collect();
            for (i, b) in v[1..].iter().enumerate() {
                m.poke(v[0] as u16 + i as u16, *b as u8);
            }
        } else if let Some(r) = l.strip_prefix("peek ") {
            let a = u16::from_str_radix(r, 16).unwrap();
            println!("   peek {a:04X} = {:02X}", m.peek(a));
        } else {
            let _ = d.execute(&mut m, l);
        }
        let t: Vec<_> = d.console.lines().map(line_string).collect();
        println!(
            "== {l}\n   {}   [mode {:?} pc {:04X}]",
            t.join(" | "),
            d.mode,
            m.cpu().pc
        );
    }
}
