//! The debugger state machine (Debug.cpp): modes, stepping, stop reasons, breakpoint checks, trace
//! output and the disassembly window arithmetic. Commands live in `cmd_*.rs`.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use apple2_core::{AccessKind, AccessObserver, Machine};

use crate::assembler::Assembler;
use crate::bp::*;
use crate::color::ColorTable;
use crate::console::*;
use crate::disasm::*;
use crate::host::{Host, NoHost};
use crate::mem::{regs_of, Regs};
use crate::opcodes::*;
use crate::symbols::{Symbols, Table};

/// `Update_e`: which parts of the display a command invalidated.
pub type Update = u32;
pub mod upd {
    use super::Update;
    pub const NOTHING: Update = 0;
    pub const BACKGROUND: Update = 1 << 0;
    pub const BREAKPOINTS: Update = 1 << 1;
    pub const CONSOLE_DISPLAY: Update = 1 << 2;
    pub const CONSOLE_INPUT: Update = 1 << 3;
    pub const DISASM: Update = 1 << 4;
    pub const FLAGS: Update = 1 << 5;
    pub const MEM_DUMP: Update = 1 << 6;
    pub const REGS: Update = 1 << 7;
    pub const STACK: Update = 1 << 8;
    pub const SYMBOLS: Update = 1 << 9;
    pub const TARGETS: Update = 1 << 10;
    pub const WATCH: Update = 1 << 11;
    pub const ZERO_PAGE: Update = 1 << 12;
    pub const SOFTSWITCHES: Update = 1 << 13;
    pub const VIDEOSCANNER: Update = 1 << 14;
    pub const ALL: Update = (1 << 15) - 1;
}

/// `g_nAppMode` (debugger side): the frontend runs the machine itself only in `Running`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    /// Emulation runs freely (no breakpoints active).
    Running,
    /// Stopped, waiting for commands.
    Debug,
    /// Single-stepping under the debugger's control: call [`Debugger::run`].
    Stepping,
}

/// `BreakpointHit_t`
pub mod hit {
    pub const NONE: u32 = 0;
    pub const INVALID: u32 = 1 << 0;
    pub const OPCODE: u32 = 1 << 1;
    pub const REG: u32 = 1 << 2;
    pub const MEM: u32 = 1 << 3;
    pub const MEMR: u32 = 1 << 4;
    pub const MEMW: u32 = 1 << 5;
    pub const PC_FLOATING_OR_IO: u32 = 1 << 6;
    pub const INTERRUPT: u32 = 1 << 7;
    pub const VIDEO_POS: u32 = 1 << 8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemView {
    Hex,
    Ascii,
    Apple,
}

/// `MemoryDump_t` (the two mini memory areas MD1/MD2).
#[derive(Clone, Copy, Debug)]
pub struct MemDump {
    pub addr: u16,
    pub view: MemView,
    pub active: bool,
    pub prefix: Prefix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Code,
    Data,
    Console,
    Source,
    Symbols,
    ZeroPage,
}

/// What `TEXT`/`GR`/`HGR`... ask the frontend to show instead of the debugger (`\` returns).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputView {
    Text40(Option<u8>),
    Text80(Option<u8>),
    Gr(Option<u8>),
    Dgr(Option<u8>),
    Hgr(Option<u8>),
    Dhgr(Option<u8>),
    Shr,
    /// Pseudo hi-res page 0-8 (`HGR0`..`HGR8`).
    HgrPseudo(u8),
    /// Current video mode (`\`).
    Current,
}

/// Profiling counters (`g_aProfileOpcodes`, `g_aProfileOpmodes`).
#[derive(Clone, Debug)]
pub struct Profile {
    pub opcodes: [u64; 256],
    pub modes: [u64; 14],
    pub begin_cycles: u64,
    pub active: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { opcodes: [0; 256], modes: [0; 14], begin_cycles: 0, active: false }
    }
}

pub const ALL_MODES: [Mode; 14] = [
    Mode::Implied,
    Mode::Imm,
    Mode::Abs,
    Mode::Zp,
    Mode::AbsX,
    Mode::AbsY,
    Mode::ZpX,
    Mode::ZpY,
    Mode::Rel,
    Mode::IndZpX,
    Mode::IndAbsX,
    Mode::IndZpY,
    Mode::IndZp,
    Mode::IndAbs,
];


