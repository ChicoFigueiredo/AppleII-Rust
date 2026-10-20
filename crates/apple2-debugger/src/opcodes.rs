//! Opcode tables of the AppleWin debugger: `g_aOpcodes65C02` / `g_aOpcodes6502`
//! (Debugger_Assembler.cpp:95-290), the addressing modes (`g_aOpmodes`, :44) and the
//! memory-access flags (Debugger_Types.h:1116). Lower-case mnemonics are the 65C02-only (in the
//! 6502 table: undocumented) opcodes, exactly as AppleWin prints them.

/// `AddressingMode_e` (Debugger_Types.h:96), restricted to the modes the tables use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Implied,
    /// `#$nn`
    Imm,
    Abs,
    Zp,
    AbsX,
    AbsY,
    ZpX,
    ZpY,
    Rel,
    /// `($nn,X)`
    IndZpX,
    /// `($nnnn,X)`
    IndAbsX,
    /// `($nn),Y`
    IndZpY,
    /// `($nn)`
    IndZp,
    /// `($nnnn)`
    IndAbs,
}

impl Mode {
    /// `g_aOpmodes[].m_nBytes`.
    pub fn bytes(self) -> u8 {
        match self {
            Mode::Implied => 1,
            Mode::Imm | Mode::Zp | Mode::ZpX | Mode::ZpY | Mode::Rel | Mode::IndZpX | Mode::IndZpY | Mode::IndZp => 2,
            _ => 3,
        }
    }

    /// `g_aOpmodes[].m_sName`.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Implied => "(implied)",
            Mode::Imm => "Immediate",
            Mode::Abs => "Absolute",
            Mode::Zp => "Zero Page",
            Mode::AbsX => "Absolute,X",
            Mode::AbsY => "Absolute,Y",
            Mode::ZpX => "Zero Page,X",
            Mode::ZpY => "Zero Page,Y",
            Mode::Rel => "Relative",
            Mode::IndZpX => "(Zero Page),X",
            Mode::IndAbsX => "(Absolute),X",
            Mode::IndZpY => "(Zero Page),Y",
            Mode::IndZp => "(Zero Page)",
            Mode::IndAbs => "(Absolute)",
        }
    }
}

pub const MEM_R: u8 = 1;
pub const MEM_W: u8 = 2;
/// Read implicit (stack pull).
pub const MEM_RI: u8 = 4;
/// Write implicit (stack push).
pub const MEM_WI: u8 = 8;
/// Stack read/write.
pub const MEM_S: u8 = 16;
/// Immediate.
pub const MEM_IM: u8 = 32;

#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub mnemonic: &'static str,
    pub mode: Mode,
    pub mem: u8,
}

impl OpInfo {
    pub const fn new(mnemonic: &'static str, mode: Mode, mem: u8) -> Self {
        OpInfo { mnemonic, mode, mem }
    }

    /// Lower-case mnemonic: 65C02-only (or, in the NMOS table, undocumented/illegal) opcode.
    pub fn is_extension(&self) -> bool {
        self.mnemonic.as_bytes()[0].is_ascii_lowercase()
    }
}

/// The table in use for a CPU (`g_aOpcodes`, selected by `DebugInitialize`).
pub fn table(cmos: bool) -> &'static [OpInfo; 256] {
    if cmos {
        &OPCODES_65C02
    } else {
        &OPCODES_6502
    }
}

pub const OPCODE_BRK: u8 = 0x00;
pub const OPCODE_JSR: u8 = 0x20;
pub const OPCODE_RTI: u8 = 0x40;
pub const OPCODE_JMP_A: u8 = 0x4C;
pub const OPCODE_RTS: u8 = 0x60;
pub const OPCODE_JMP_NA: u8 = 0x6C;
pub const OPCODE_JMP_IAX: u8 = 0x7C;
pub const OPCODE_NOP: u8 = 0xEA;

/// Branch opcodes (`_6502_IsOpcodeBranch`): all `Rel` mode opcodes.
pub fn is_branch(op: u8, cmos: bool) -> bool {
    table(cmos)[op as usize].mode == Mode::Rel
}

