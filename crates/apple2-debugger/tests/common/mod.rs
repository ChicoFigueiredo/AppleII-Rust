#![allow(dead_code)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::console::line_string;
use apple2_debugger::display::{Grid, GRID_W};
use apple2_debugger::host::Host;
use apple2_debugger::{Debugger, Key, KeyMods, RunMode};

pub type Files = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct SharedHost(Files);

struct Sink(Files, String);

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0
            .borrow_mut()
            .entry(self.1.clone())
            .or_default()
            .extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Host for SharedHost {
    fn read(&mut self, name: &str) -> io::Result<Vec<u8>> {
        self.0
            .borrow()
            .get(name)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, name.to_string()))
    }
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.0.borrow_mut().insert(name.into(), data.to_vec());
        Ok(())
    }
    fn create(&mut self, name: &str) -> io::Result<Box<dyn Write>> {
        self.0.borrow_mut().insert(name.into(), Vec::new());
        Ok(Box::new(Sink(self.0.clone(), name.into())))
    }
    fn dir(&self) -> String {
        "/d/".into()
    }
    fn set_dir(&mut self, _: &str) -> bool {
        true
    }
}

pub const NONE: KeyMods = KeyMods {
    shift: false,
    ctrl: false,
    alt: false,
};
pub const CTRL: KeyMods = KeyMods {
    shift: false,
    ctrl: true,
    alt: false,
};

/// Apple II+ com ROM de NOPs (vetores em $0500/$0300/$0400); RAM zerada = BRK.
pub fn nop_machine() -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

pub fn setup_with(mut m: Machine) -> (Machine, Debugger, Files) {
    let files = Files::default();
    let mut d = Debugger::new();
    d.set_host(Box::new(SharedHost(files.clone())));
    d.initialize(&mut m);
    d.enter(&mut m);
    d.console.clear();
    (m, d, files)
}

pub fn setup() -> (Machine, Debugger, Files) {
    setup_with(nop_machine())
}

pub fn setup_e() -> (Machine, Debugger, Files) {
    setup_with(Machine::new(MachineConfig::for_model(
        Model::Apple2eEnhanced,
    )))
}

pub fn load(m: &mut Machine, at: u16, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        m.poke(at.wrapping_add(i as u16), *b);
    }
}

pub fn text(d: &Debugger) -> String {
    d.console
        .lines()
        .map(line_string)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn cmd(d: &mut Debugger, m: &mut Machine, line: &str) -> String {
    d.console.clear();
    d.execute(m, line);
    text(d)
}

/// Roda fatias limitadas até o depurador parar (falha em vez de travar a suíte).
pub fn run_until_stopped(d: &mut Debugger, m: &mut Machine) {
    for _ in 0..4000 {
        if d.mode != RunMode::Stepping {
            return;
        }
        d.run(m, 1000);
    }
    panic!("o depurador não parou");
}

pub fn key(d: &mut Debugger, m: &mut Machine, k: Key, mods: KeyMods) {
    d.process_key(m, k, mods);
}

pub fn row(g: &Grid, y: usize) -> String {
    (0..GRID_W).map(|x| g.cell(x, y).ch).collect()
}

pub fn screen(g: &Grid) -> String {
    (0..g.h).map(|y| row(g, y)).collect::<Vec<_>>().join("\n")
}