pub fn mode_index(m: Mode) -> usize {
    ALL_MODES.iter().position(|x| *x == m).unwrap_or(0)
}

/// Shared state between the debugger and the bus observer installed in the machine.
#[derive(Default)]
struct ObsShared {
    /// bit 0: watch reads, bit 1: watch writes, per address.
    watch: RefCell<Vec<u8>>,
    log: RefCell<Vec<(u16, u8, AccessKind)>>,
}

struct Obs(Rc<ObsShared>);

impl AccessObserver for Obs {
    fn access(&mut self, addr: u16, value: u8, kind: AccessKind) {
        let bit = if kind == AccessKind::Read { 1 } else { 2 };
        let w = self.0.watch.borrow();
        if w.get(addr as usize).copied().unwrap_or(0) & bit != 0 {
            let mut l = self.0.log.borrow_mut();
            if l.len() < 4096 {
                l.push((addr, value, kind));
            }
        }
    }
}

pub struct Debugger {
    pub symbols: Symbols,
    pub data: DataBlocks,
    pub colors: ColorTable,
    pub cfg: DisasmConfig,
    pub console: Console,
    pub mode: RunMode,
    pub breakpoints: BpTable,
    pub watches: BpTable,
    pub zero_page: BpTable,
    pub bookmarks: [Option<u16>; MAX_BOOKMARKS],
    pub asm: Assembler,
    pub asm_active: bool,
    pub disasm_top: u16,
    pub disasm_cur: u16,
    pub disasm_cur_line: usize,
    pub disasm_height: usize,
    pub disasm_cur_bad: bool,
    pub mem_dump: [MemDump; 2],
    pub data_addr: u16,
    pub window: Window,
    pub last_window: Window,
    pub info_visible: bool,
    /// `CODE2`/`DATA2`: code and data windows share the screen.
    pub split: bool,
    pub search_results: Vec<u16>,
    pub steps: i32,
    pub step_until: Option<u16>,
    pub skip: Option<(u16, u16)>,
    /// `GG` vs `G`: tells the frontend whether to throttle to real speed.
    pub full_speed: bool,
    pub last_go_full_speed: bool,
    /// Bit n: break on BRK (0) / invalid opcode of 1, 2, 3 bytes.
    pub break_invalid: u8,
    pub break_opcode: Option<u8>,
    pub break_on_interrupt: bool,
    pub(crate) trace: Option<Box<dyn Write>>,
    pub(crate) trace_header: bool,
    pub(crate) trace_scanner: bool,
    pub profile: Profile,
    pub stop_reason: Option<String>,
    pub host: Box<dyn Host>,
    pub lbr: Option<u16>,
    pub output_view: Option<OutputView>,
    pub video_decimal: bool,
    pub video_horz_real: bool,
    pub cycle_mode: u8,
    pub(crate) cycle_mark: u64,
    pub(crate) cycle_delta: u64,
    pub echo_scripts: bool,
    pub info_target_pointer: bool,
    pub(crate) last_cmd: Option<crate::cmdtable::Cmd>,
    shared: Rc<ObsShared>,
    attached: bool,
    force_single_step_next: bool,
    hit_id: Option<usize>,
    pre_hit: Vec<usize>,
    pub(crate) script_depth: u32,
    pub(crate) break_mem_addr: u16,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    pub fn new() -> Self {
        let md = MemDump { addr: 0, view: MemView::Hex, active: false, prefix: Prefix::default() };
        Debugger {
            symbols: Symbols::default(),
            data: DataBlocks::default(),
            colors: ColorTable::default(),
            cfg: DisasmConfig::default(),
            console: Console::default(),
            mode: RunMode::Running,
            breakpoints: BpTable::new(MAX_BREAKPOINTS),
            watches: BpTable::new(MAX_WATCHES),
            zero_page: BpTable::new(MAX_ZEROPAGE_POINTERS),
            bookmarks: [None; MAX_BOOKMARKS],
            asm: Assembler::default(),
            asm_active: false,
            disasm_top: 0,
            disasm_cur: 0,
            disasm_cur_line: (crate::display::WINDOW_HEIGHT - 1) / 2,
            disasm_height: crate::display::WINDOW_HEIGHT,
            disasm_cur_bad: false,
            mem_dump: [md, md],
            data_addr: 0,
            window: Window::Code,
            last_window: Window::Code,
            info_visible: true,
            split: false,
            search_results: Vec::new(),
            steps: 0,
            step_until: None,
            skip: None,
            full_speed: false,
            last_go_full_speed: false,
            break_invalid: 0,
            break_opcode: None,
            break_on_interrupt: false,
            trace: None,
            trace_header: false,
            trace_scanner: false,
            profile: Profile::default(),
            stop_reason: None,
            host: Box::new(NoHost),
            lbr: None,
            output_view: None,
            video_decimal: false,
            video_horz_real: false,
            cycle_mode: 0,
            cycle_mark: 0,
            cycle_delta: 0,
            echo_scripts: false,
            info_target_pointer: false,
            last_cmd: None,
            shared: Rc::new(ObsShared::default()),
            attached: false,
            force_single_step_next: false,
            hit_id: None,
            pre_hit: Vec::new(),
            script_depth: 0,
            break_mem_addr: 0,
        }
    }

