//! Commands that reach outside the CPU/memory model: `BENCHMARK`, `DISK`, `FONT`, `HCOLOR`,
//! `NTSC`, `SOURCE`, `SYNC` (Debug.cpp:764-800, 2950, 3925-4045, 5323-5850, 7009-7085;
//! Debugger_Win32.cpp:43-312).
//!
//! The port keeps the AppleWin semantics wherever the target exists here; the `Win32`/GDI parts
//! (fonts) are the built-in Apple II font, which is what AppleWin itself uses when
//! `USE_APPLE_FONT` is on (and `OLD_FONT`, the GDI path, is compiled out).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use apple2_core::Machine;
use apple2_storage::disk2::{Disk2Card, DiskStatus};

use crate::args::*;
use crate::cmdtable::Cmd;
use crate::color::DebugColor;
use crate::console::*;
use crate::debugger::*;
use crate::symbols::Table;

/// Source level debugging state (`g_bSourceLevelDebugging`, `g_AssemblerSourceBuffer`,
/// `g_aSourceDebug`, ...).
#[derive(Clone, Debug, Default)]
pub struct SourceState {
    pub active: bool,
    pub file_name: String,
    /// Non-empty lines of the listing (`MemoryTextFile_t` drops the empty ones).
    pub lines: Vec<String>,
    /// Address to line number (`g_aSourceDebug`).
    pub by_addr: BTreeMap<u16, usize>,
    add_memory: bool,
    add_symbols: bool,
    pub symbols: usize,
    pub bytes: usize,
}

/// `CONSOLE_FONT_WIDTH` / `CONSOLE_FONT_HEIGHT`: the Apple II font cell used for every `FONT_*`.
const FONT_WIDTH: usize = 7;
const FONT_HEIGHT: usize = 8;
const NUM_FONTS: usize = 4;
const NUM_FONT_SPACING: u16 = 3;

/// `benchopcode[]` (CPU.cpp:112): compressed 6502 code for `CpuSetupBenchmark`.
const BENCH_OPCODES: [u8; 33] = [
    0x06, 0x16, 0x24, 0x45, 0x48, 0x65, 0x68, 0x76, 0x84, 0x85, 0x86, 0x91, 0x94, 0xA4, 0xA5, 0xA6,
    0xB1, 0xB4, 0xC0, 0xC4, 0xC5, 0xE6, 0x19, 0x6D, 0x8D, 0x99, 0x9D, 0xAD, 0xB9, 0xBD, 0xDD, 0xED,
    0xEE,
];
const SHORT_OPCODES: usize = 22;
/// Cycles the timed benchmark runs (about one second of an Apple II).
const BENCH_CYCLES: u64 = 1_000_000;
const NTSC_CLOCK_HZ: f64 = 1_020_484.4;

/// `g_nChromaSize` of the hue table: 4 phases x 4096 sequences x 4 bytes.
const CHROMA_SIZE: usize = 4 * 4096 * 4;
const DEFAULT_PALETTE_FILE: &str = "AppleWinNTSC4096x4@32.data";

/// The `_PARAM_GENERAL_BEGIN.._END` words (Debugger_Commands.cpp:477-489).
const GENERAL_PARAMS: [&str; 13] = [
    "FIND", "BRANCH", "CATEGORY", "CLEAR", "LOAD", "LIST", "OFF", "ON", "RESET", "SAVE", "START",
    "STOP", "ALL",
];

fn bmp_header(w: u32, h: u32, bpp: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(54);
    v.extend_from_slice(b"BM");
    v.extend_from_slice(&0u32.to_le_bytes()); // file size: 0 = ignore
    v.extend_from_slice(&0u32.to_le_bytes()); // reserved
    v.extend_from_slice(&54u32.to_le_bytes()); // offset to the pixels
    v.extend_from_slice(&0x28u32.to_le_bytes());
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&bpp.to_le_bytes());
    v.extend_from_slice(&[0u8; 24]); // BI_RGB, size, ppm x2, colors, important
    v
}