pub static OPCODES_65C02: [OpInfo; 256] = [
    OpInfo::new("BRK", Mode::Implied, 24), OpInfo::new("ORA", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TSB", Mode::Zp, 2), OpInfo::new("ORA", Mode::Zp, 1), OpInfo::new("ASL", Mode::Zp, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("PHP", Mode::Implied, 24), OpInfo::new("ORA", Mode::Imm, 32), OpInfo::new("ASL", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TSB", Mode::Abs, 2), OpInfo::new("ORA", Mode::Abs, 1), OpInfo::new("ASL", Mode::Abs, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BPL", Mode::Rel, 0), OpInfo::new("ORA", Mode::IndZpY, 1), OpInfo::new("ORA", Mode::IndZp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TRB", Mode::Zp, 2), OpInfo::new("ORA", Mode::ZpX, 1), OpInfo::new("ASL", Mode::ZpX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CLC", Mode::Implied, 0), OpInfo::new("ORA", Mode::AbsY, 1), OpInfo::new("INC", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TRB", Mode::Abs, 2), OpInfo::new("ORA", Mode::AbsX, 1), OpInfo::new("ASL", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("JSR", Mode::Abs, 24), OpInfo::new("AND", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BIT", Mode::Zp, 1), OpInfo::new("AND", Mode::Zp, 1), OpInfo::new("ROL", Mode::Zp, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("PLP", Mode::Implied, 20), OpInfo::new("AND", Mode::Imm, 32), OpInfo::new("ROL", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BIT", Mode::Abs, 1), OpInfo::new("AND", Mode::Abs, 1), OpInfo::new("ROL", Mode::Abs, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BMI", Mode::Rel, 0), OpInfo::new("AND", Mode::IndZpY, 1), OpInfo::new("AND", Mode::IndZp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BIT", Mode::ZpX, 1), OpInfo::new("AND", Mode::ZpX, 1), OpInfo::new("ROL", Mode::ZpX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("SEC", Mode::Implied, 0), OpInfo::new("AND", Mode::AbsY, 1), OpInfo::new("DEC", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BIT", Mode::AbsX, 1), OpInfo::new("AND", Mode::AbsX, 1), OpInfo::new("ROL", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("RTI", Mode::Implied, 20), OpInfo::new("EOR", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::Zp, 0), OpInfo::new("EOR", Mode::Zp, 1), OpInfo::new("LSR", Mode::Zp, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("PHA", Mode::Implied, 24), OpInfo::new("EOR", Mode::Imm, 32), OpInfo::new("LSR", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("JMP", Mode::Abs, 0), OpInfo::new("EOR", Mode::Abs, 1), OpInfo::new("LSR", Mode::Abs, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BVC", Mode::Rel, 0), OpInfo::new("EOR", Mode::IndZpY, 1), OpInfo::new("EOR", Mode::IndZp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("EOR", Mode::ZpX, 1), OpInfo::new("LSR", Mode::ZpX, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CLI", Mode::Implied, 0), OpInfo::new("EOR", Mode::AbsY, 1), OpInfo::new("PHY", Mode::Implied, 24), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("EOR", Mode::AbsX, 1), OpInfo::new("LSR", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("RTS", Mode::Implied, 20), OpInfo::new("ADC", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STZ", Mode::Zp, 2), OpInfo::new("ADC", Mode::Zp, 1), OpInfo::new("ROR", Mode::Zp, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("PLA", Mode::Implied, 20), OpInfo::new("ADC", Mode::Imm, 32), OpInfo::new("ROR", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("JMP", Mode::IndAbs, 1), OpInfo::new("ADC", Mode::Abs, 1), OpInfo::new("ROR", Mode::Abs, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BVS", Mode::Rel, 0), OpInfo::new("ADC", Mode::IndZpY, 1), OpInfo::new("ADC", Mode::IndZp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STZ", Mode::ZpX, 2), OpInfo::new("ADC", Mode::ZpX, 1), OpInfo::new("ROR", Mode::ZpX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("SEI", Mode::Implied, 0), OpInfo::new("ADC", Mode::AbsY, 1), OpInfo::new("PLY", Mode::Implied, 20), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("JMP", Mode::IndAbsX, 1), OpInfo::new("ADC", Mode::AbsX, 1), OpInfo::new("ROR", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BRA", Mode::Rel, 0), OpInfo::new("STA", Mode::IndZpX, 2), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STY", Mode::Zp, 2), OpInfo::new("STA", Mode::Zp, 2), OpInfo::new("STX", Mode::Zp, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("DEY", Mode::Implied, 0), OpInfo::new("BIT", Mode::Imm, 32), OpInfo::new("TXA", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STY", Mode::Abs, 2), OpInfo::new("STA", Mode::Abs, 2), OpInfo::new("STX", Mode::Abs, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BCC", Mode::Rel, 0), OpInfo::new("STA", Mode::IndZpY, 2), OpInfo::new("STA", Mode::IndZp, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STY", Mode::ZpX, 2), OpInfo::new("STA", Mode::ZpX, 2), OpInfo::new("STX", Mode::ZpY, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TYA", Mode::Implied, 0), OpInfo::new("STA", Mode::AbsY, 2), OpInfo::new("TXS", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("STZ", Mode::Abs, 2), OpInfo::new("STA", Mode::AbsX, 2), OpInfo::new("STZ", Mode::AbsX, 2), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("LDY", Mode::Imm, 32), OpInfo::new("LDA", Mode::IndZpX, 1), OpInfo::new("LDX", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("LDY", Mode::Zp, 1), OpInfo::new("LDA", Mode::Zp, 1), OpInfo::new("LDX", Mode::Zp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("TAY", Mode::Implied, 0), OpInfo::new("LDA", Mode::Imm, 32), OpInfo::new("TAX", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("LDY", Mode::Abs, 1), OpInfo::new("LDA", Mode::Abs, 1), OpInfo::new("LDX", Mode::Abs, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BCS", Mode::Rel, 0), OpInfo::new("LDA", Mode::IndZpY, 1), OpInfo::new("LDA", Mode::IndZp, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("LDY", Mode::ZpX, 1), OpInfo::new("LDA", Mode::ZpX, 1), OpInfo::new("LDX", Mode::ZpY, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CLV", Mode::Implied, 0), OpInfo::new("LDA", Mode::AbsY, 1), OpInfo::new("TSX", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("LDY", Mode::AbsX, 1), OpInfo::new("LDA", Mode::AbsX, 1), OpInfo::new("LDX", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPY", Mode::Imm, 32), OpInfo::new("CMP", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPY", Mode::Zp, 1), OpInfo::new("CMP", Mode::Zp, 1), OpInfo::new("DEC", Mode::Zp, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("INY", Mode::Implied, 0), OpInfo::new("CMP", Mode::Imm, 32), OpInfo::new("DEX", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPY", Mode::Abs, 1), OpInfo::new("CMP", Mode::Abs, 1), OpInfo::new("DEC", Mode::Abs, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BNE", Mode::Rel, 0), OpInfo::new("CMP", Mode::IndZpY, 1), OpInfo::new("CMP", Mode::IndZp, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("CMP", Mode::ZpX, 1), OpInfo::new("DEC", Mode::ZpX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CLD", Mode::Implied, 0), OpInfo::new("CMP", Mode::AbsY, 1), OpInfo::new("PHX", Mode::Implied, 24), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("CMP", Mode::AbsX, 1), OpInfo::new("DEC", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPX", Mode::Imm, 32), OpInfo::new("SBC", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPX", Mode::Zp, 1), OpInfo::new("SBC", Mode::Zp, 1), OpInfo::new("INC", Mode::Zp, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("INX", Mode::Implied, 0), OpInfo::new("SBC", Mode::Imm, 1), OpInfo::new("NOP", Mode::Implied, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("CPX", Mode::Abs, 1), OpInfo::new("SBC", Mode::Abs, 1), OpInfo::new("INC", Mode::Abs, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("BEQ", Mode::Rel, 0), OpInfo::new("SBC", Mode::IndZpY, 1), OpInfo::new("SBC", Mode::IndZp, 0), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("SBC", Mode::ZpX, 1), OpInfo::new("INC", Mode::ZpX, 3), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("SED", Mode::Implied, 0), OpInfo::new("SBC", Mode::AbsY, 1), OpInfo::new("PLX", Mode::Implied, 20), OpInfo::new("nop", Mode::Implied, 0),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("SBC", Mode::AbsX, 1), OpInfo::new("INC", Mode::AbsX, 3), OpInfo::new("nop", Mode::Implied, 0),
];

pub static OPCODES_6502: [OpInfo; 256] = [
    OpInfo::new("BRK", Mode::Implied, 24), OpInfo::new("ORA", Mode::IndZpX, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("aso", Mode::IndZpX, 3),
    OpInfo::new("nop", Mode::Zp, 1), OpInfo::new("ORA", Mode::Zp, 1), OpInfo::new("ASL", Mode::Zp, 3), OpInfo::new("aso", Mode::Zp, 3),
    OpInfo::new("PHP", Mode::Implied, 24), OpInfo::new("ORA", Mode::Imm, 32), OpInfo::new("ASL", Mode::Implied, 0), OpInfo::new("anc", Mode::Imm, 32),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("ORA", Mode::Abs, 1), OpInfo::new("ASL", Mode::Abs, 3), OpInfo::new("aso", Mode::Abs, 3),
    OpInfo::new("BPL", Mode::Rel, 0), OpInfo::new("ORA", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("aso", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("ORA", Mode::ZpX, 1), OpInfo::new("ASL", Mode::ZpX, 3), OpInfo::new("aso", Mode::ZpX, 3),
    OpInfo::new("CLC", Mode::Implied, 0), OpInfo::new("ORA", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("aso", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("ORA", Mode::AbsX, 1), OpInfo::new("ASL", Mode::AbsX, 3), OpInfo::new("aso", Mode::AbsX, 3),
    OpInfo::new("JSR", Mode::Abs, 24), OpInfo::new("AND", Mode::IndZpX, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("rla", Mode::IndZpX, 3),
    OpInfo::new("BIT", Mode::Zp, 1), OpInfo::new("AND", Mode::Zp, 1), OpInfo::new("ROL", Mode::Zp, 3), OpInfo::new("rla", Mode::Zp, 3),
    OpInfo::new("PLP", Mode::Implied, 20), OpInfo::new("AND", Mode::Imm, 32), OpInfo::new("ROL", Mode::Implied, 0), OpInfo::new("anc", Mode::Imm, 32),
    OpInfo::new("BIT", Mode::Abs, 1), OpInfo::new("AND", Mode::Abs, 1), OpInfo::new("ROL", Mode::Abs, 3), OpInfo::new("rla", Mode::Abs, 3),
    OpInfo::new("BMI", Mode::Rel, 0), OpInfo::new("AND", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("rla", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("AND", Mode::ZpX, 1), OpInfo::new("ROL", Mode::ZpX, 3), OpInfo::new("rla", Mode::ZpX, 3),
    OpInfo::new("SEC", Mode::Implied, 0), OpInfo::new("AND", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("rla", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("AND", Mode::AbsX, 1), OpInfo::new("ROL", Mode::AbsX, 3), OpInfo::new("rla", Mode::AbsX, 3),
    OpInfo::new("RTI", Mode::Implied, 20), OpInfo::new("EOR", Mode::IndZpX, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("lse", Mode::IndZpX, 3),
    OpInfo::new("nop", Mode::Zp, 0), OpInfo::new("EOR", Mode::Zp, 1), OpInfo::new("LSR", Mode::Zp, 3), OpInfo::new("lse", Mode::Zp, 3),
    OpInfo::new("PHA", Mode::Implied, 24), OpInfo::new("EOR", Mode::Imm, 32), OpInfo::new("LSR", Mode::Implied, 0), OpInfo::new("alr", Mode::Imm, 32),
    OpInfo::new("JMP", Mode::Abs, 0), OpInfo::new("EOR", Mode::Abs, 1), OpInfo::new("LSR", Mode::Abs, 3), OpInfo::new("lse", Mode::Abs, 3),
    OpInfo::new("BVC", Mode::Rel, 0), OpInfo::new("EOR", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("lse", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("EOR", Mode::ZpX, 1), OpInfo::new("LSR", Mode::ZpX, 3), OpInfo::new("lse", Mode::ZpX, 3),
    OpInfo::new("CLI", Mode::Implied, 0), OpInfo::new("EOR", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("lse", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("EOR", Mode::AbsX, 1), OpInfo::new("LSR", Mode::AbsX, 3), OpInfo::new("lse", Mode::AbsX, 3),
    OpInfo::new("RTS", Mode::Implied, 20), OpInfo::new("ADC", Mode::IndZpX, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("rra", Mode::IndZpX, 3),
    OpInfo::new("nop", Mode::Zp, 0), OpInfo::new("ADC", Mode::Zp, 1), OpInfo::new("ROR", Mode::Zp, 3), OpInfo::new("rra", Mode::Zp, 3),
    OpInfo::new("PLA", Mode::Implied, 20), OpInfo::new("ADC", Mode::Imm, 32), OpInfo::new("ROR", Mode::Implied, 0), OpInfo::new("arr", Mode::Imm, 32),
    OpInfo::new("JMP", Mode::IndAbs, 1), OpInfo::new("ADC", Mode::Abs, 1), OpInfo::new("ROR", Mode::Abs, 3), OpInfo::new("rra", Mode::Abs, 3),
    OpInfo::new("BVS", Mode::Rel, 0), OpInfo::new("ADC", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("rra", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("ADC", Mode::ZpX, 1), OpInfo::new("ROR", Mode::ZpX, 3), OpInfo::new("rra", Mode::ZpX, 3),
    OpInfo::new("SEI", Mode::Implied, 0), OpInfo::new("ADC", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("rra", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("ADC", Mode::AbsX, 1), OpInfo::new("ROR", Mode::AbsX, 3), OpInfo::new("rra", Mode::AbsX, 3),
    OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("STA", Mode::IndZpX, 2), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("axs", Mode::IndZpX, 2),
    OpInfo::new("STY", Mode::Zp, 2), OpInfo::new("STA", Mode::Zp, 2), OpInfo::new("STX", Mode::Zp, 2), OpInfo::new("axs", Mode::Zp, 2),
    OpInfo::new("DEY", Mode::Implied, 0), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("TXA", Mode::Implied, 0), OpInfo::new("xaa", Mode::Imm, 32),
    OpInfo::new("STY", Mode::Abs, 2), OpInfo::new("STA", Mode::Abs, 2), OpInfo::new("STX", Mode::Abs, 2), OpInfo::new("axs", Mode::Abs, 2),
    OpInfo::new("BCC", Mode::Rel, 0), OpInfo::new("STA", Mode::IndZpY, 2), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("axa", Mode::IndZpY, 2),
    OpInfo::new("STY", Mode::ZpX, 2), OpInfo::new("STA", Mode::ZpX, 2), OpInfo::new("STX", Mode::ZpY, 2), OpInfo::new("axs", Mode::ZpY, 2),
    OpInfo::new("TYA", Mode::Implied, 0), OpInfo::new("STA", Mode::AbsY, 2), OpInfo::new("TXS", Mode::Implied, 0), OpInfo::new("tas", Mode::AbsY, 2),
    OpInfo::new("say", Mode::AbsX, 2), OpInfo::new("STA", Mode::AbsX, 2), OpInfo::new("xas", Mode::AbsX, 2), OpInfo::new("axa", Mode::AbsY, 2),
    OpInfo::new("LDY", Mode::Imm, 32), OpInfo::new("LDA", Mode::IndZpX, 1), OpInfo::new("LDX", Mode::Imm, 32), OpInfo::new("lax", Mode::IndZpX, 1),
    OpInfo::new("LDY", Mode::Zp, 1), OpInfo::new("LDA", Mode::Zp, 1), OpInfo::new("LDX", Mode::Zp, 1), OpInfo::new("lax", Mode::Zp, 1),
    OpInfo::new("TAY", Mode::Implied, 0), OpInfo::new("LDA", Mode::Imm, 32), OpInfo::new("TAX", Mode::Implied, 0), OpInfo::new("oal", Mode::Imm, 32),
    OpInfo::new("LDY", Mode::Abs, 1), OpInfo::new("LDA", Mode::Abs, 1), OpInfo::new("LDX", Mode::Abs, 1), OpInfo::new("lax", Mode::Abs, 1),
    OpInfo::new("BCS", Mode::Rel, 0), OpInfo::new("LDA", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("lax", Mode::IndZpY, 1),
    OpInfo::new("LDY", Mode::ZpX, 1), OpInfo::new("LDA", Mode::ZpX, 1), OpInfo::new("LDX", Mode::ZpY, 1), OpInfo::new("lax", Mode::ZpY, 0),
    OpInfo::new("CLV", Mode::Implied, 0), OpInfo::new("LDA", Mode::AbsY, 1), OpInfo::new("TSX", Mode::Implied, 0), OpInfo::new("las", Mode::AbsY, 1),
    OpInfo::new("LDY", Mode::AbsX, 1), OpInfo::new("LDA", Mode::AbsX, 1), OpInfo::new("LDX", Mode::AbsY, 1), OpInfo::new("lax", Mode::AbsY, 1),
    OpInfo::new("CPY", Mode::Imm, 32), OpInfo::new("CMP", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("dcm", Mode::IndZpX, 3),
    OpInfo::new("CPY", Mode::Zp, 1), OpInfo::new("CMP", Mode::Zp, 1), OpInfo::new("DEC", Mode::Zp, 3), OpInfo::new("dcm", Mode::Zp, 3),
    OpInfo::new("INY", Mode::Implied, 0), OpInfo::new("CMP", Mode::Imm, 32), OpInfo::new("DEX", Mode::Implied, 0), OpInfo::new("sax", Mode::Imm, 32),
    OpInfo::new("CPY", Mode::Abs, 1), OpInfo::new("CMP", Mode::Abs, 1), OpInfo::new("DEC", Mode::Abs, 3), OpInfo::new("dcm", Mode::Abs, 3),
    OpInfo::new("BNE", Mode::Rel, 0), OpInfo::new("CMP", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("dcm", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("CMP", Mode::ZpX, 1), OpInfo::new("DEC", Mode::ZpX, 3), OpInfo::new("dcm", Mode::ZpX, 3),
    OpInfo::new("CLD", Mode::Implied, 0), OpInfo::new("CMP", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("dcm", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("CMP", Mode::AbsX, 1), OpInfo::new("DEC", Mode::AbsX, 3), OpInfo::new("dcm", Mode::AbsX, 3),
    OpInfo::new("CPX", Mode::Imm, 32), OpInfo::new("SBC", Mode::IndZpX, 1), OpInfo::new("nop", Mode::Imm, 32), OpInfo::new("ins", Mode::IndZpX, 3),
    OpInfo::new("CPX", Mode::Zp, 1), OpInfo::new("SBC", Mode::Zp, 1), OpInfo::new("INC", Mode::Zp, 3), OpInfo::new("ins", Mode::Zp, 3),
    OpInfo::new("INX", Mode::Implied, 0), OpInfo::new("SBC", Mode::Imm, 32), OpInfo::new("NOP", Mode::Implied, 0), OpInfo::new("sbc", Mode::Imm, 32),
    OpInfo::new("CPX", Mode::Abs, 1), OpInfo::new("SBC", Mode::Abs, 1), OpInfo::new("INC", Mode::Abs, 3), OpInfo::new("ins", Mode::Abs, 3),
    OpInfo::new("BEQ", Mode::Rel, 0), OpInfo::new("SBC", Mode::IndZpY, 1), OpInfo::new("hlt", Mode::Implied, 0), OpInfo::new("ins", Mode::IndZpY, 3),
    OpInfo::new("nop", Mode::ZpX, 0), OpInfo::new("SBC", Mode::ZpX, 1), OpInfo::new("INC", Mode::ZpX, 3), OpInfo::new("ins", Mode::ZpX, 3),
    OpInfo::new("SED", Mode::Implied, 0), OpInfo::new("SBC", Mode::AbsY, 1), OpInfo::new("nop", Mode::Implied, 0), OpInfo::new("ins", Mode::AbsY, 3),
    OpInfo::new("nop", Mode::AbsX, 0), OpInfo::new("SBC", Mode::AbsX, 1), OpInfo::new("INC", Mode::AbsX, 3), OpInfo::new("ins", Mode::AbsX, 3),
];