    pub fn set_host(&mut self, host: Box<dyn Host>) {
        self.host = host;
    }

    /// Redirects `TF` trace output to any writer (the frontend or a test).
    pub fn set_trace_writer(&mut self, w: Option<Box<dyn Write>>) {
        self.trace_header = w.is_some();
        self.trace = w;
    }

    // ---- context helpers ----

    pub fn ctx<'a>(&'a self, m: &'a Machine) -> DisasmCtx<'a> {
        DisasmCtx { mem: m, regs: regs_of(m), cmos: m.cpu().is_cmos(), symbols: &self.symbols, data: &self.data, cfg: &self.cfg }
    }

    pub fn disasm_line(&self, m: &Machine, addr: u16) -> DisasmLine {
        disassemble(&self.ctx(m), addr)
    }

    fn paging(m: &Machine) -> PagingView {
        PagingView { memmode: m.memmode(), aux_bank_plus1: m.board().memory.aux_bank() as u32 + 1 }
    }

    // ---- entering and leaving ----

    /// Installs the bus observer (idempotent) and refreshes the watched-address map.
    pub fn attach(&mut self, m: &mut Machine) {
        if !self.attached {
            m.set_access_observer(Some(Box::new(Obs(self.shared.clone()))));
            self.attached = true;
        }
        self.sync_watch();
    }

    /// Removes the observer from the machine.
    pub fn detach(&mut self, m: &mut Machine) {
        if self.attached {
            m.set_access_observer(None);
            self.attached = false;
        }
    }

    /// Rebuilds the address map the observer filters on from the enabled memory breakpoints.
    pub(crate) fn sync_watch(&mut self) {
        let mut w = self.shared.watch.borrow_mut();
        w.clear();
        w.resize(0x10000, 0);
        for (_, b) in self.breakpoints.iter_set() {
            if !b.valid() || !b.source.is_mem() || b.op != Op::Eq {
                continue;
            }
            let bits = match b.source {
                Source::MemRead => 1,
                Source::MemWrite => 2,
                _ => 3,
            };
            for a in b.address as u32..(b.address as u32 + b.length).min(0x10000) {
                w[a as usize] |= bits;
            }
        }
        self.shared.log.borrow_mut().clear();
    }

    /// `DebugBegin`: called every time the debugger is entered (F7).
    pub fn enter(&mut self, m: &mut Machine) {
        self.attach(m);
        self.mode = RunMode::Debug;
        self.steps = 0;
        self.disasm_cur = m.cpu().pc;
        self.calc_top_bot(m);
        self.lbr = None;
        self.output_view = None;
        self.asm_active = false;
        self.console.set_prompt('>');
    }