impl Debugger {
    /// `g_sCurrentDir`: the debugger directory with a trailing separator (empty if none).
    fn dir_prefix(&self) -> String {
        let d = self.host.dir();
        if d.is_empty() || d.ends_with('/') || d.ends_with('\\') {
            d
        } else {
            format!("{d}/")
        }
    }

    // ---- BENCHMARK ----

    /// `CpuSetupBenchmark` (CPU.cpp:822): registers and the opcode mix at `$0300`.
    fn setup_benchmark(&mut self, m: &mut Machine) {
        let cpu = m.cpu_mut();
        cpu.a = 0;
        cpu.x = 0;
        cpu.y = 0;
        cpu.pc = 0x0300;
        cpu.sp = 0xFF;
        let mut addr: u32 = 0x300;
        let mut opcode = 0usize;
        let put = |m: &mut Machine, addr: &mut u32, v: u8| {
            m.poke(*addr as u16, v);
            *addr += 1;
        };
        loop {
            put(m, &mut addr, BENCH_OPCODES[opcode]);
            put(m, &mut addr, BENCH_OPCODES[opcode]);
            if opcode >= SHORT_OPCODES {
                put(m, &mut addr, 0);
            }
            opcode += 1;
            if opcode >= BENCH_OPCODES.len() || (addr & 0x0F) >= 0x0B {
                put(m, &mut addr, 0x4C);
                let value = if opcode >= BENCH_OPCODES.len() {
                    0x00
                } else {
                    (((addr >> 4) + 1) << 4) as u8
                };
                put(m, &mut addr, value);
                put(m, &mut addr, 0x03);
                while addr & 0x0F != 0 {
                    addr += 1;
                }
            }
            if opcode >= BENCH_OPCODES.len() {
                break;
            }
        }
    }

    /// `BENCHMARK` / `BENCH` (`CmdBenchmarkStart`): loads the benchmark program, points the
    /// cursor at it and measures how fast this build runs it. AppleWin only prepares the program
    /// (the elapsed time is shown by its Win32 front end); the timed run is the portable part.
    pub(crate) fn cmd_benchmark(&mut self, m: &mut Machine) -> Update {
        self.setup_benchmark(m);
        self.disasm_cur = m.cpu().pc;
        self.calc_top_bot(m);
        self.benchmarking = true;
        self.detach(m);
        let t = Instant::now();
        let cycles = m.run_cycles(BENCH_CYCLES);
        let secs = t.elapsed().as_secs_f64().max(1e-9);
        self.attach(m);
        // leave the program ready to run, as CmdBenchmarkStart does
        self.setup_benchmark(m);
        self.disasm_cur = m.cpu().pc;
        self.calc_top_bot(m);
        let mhz = cycles as f64 / secs / 1e6;
        self.console.print(&format!(
            "{CHC_INFO}Benchmark{CHC_DEFAULT}: {CHC_NUM_DEC}{cycles}{CHC_DEFAULT} cycles in {CHC_NUM_DEC}{secs:.3}{CHC_DEFAULT} s = {CHC_NUM_DEC}{mhz:.2}{CHC_DEFAULT} MHz ({CHC_NUM_DEC}{:.1}{CHC_DEFAULT}x real speed)",
            mhz * 1e6 / NTSC_CLOCK_HZ
        ));
        upd::ALL
    }

    // ---- DISK ----

    pub(crate) fn cmd_disk(&mut self, m: &mut Machine, a: &Args) -> Update {
        let n = a.n();
        if n == 0 {
            return self.help_last();
        }
        let first = a.text(1).to_ascii_uppercase();
        if first == "SLOT" {
            if n > 2 {
                return self.help_last();
            }
            if n > 1 {
                let slot = a.value(2);
                if !(1..=7).contains(&slot) {
                    return self.help_last();
                }
                self.disk_slot = slot as u8;
            }
            let s = self.disk_slot;
            self.console.print(&format!("Current Disk II slot = {s}"));
            return upd::CONSOLE_DISPLAY;
        }
        let slot = self.disk_slot;
        let dir = self.host.dir();
        let Some(card) = m.card_mut::<Disk2Card>(slot) else {
            self.console
                .error(&format!("No Disk II card in slot-{slot}"));
            return upd::CONSOLE_DISPLAY;
        };

        if first == "INFO" {
            if n > 1 {
                return self.help_last();
            }
            let state = card.current_state();
            let shift = card.current_shift_reg();
            let (name, tail) = match state {
                DiskStatus::Off => (
                    "Off",
                    format!("{CHC_INFO}Off{CHC_DEFAULT}    {CHC_NUM_HEX}    "),
                ),
                DiskStatus::Read => (
                    "R",
                    format!("{CHC_COMMAND}R{CHC_DEFAULT} << {CHC_NUM_HEX}{shift:02X}"),
                ),
                DiskStatus::Write => (
                    "W",
                    format!("{CHC_ERROR}W{CHC_DEFAULT} >> {CHC_NUM_HEX}{shift:02X}"),
                ),
                DiskStatus::Prot => (
                    "WP",
                    format!("{CHC_WARNING}WP{CHC_DEFAULT} >| {CHC_NUM_HEX}{shift:02X}"),
                ),
                DiskStatus::Empty => (
                    "n/a",
                    format!("{CHC_INFO}n/a{CHC_DEFAULT}     {CHC_NUM_HEX}    "),
                ),
                DiskStatus::Spin => (
                    "Spin",
                    format!("{CHC_INFO}Spin{CHC_DEFAULT}     {CHC_NUM_HEX}    "),
                ),
            };
            let _ = name;
            let line = format!(
                "FW{CHC_NUM_DEC}{:2}{CHC_ARG_SEP}:{CHC_DEFAULT} D{CHC_NUM_DEC}{}{CHC_DEFAULT} T${CHC_NUM_HEX}{}{CHC_ARG_SEP},{CHC_DEFAULT} Phase ${CHC_NUM_HEX}{}{CHC_ARG_SEP},{CHC_DEFAULT} bitOffset ${CHC_ADDRESS}{:04X}{CHC_ARG_SEP},{CHC_DEFAULT} Cycles {CHC_NUM_DEC}{:.2}{CHC_ARG_SEP}, {tail}",
                card.current_firmware(),
                card.current_drive() + 1,
                card.track_string(card.current_drive()),
                card.current_phase_string(),
                card.current_bit_offset(),
                card.current_extra_cycles(),
            );
            self.console.print(&line);
            return upd::CONSOLE_DISPLAY;
        }

        if n < 2 {
            return self.help_last();
        }
        let drive = a.value(1) as usize;
        if !(1..=2).contains(&drive) {
            return self.help_last();
        }
        let drive = drive - 1;
        let sub = a.text(2).to_ascii_uppercase();
        if !["INFO", "SLOT", "EJECT", "PROTECT", "READ"].contains(&sub.as_str()) {
            return self.help_last();
        }
        match sub.as_str() {
            "EJECT" => {
                if n > 2 {
                    return self.help_last();
                }
                card.eject_disk(drive);
            }
            "PROTECT" => {
                if n > 3 {
                    return self.help_last();
                }
                let protect = n < 3 || a.value(3) != 0;
                card.set_write_protect(drive, protect);
            }
            _ => {
                if n != 3 {
                    return self.help_last();
                }
                let name = a.text(3).to_string();
                let path = if dir.is_empty() || Path::new(&name).is_absolute() {
                    Path::new(&name).to_path_buf()
                } else {
                    Path::new(&dir).join(&name)
                };
                // IMAGE_FORCE_WRITE_PROTECTED, IMAGE_DONT_CREATE
                if let Err(e) = card.insert_path(drive, &path, true) {
                    self.console.error(&format!("Couldn't mount {name}: {e}"));
                }
            }
        }
        upd::CONSOLE_DISPLAY
    }

    // ---- FONT / HCOLOR ----