    /// `DebugExitDebugger`: with breakpoints or a trace file active the emulation keeps being
    /// single-stepped under [`Debugger::run`], otherwise it runs free.
    pub fn exit(&mut self, m: &mut Machine) {
        self.breakpoints.clear_temp();
        if self.breakpoints.count == 0 && self.trace.is_none() && self.watches_none_hot() {
            self.mode = RunMode::Running;
            self.steps = 0;
            if let Some(mut t) = self.trace.take() {
                let _ = t.flush();
            }
            self.detach(m);
            return;
        }
        let full = self.last_go_full_speed;
        self.begin_go(m.cpu().pc, None, None, full);
    }

    fn watches_none_hot(&self) -> bool {
        true
    }

    /// True if the frontend must call [`Debugger::run`] instead of running the machine itself.
    pub fn is_stepping(&self) -> bool {
        self.mode == RunMode::Stepping
    }

    /// `CmdGo`: free-run until `until` (or a breakpoint), skipping `skip` (start, length).
    pub(crate) fn begin_go(&mut self, _pc: u16, until: Option<u16>, skip: Option<(u16, u16)>, full_speed: bool) {
        self.steps = -1;
        self.step_until = until;
        self.skip = skip;
        self.full_speed = full_speed;
        self.last_go_full_speed = full_speed;
        self.enter_stepping();
    }

    /// `DebugEnterStepping`
    pub(crate) fn enter_stepping(&mut self) {
        self.breakpoints.clear_temp();
        self.mode = RunMode::Stepping;
    }

    // ---- disassembly window arithmetic ----

    /// `_6502_GetOpmodeOpbyte` length only.
    pub fn instr_len(&self, m: &Machine, addr: u16) -> usize {
        opmode_opbyte(&self.ctx(m), addr).2
    }

    /// `DisasmCalcAddressFromLines`
    pub fn addr_after_lines(&self, m: &Machine, mut a: u16, lines: usize) -> u16 {
        for _ in 0..lines {
            a = a.wrapping_add(self.instr_len(m, a) as u16);
        }
        a
    }

    /// `DisasmCalcTopFromCurAddress`: finds a top such that `cur` lands on `cur_line`.
    pub fn calc_top_from_cur(&mut self, m: &Machine) {
        let n = self.disasm_cur_line * 3;
        self.disasm_cur_bad = false;
        let cur = self.disasm_cur;
        let mut top = cur.wrapping_sub(n as u16);
        let mut found = false;
        for _ in 0..=n {
            let mut a = top;
            for line in 0..=n {
                if line == self.disasm_cur_line {
                    if a == cur {
                        self.disasm_top = top;
                        found = true;
                    }
                    break;
                }
                a = a.wrapping_add(self.instr_len(m, a) as u16);
            }
            if found {
                break;
            }
            top = top.wrapping_add(1);
        }
        if !found {
            self.disasm_top = cur;
            self.disasm_cur_bad = true;
        }
    }

    /// `DisasmCalcTopBotAddress`
    pub fn calc_top_bot(&mut self, m: &Machine) {
        self.calc_top_from_cur(m);
    }

    /// `DisasmCalcCurFromTopAddress`
    pub fn calc_cur_from_top(&mut self, m: &Machine) {
        self.disasm_cur = self.addr_after_lines(m, self.disasm_top, self.disasm_cur_line);
    }

    pub fn disasm_bottom(&self, m: &Machine) -> u16 {
        self.addr_after_lines(m, self.disasm_top, self.disasm_height)
    }

    /// Moves the cursor to `addr` and recomputes the window (`g_nDisasmCurAddress = ..`).
    pub fn goto(&mut self, m: &Machine, addr: u16) {
        self.disasm_cur = addr;
        self.calc_top_bot(m);
    }

    // ---- breakpoint checking ----

    fn do_hit(&mut self, table: u8, i: usize, kind: u32) -> u32 {
        let _ = table;
        let b = &mut self.breakpoints.slots[i];
        if b.stop && self.hit_id.is_none() {
            self.hit_id = Some(i);
        }
        b.hit = true;
        b.hit_count += 1;
        if b.stop {
            kind
        } else {
            hit::NONE
        }
    }