    /// `CmdConfigFont` with `USE_APPLE_FONT`: the debugger draws with the Apple II font, so no
    /// GDI font can be chosen; names are empty and the cell is 7x8, as in AppleWin.
    pub(crate) fn cmd_config_font(&mut self, a: &Args) -> Update {
        let n = a.n();
        if n == 0 {
            for _ in 0..NUM_FONTS {
                self.console.print(&format!(
                    "  Font: {:<20}  A:{:2}  M:{:2}",
                    "", FONT_WIDTH, FONT_WIDTH
                ));
            }
            return upd::CONSOLE_DISPLAY;
        }
        if n > 2 {
            return self.help_for(Cmd::ConfigFont);
        }
        let first = a.text(1).to_ascii_uppercase();
        if first == "*" || first == "?" {
            let line_px = match self.font_spacing {
                0 => FONT_HEIGHT + 1,
                1 => FONT_HEIGHT,
                _ => FONT_HEIGHT - 1,
            };
            self.console.print(&format!(
                "Lines: {}  Font Px: {}  Line Px: {}",
                self.window_height(),
                FONT_HEIGHT,
                line_px
            ));
            return upd::CONSOLE_DISPLAY;
        }
        match first.as_str() {
            // CmdConfigFontLoad / CmdConfigFontSave are empty in AppleWin.
            "LOAD" | "SAVE" => upd::CONSOLE_DISPLAY,
            "MODE" => {
                if n != 2 {
                    return self.help_for(Cmd::ConfigFont);
                }
                let mode = a.value(2);
                if mode >= NUM_FONT_SPACING {
                    return self.help_for(Cmd::ConfigFont);
                }
                self.font_spacing = mode as u8;
                upd::CONSOLE_DISPLAY | upd::DISASM
            }
            // CmdConfigSetFont is compiled out (OLD_FONT) and just redraws.
            _ => upd::ALL,
        }
    }

    /// `CmdConfigHColor`: validates its arguments and does nothing else, like the C++.
    pub(crate) fn cmd_config_hcolor(&mut self, a: &Args) -> Update {
        let n = a.n();
        if n != 1 && n != 4 {
            return self.help_for(Cmd::ConfigHcolor);
        }
        if a.value(1) as usize >= DebugColor::COUNT {
            return self.help_for(Cmd::ConfigHcolor);
        }
        if n == 1 {
            upd::CONSOLE_DISPLAY
        } else {
            upd::ALL
        }
    }

    // ---- NTSC ----

    fn push_filename(&mut self, prefix: &str, path: &str) {
        if prefix.len() + path.len() >= CONSOLE_WIDTH {
            self.console.print(prefix);
            let cut: String = path.chars().take(CONSOLE_WIDTH).collect();
            self.console.print(&cut);
        } else {
            self.console.print(&format!("{prefix}: {path}"));
        }
    }

    /// The hue table `NTSC_VideoGetChromaTable( false, bColorTV )` returns, as BGRA bytes.
    fn chroma_bytes(&self) -> Vec<u8> {
        let c = apple2_video::chroma::chroma();
        let tables = if self.ntsc_color_tv {
            &c.hue_tv
        } else {
            &c.hue_monitor
        };
        let mut out = Vec::with_capacity(CHROMA_SIZE);
        for phase in tables.iter() {
            for px in phase {
                out.extend_from_slice(&px.to_le_bytes());
            }
        }
        out
    }

    pub(crate) fn cmd_ntsc(&mut self, a: &Args) -> Update {
        let n = a.n();
        let first = a.text(1).to_ascii_uppercase();
        let found = n > 0 && GENERAL_PARAMS.contains(&first.as_str());
        let file = if n > 1 { a.text(2) } else { "" };
        let lower = file.to_ascii_lowercase();
        let is_bmp = lower.len() > 1 && lower[1..].ends_with(".bmp");
        let name = if file.is_empty() {
            DEFAULT_PALETTE_FILE.to_string()
        } else {
            file.to_string()
        };
        let path = format!("{}{}", self.dir_prefix(), name);
        if !found {
            return upd::CONSOLE_DISPLAY;
        }
        match first.as_str() {
            "RESET" => {
                // NTSC_VideoInitChroma() regenerates the default tables; ours are never altered.
                self.console.print(" Resetting NTSC palette.");
            }
            "SAVE" => {
                let src = self.chroma_bytes();
                let data = if is_bmp {
                    // Transpose4096x4::transposeTo64x256, then the 32-bit header.
                    let mut img = vec![0u8; CHROMA_SIZE];
                    for phase in 0..4 {
                        for x in 0..256 {
                            let s = (phase * 4096 + x * 16) * 4;
                            let d = (x * 64 + phase * 16) * 4;
                            img[d..d + 64].copy_from_slice(&src[s..s + 64]);
                        }
                    }
                    let mut v = bmp_header(64, 256, 32);
                    v.extend_from_slice(&img);
                    v
                } else {
                    // RGBAswapBGRA: raw files are RGBA with opaque alpha.
                    let mut v = Vec::with_capacity(CHROMA_SIZE);
                    for px in src.chunks_exact(4) {
                        v.extend_from_slice(&[px[2], px[1], px[0], 255]);
                    }
                    v
                };
                match self.host.write(&name, &data) {
                    Ok(()) => self.push_filename("Saved", &path),
                    Err(_) => {
                        self.push_filename("File", &path);
                        self.console.print("Error couldn't open file for writing.");
                    }
                }
            }
            "LOAD" => {
                let status = match self.host.read(&name) {
                    Err(_) => {
                        self.console.print("Error couldn't open file for reading.");
                        "File: ".to_string()
                    }
                    Ok(bytes) => Self::check_palette_file(&bytes, is_bmp),
                };
                self.push_filename(&status, &path);
            }
            _ => return self.help_last(),
        }
        upd::CONSOLE_DISPLAY
    }