    /// `CheckBreakpointsIO`: memory breakpoints against the targets of the instruction about to run.
    fn check_breakpoints_mem(&mut self, m: &Machine) -> u32 {
        let pg = Self::paging(m);
        let pc = m.cpu().pc;
        let (t1, t2, t3, nbytes) = get_targets(&self.ctx(m), pc, true, false);
        if nbytes == 0 {
            return 0;
        }
        let op = m.peek(pc);
        let info = table(m.cpu().is_cmos())[op as usize];
        let mut res = 0;
        for target in [t1, t2, t3] {
            if target == NO_TARGET {
                continue;
            }
            for i in 0..self.breakpoints.slots.len() {
                let b = self.breakpoints.slots[i].clone();
                if !b.valid() || !b.source.is_mem() || !b.check_value(target as u32, &pg) {
                    continue;
                }
                let kind = match b.source {
                    Source::MemRw => hit::MEM,
                    Source::MemRead if info.mem & (MEM_RI | MEM_R) != 0 => hit::MEMR,
                    Source::MemWrite if info.mem & (MEM_WI | MEM_W) != 0 => hit::MEMW,
                    _ => continue,
                };
                self.break_mem_addr = target as u16;
                self.pre_hit.push(i);
                res |= self.do_hit(0, i, kind);
            }
        }
        res
    }

    /// `CheckBreakpointsReg`
    fn check_breakpoints_reg(&mut self, m: &Machine) -> u32 {
        let pg = Self::paging(m);
        let c = m.cpu();
        let mut res = 0;
        for i in 0..self.breakpoints.slots.len() {
            let b = self.breakpoints.slots[i].clone();
            if !b.valid() {
                continue;
            }
            let v = match b.source {
                Source::RegPc => c.pc as u32,
                Source::RegA => c.a as u32,
                Source::RegX => c.x as u32,
                Source::RegY => c.y as u32,
                Source::RegP => c.p as u32,
                Source::RegS => c.sp as u32,
                _ => continue,
            };
            if b.check_value(v, &pg) {
                res = self.do_hit(0, i, hit::REG);
            }
        }
        res
    }

    /// `CheckBreakpointsVideo`
    fn check_breakpoints_video(&mut self, m: &Machine) -> u32 {
        let pg = Self::paging(m);
        let mut res = 0;
        for i in 0..self.breakpoints.slots.len() {
            let b = self.breakpoints.slots[i].clone();
            if !b.valid() || b.source != Source::Video {
                continue;
            }
            let vert = m.video_position().0;
            if b.check_value(vert, &pg) {
                res = self.do_hit(0, i, hit::VIDEO_POS);
                self.breakpoints.slots[i].enabled = false; // else it fires many times on the line
            }
        }
        res
    }

    /// Accesses seen on the bus during the last instruction that match a memory breakpoint and were
    /// not already reported before the instruction ran.
    fn check_bus_hits(&mut self, m: &Machine, old_pc: u16, fetch_len: usize) -> u32 {
        let log: Vec<_> = std::mem::take(&mut *self.shared.log.borrow_mut());
        if log.is_empty() {
            self.pre_hit.clear();
            return 0;
        }
        let pg = Self::paging(m);
        let mut res = 0;
        let mut skip_fetch = fetch_len;
        for (addr, _v, kind) in log {
            // The first reads are the opcode/operand fetches of the instruction itself.
            if kind == AccessKind::Read && skip_fetch > 0 && addr.wrapping_sub(old_pc) < fetch_len as u16 {
                skip_fetch -= 1;
                continue;
            }
            for i in 0..self.breakpoints.slots.len() {
                let b = self.breakpoints.slots[i].clone();
                if !b.valid() || !b.source.is_mem() || self.pre_hit.contains(&i) {
                    continue;
                }
                let ok = matches!((b.source, kind), (Source::MemRw, _) | (Source::MemRead, AccessKind::Read) | (Source::MemWrite, AccessKind::Write));
                if ok && b.check_value(addr as u32, &pg) {
                    self.break_mem_addr = addr;
                    let k = match b.source {
                        Source::MemRead => hit::MEMR,
                        Source::MemWrite => hit::MEMW,
                        _ => hit::MEM,
                    };
                    res |= self.do_hit(0, i, k);
                }
            }
        }
        self.pre_hit.clear();
        res
    }

    /// `CheckBreakOpcode`
    fn check_break_opcode(&self, op: u8, cmos: bool) -> u32 {
        let mut r = 0;
        if op == OPCODE_BRK && self.break_invalid & 1 != 0 {
            r |= hit::INVALID;
        }
        let info = table(cmos)[op as usize];
        if info.is_extension() {
            let t = match info.mode.bytes() {
                1 => 1,
                2 => 2,
                _ => 3,
            };
            if self.break_invalid & (1 << t) != 0 {
                r |= hit::INVALID;
            }
        }
        if let Some(b) = self.break_opcode {
            if b == op && op != 0 {
                r |= hit::OPCODE;
            }
        }
        r
    }

    // ---- running ----

    /// Executes one instruction with the profile and last-branch bookkeeping; true if an interrupt
    /// was taken instead/as well.
    fn single_step(&mut self, m: &mut Machine) -> bool {
        let pc = m.cpu().pc;
        let op = m.peek(pc);
        let cmos = m.cpu().is_cmos();
        self.update_lbr(m, op);
        let mark = m.cycles();
        m.step();
        let cyc = m.cycles() - mark;
        if self.profile.active {
            let info = table(cmos)[op as usize];
            self.profile.opcodes[op as usize] += 1;
            self.profile.modes[mode_index(info.mode)] += 1;
        }
        let _ = cyc;
        // An interrupt entry lands on a vector without a BRK/JSR/JMP having been executed.
        let newpc = m.cpu().pc;
        let irq_v = m.peek(0xFFFE) as u16 | (m.peek(0xFFFF) as u16) << 8;
        let nmi_v = m.peek(0xFFFA) as u16 | (m.peek(0xFFFB) as u16) << 8;
        let jumpish = matches!(op, 0x00 | 0x20 | 0x4C | 0x6C | 0x7C | 0x40 | 0x60);
        let intr = (newpc == irq_v || newpc == nmi_v) && !jumpish;
        if intr {
            self.lbr = Some(pc);
        }
        intr
    }

    /// `UpdateLBR`
    fn update_lbr(&mut self, m: &Machine, op: u8) {
        let p = m.cpu().p;
        let cmos = m.cpu().is_cmos();
        let mut flow = matches!(op, OPCODE_BRK | OPCODE_JSR | OPCODE_JMP_A | OPCODE_RTI | OPCODE_RTS | OPCODE_JMP_NA) || (cmos && op == OPCODE_JMP_IAX);
        if table(cmos)[op as usize].mode == Mode::Rel {
            let taken = match op {
                0x80 => true,
                0x10 => p & 0x80 == 0,
                0x30 => p & 0x80 != 0,
                0x50 => p & 0x40 == 0,
                0x70 => p & 0x40 != 0,
                0x90 => p & 1 == 0,
                0xB0 => p & 1 != 0,
                0xD0 => p & 2 == 0,
                0xF0 => p & 2 != 0,
                _ => false,
            };
            flow |= taken;
        }
        if flow {
            self.lbr = Some(m.cpu().pc);
        }
    }