    /// The checks `NTSC LOAD` makes before it copies the table in. The port's renderer builds
    /// its tables once (`apple2_video::chroma`) and has no hook to replace them, so a valid file
    /// is reported as such but not applied.
    fn check_palette_file(bytes: &[u8], is_bmp: bool) -> String {
        if !is_bmp {
            if bytes.len() != CHROMA_SIZE {
                return format!("Raw size != {}", 64 * 256 * 4);
            }
            return "Valid palette, not applied (the port's NTSC table is fixed)".into();
        }
        let le32 = |o: usize| -> Option<u32> {
            bytes
                .get(o..o + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (Some(off), Some(w), Some(h)) = (le32(0x0A), le32(0x12), le32(0x16)) else {
            return "Bitmap not 32-bit RGBA".into();
        };
        let bpp = bytes
            .get(0x1C..0x1E)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .unwrap_or(0);
        if bpp != 32 {
            return "Bitmap not 32-bit RGBA".into();
        }
        if off as usize > bytes.len() {
            return "Bad BITMAP: Data > file size !?".into();
        }
        if !matches!((w, h), (64, 256) | (64, 1) | (16, 1)) {
            return "Bitmap not 64x256, 64x1, or 16x1".into();
        }
        "Valid palette, not applied (the port's NTSC table is fixed)".into()
    }

    // ---- SOURCE / SYNC ----

    /// `BufferAssemblyListing`: reads the listing and keeps its non-empty lines.
    fn buffer_assembly_listing(&mut self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        self.source.lines.clear();
        self.source.by_addr.clear();
        let Ok(bytes) = self.host.read(name) else {
            return false;
        };
        let text = String::from_utf8_lossy(&bytes);
        self.source.lines = text
            .split(['\r', '\n'])
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if self.source.lines.is_empty() {
            return false;
        }
        self.source.active = true;
        true
    }

    /// `ParseAssemblyListing`: `xxxx: b1 [b2] [b3] ...` lines give addresses and bytes;
    /// `label EQU $addr` / `addr: label DFB ...` lines give user symbols.
    fn parse_assembly_listing(&mut self, m: &mut Machine) {
        self.source.bytes = 0;
        self.source.symbols = 0;
        let (to_mem, add_sym) = (self.source.add_memory, self.source.add_symbols);
        let lines = self.source.lines.clone();
        for (i_line, text) in lines.iter().enumerate() {
            let b = text.as_bytes();
            if let Some(colon) = text.find(':') {
                // sscanf( "%X" ): skips white space, reads hex digits
                let head = text[..colon].trim_start();
                let digits: String = head.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                let Ok(addr) = u32::from_str_radix(&digits, 16) else {
                    continue;
                };
                if addr > 0xFFFF {
                    continue;
                }
                if to_mem {
                    let mut end = colon + 1;
                    let mut i_byte = 0u16;
                    while i_byte < 4 {
                        let start = end + 1;
                        let mut stop = start.min(b.len());
                        while stop < b.len() && b[stop] != b' ' && b[stop] != b'\t' {
                            stop += 1;
                        }
                        end = stop;
                        if stop - start.min(stop) != 2 {
                            break;
                        }
                        let s = &text[start..stop];
                        if let Ok(v) = u8::from_str_radix(s, 16) {
                            m.poke((addr as u16).wrapping_add(i_byte), v);
                        }
                        i_byte += 1;
                    }
                    self.source.bytes += i_byte as usize;
                }
                self.source.by_addr.insert(addr as u16, i_line);
            }
            if add_sym {
                // EQU / DFB: the label is the word before the keyword
                let p_equ = text.find("EQU");
                let p_dfb = text.find("DFB");
                let Some(p_label) = p_dfb.or(p_equ) else {
                    continue;
                };
                if p_label == 0 {
                    continue;
                }
                let mut end = p_label - 1;
                while end > 0 && (b[end] == b' ' || b[end] == b'\t') {
                    end -= 1;
                }
                let mut start = end;
                while start > 0 && b[start] != b' ' && b[start] != b'\t' {
                    start -= 1;
                }
                let end = end + 1;
                let start = start + 1;
                let len = end
                    .saturating_sub(start)
                    .min(crate::symbols::MAX_SYMBOLS_LEN);
                if len == 0 {
                    continue;
                }
                // AppleWin copies `len` bytes then writes the terminator at [len - 1], so the
                // last character of the label is lost ("LABEL" is stored as "LABE").
                let name: String = text[start..start + len].chars().take(len - 1).collect();
                let addr_str = if let Some(c) = text.find(':') {
                    Some(&text[..c])
                } else {
                    text[p_label..].find('$').map(|d| &text[p_label + d + 1..])
                };
                if let Some(s) = addr_str {
                    let s = s.trim_start();
                    let digits: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    let v = u32::from_str_radix(&digits, 16).unwrap_or(0);
                    self.symbols.set(Table::Src2, &name, v as u16);
                    self.source.symbols += 1;
                }
            }
        }
    }

    pub(crate) fn cmd_source(&mut self, m: &mut Machine, a: &Args) -> Update {
        let n = a.n();
        if n == 0 {
            self.source.active = false;
            return upd::CONSOLE_DISPLAY;
        }
        self.source.add_memory = false;
        self.source.add_symbols = false;
        for i in 1..=n {
            let arg = a.text(i).to_string();
            match arg.to_ascii_uppercase().as_str() {
                "SYM" | "SYMBOLS" => self.source.add_symbols = true,
                "MEM" | "MEMORY" => self.source.add_memory = true,
                _ => {
                    let full = format!("{}{}", self.dir_prefix(), arg);
                    let mini: String = full.chars().take(20).collect();
                    if self.buffer_assembly_listing(&arg) {
                        self.source.file_name = arg;
                        self.parse_assembly_listing(m);
                        if self.source.bytes > 0 {
                            let (l, s, b) = (
                                self.source.lines.len(),
                                self.source.symbols,
                                self.source.bytes,
                            );
                            self.console
                                .print(&format!("  Read: {l} lines, {s} symbols, {b} bytes"));
                        } else {
                            let (l, s) = (self.source.lines.len(), self.source.symbols);
                            self.console
                                .print(&format!("  Read: {l} lines, {s} symbols"));
                        }
                    } else {
                        self.console.print(&format!("Error reading: {mini}"));
                    }
                }
            }
        }
        upd::CONSOLE_DISPLAY
    }

    /// `CmdSync` is a `// TODO` that only redraws the console in AppleWin.
    pub(crate) fn cmd_sync(&mut self) -> Update {
        upd::CONSOLE_DISPLAY
    }
}