    /// One iteration of `DebugContinueStepping` (Debug.cpp:9050). Returns the cycles executed.
    pub fn continue_stepping(&mut self, m: &mut Machine) -> u64 {
        let start = m.cycles();
        if let Some((s, len)) = self.skip {
            let pc = m.cpu().pc;
            if pc >= s && (pc as u32) < s as u32 + len as u32 {
                self.steps = -1;
            } else {
                self.steps = 1;
            }
            self.mode = RunMode::Stepping;
        }
        let mut hits = 0u32;
        if self.steps != 0 {
            let mut do_step = true;
            if self.force_single_step_next {
                self.force_single_step_next = false;
                hits = 0;
            } else {
                if self.trace.is_some() {
                    self.output_trace_line(m);
                }
                let pc = m.cpu().pc;
                if !(0xC000..=0xC0FF).contains(&pc) {
                    hits |= self.check_break_opcode(m.peek(pc), m.cpu().is_cmos());
                } else {
                    hits |= hit::PC_FLOATING_OR_IO;
                }
                // Memory breakpoints are checked against the instruction about to run (AppleWin
                // runs CheckBreakpointsIO right after the previous step, with the new PC).
                self.hit_id = None;
                hits |= self.check_breakpoints_mem(m);
                if hits != 0 {
                    do_step = false;
                    self.force_single_step_next = true;
                }
            }
            if do_step {
                let old_pc = m.cpu().pc;
                let len = self.instr_len(m, old_pc);
                self.shared.log.borrow_mut().clear();
                let intr = self.single_step(m);
                if intr && self.break_on_interrupt {
                    hits |= hit::INTERRUPT;
                }
                self.hit_id = None;
                hits |= self.check_bus_hits(m, old_pc, len);
                hits |= self.check_breakpoints_reg(m) | self.check_breakpoints_video(m);
                // the instruction at the new PC is checked for memory hits on the next iteration
            }
            let pc = m.cpu().pc;
            if Some(pc) == self.step_until || hits != 0 {
                let reason = self.stop_reason_text(m, hits, Some(pc) == self.step_until);
                let id = match self.hit_id {
                    Some(i) => format!("{CHC_DEFAULT}[{CHC_ARG_SEP}B#{CHC_NUM_HEX}{i:X}{CHC_DEFAULT}]"),
                    None => format!("{CHC_DEFAULT}[{CHC_ARG_SEP}B#{CHC_NUM_HEX}-{CHC_DEFAULT}]"),
                };
                self.console.print(&format!("{CHC_INFO}Stop reason: {id} {CHC_DEFAULT}{reason}"));
                self.stop_reason = Some(reason);
                self.hit_id = None;
                self.steps = 0;
            }
            if self.steps > 0 {
                self.steps -= 1;
            }
        }
        if self.steps == 0 {
            self.mode = RunMode::Debug;
            self.disasm_cur = m.cpu().pc;
            self.calc_top_bot(m);
        }
        m.cycles() - start
    }

    pub(crate) fn stop_reason_text(&mut self, m: &Machine, hits: u32, until: bool) -> String {
        let pc = m.cpu().pc;
        let pfx = self.hit_id.map(|i| self.breakpoints.slots[i].prefix.text()).unwrap_or_default();
        if until {
            format!("{CHC_DEFAULT}Register {CHC_REGS}PC{CHC_DEFAULT} matches '{CHC_INFO}Go until{CHC_DEFAULT}' address ${CHC_ADDRESS}{pc:04X}")
        } else if hits & hit::INVALID != 0 {
            "Invalid opcode".into()
        } else if hits & hit::OPCODE != 0 {
            format!("Opcode match at {CHC_ARG_SEP}${CHC_ADDRESS}{pc:04X}")
        } else if hits & hit::REG != 0 {
            match self.hit_id {
                Some(i) => format!("Register {CHC_REGS}{}{CHC_DEFAULT} matches value", self.breakpoints.slots[i].source.name()),
                None => "Register matches value".into(),
            }
        } else if hits & hit::MEM != 0 {
            format!("Memory access at {CHC_INFO}{pfx}{CHC_ADDRESS}{:04X}", self.break_mem_addr)
        } else if hits & hit::MEMW != 0 {
            format!("Write access at {CHC_INFO}{pfx}{CHC_ADDRESS}{:04X}", self.break_mem_addr)
        } else if hits & hit::MEMR != 0 {
            format!("Read access at {CHC_INFO}{pfx}{CHC_ADDRESS}{:04X}", self.break_mem_addr)
        } else if hits & hit::PC_FLOATING_OR_IO != 0 {
            "PC reads from floating bus or I/O memory".into()
        } else if hits & hit::INTERRUPT != 0 {
            match self.lbr {
                Some(a) => format!("Interrupt occurred at {CHC_ARG_SEP}${CHC_ADDRESS}{a:04X}"),
                None => "Interrupt occurred (LBR unknown)".into(),
            }
        } else if hits & hit::VIDEO_POS != 0 {
            format!("Video scanner position matches at vpos=${:04X}", m.video_position().0)
        } else {
            "Unknown!".into()
        }
    }

    /// Runs the machine under debugger control for about `max_cycles` cycles, until a stop condition
    /// ends [`RunMode::Stepping`]. Returns the cycles executed.
    pub fn run(&mut self, m: &mut Machine, max_cycles: u64) -> u64 {
        if self.mode != RunMode::Stepping {
            return 0;
        }
        self.attach(m);
        let start = m.cycles();
        while self.mode == RunMode::Stepping && m.cycles() - start < max_cycles {
            self.continue_stepping(m);
        }
        m.cycles() - start
    }

    /// `DebugStopStepping`: the next [`Debugger::run`] iteration returns to [`RunMode::Debug`].
    pub fn stop_stepping(&mut self) {
        if self.mode == RunMode::Stepping {
            self.steps = 0;
        }
    }

    // ---- trace ----

    /// `FormatDisassemblyLine` (Debugger_Disassembler.cpp:658), used by trace files.
    pub fn format_trace_disasm(&self, line: &DisasmLine) -> String {
        let mut s = format!("{}:{} {} ", line.address, line.opcodes, line.mnemonic);
        if line.target_value_flag || line.target_relative || line.target_immediate {
            if line.target_relative {
                s.push('$');
                s += &line.target_value;
            } else if line.target_immediate {
                s += "#$";
                s += &line.target;
            } else {
                let t = line.n_target;
                s.push('$');
                s += &match line.mode {
                    Some(Mode::Abs) => format!("{t:04X}"),
                    Some(Mode::Zp) => format!("{:02X}", t as u8),
                    Some(Mode::AbsX) => format!("{t:04X},X"),
                    Some(Mode::AbsY) => format!("{t:04X},Y"),
                    Some(Mode::ZpX) => format!("{:02X},X", t as u8),
                    Some(Mode::ZpY) => format!("{:02X},Y", t as u8),
                    Some(Mode::IndZpX) => format!("({:02X},X)", t as u8),
                    Some(Mode::IndAbsX) => format!("({t:04X},X)"),
                    Some(Mode::IndZpY) => format!("({:02X}),Y", t as u8),
                    Some(Mode::IndZp) => format!("({:02X})", t as u8),
                    Some(Mode::IndAbs) => format!("({t:04X})"),
                    _ => String::new(),
                };
            }
        }
        s
    }

    /// `OutputTraceLine` (Debug.cpp:8466).
    fn output_trace_line(&mut self, m: &Machine) {
        let line = self.disasm_line(m, m.cpu().pc);
        let dis = self.format_trace_disasm(&line);
        let r = regs_of(m);
        let flags: String = (0..8)
            .rev()
            .map(|bit| if r.p & (1 << bit) != 0 { ['C', 'Z', 'I', 'D', 'B', 'R', 'V', 'N'][bit] } else { '.' })
            .collect();
        let header = std::mem::take(&mut self.trace_header);
        let text = if self.trace_scanner {
            let (v, h) = m.video_position();
            let addr = 0u16;
            let hdr = if header { "Vert Horz Addr Data A: X: Y: SP:  Flags     Addr:Opcode    Mnemonic\n" } else { "" };
            format!("{hdr}{v:04X} {h:04X} {addr:04X}   {:02X} {:02X} {:02X} {:02X} {:04X} {flags}  {dis}\n", 0, r.a, r.x, r.y, 0x100 | r.sp as u16)
        } else {
            let hdr = if header { "Cycles   A: X: Y: SP:  Flags     Addr:Opcode    Mnemonic\n" } else { "" };
            format!("{hdr}{:08X} {:02X} {:02X} {:02X} {:04X} {flags}  {dis}\n", m.cycles() as u32, r.a, r.x, r.y, 0x100 | r.sp as u16)
        };
        if let Some(w) = self.trace.as_mut() {
            let _ = w.write_all(text.as_bytes());
        }
    }

    // ---- misc state helpers used by the commands ----

    pub fn regs(&self, m: &Machine) -> Regs {
        regs_of(m)
    }

    /// `_6502_GetStackReturnAddress`: the address a RTS would return to.
    pub fn stack_return_address(m: &Machine) -> u16 {
        let sp = m.cpu().sp as u16 + 1;
        let lo = m.peek(0x100 | (sp & 0xFF)) as u16;
        let hi = m.peek(0x100 | ((sp + 1) & 0xFF)) as u16;
        (hi << 8 | lo).wrapping_add(1)
    }

    pub fn is_symbol_table_loaded(&self, t: Table) -> bool {
        self.symbols.len(t) > 0
    }
}
