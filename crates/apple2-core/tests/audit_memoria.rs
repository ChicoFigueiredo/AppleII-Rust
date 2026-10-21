//! HU-0102: regressoes das issues do upstream da secao "Memoria / Language Card / Saturn /
//! RamWorks" de docs/conformance/wave1-candidates.md. Cada teste reproduz o cenario da issue no
//! port; o oraculo e o C++ do AppleWin (LanguageCard.cpp, Memory.cpp) transcrito, a especificacao
//! de hardware (UTAIIe) ou o a2audit (crates/apple2-conformance/tests/a2audit.rs).
//!
//! Issues sem teste aqui (ver docs/conformance/results/memoria.csv): #1126 (WOZ/Total Replay),
//! #222, #225, #297 (software de terceiros sem fixture).

use apple2_core::card::{Card, CardCtx, SLOT_ROM_SIZE};
use apple2_core::irq::IrqSource;
use apple2_core::memory::mf;
use apple2_core::{AuxCardKind, CardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

// ---------------------------------------------------------------- helpers

/// 12K ][ ROM: $D000=$D0, $E000=$E0, $F800=$F8; RESET=$0300, IRQ=$6000.
fn rom_ii() -> Vec<u8> {
    let mut r = vec![0xEAu8; 0x3000];
    r[0] = 0xD0;
    r[0x1000] = 0xE0;
    r[0x2800] = 0xF8;
    r[0x2FFC..0x3000].copy_from_slice(&[0x00, 0x03, 0x00, 0x60]);
    r
}

/// 16K //e ROM: 4K internal Cx ROM ($CC, C3xx=$33) + the 12K above.
fn rom_e() -> Vec<u8> {
    let mut r = vec![0xCCu8; 0x1000];
    r[0x300..0x400].fill(0x33);
    r.extend(rom_ii());
    r
}

fn machine(model: Model, slots: &[(usize, CardKind)]) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.slots = [None; 8];
    if model.is_apple2() && model != Model::Apple2 {
        cfg.slots[0] = Some(CardKind::LanguageCard16K);
    }
    for &(s, k) in slots {
        cfg.slots[s] = Some(k);
    }
    cfg.custom_rom = Some(if model.is_apple2() { rom_ii() } else { rom_e() });
    Machine::new(cfg)
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke_main(at + i as u16, *b);
    }
}

/// Runs `n` instructions.
fn steps(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.step();
    }
}

fn flags(m: &Machine) -> u32 {
    m.memmode() & mf::LANGCARD_MASK
}

/// Slot card with no ROM of its own whose `$Cn00` space floats, like the Uthernet I
/// (`TfeIoCxxx` = `IO_Null`, Uthernet1.cpp:978) and a 6502-bus without a ROM chip.
struct FloatingRomCard;

impl Card for FloatingRomCard {
    fn name(&self) -> &'static str {
        "FloatingRom"
    }
    fn slot_rom_read(&mut self, _offset: u8, ctx: &mut CardCtx) -> u8 {
        ctx.floating_bus
    }
}

/// Slot card with ROM $A9 `id` $60 ... and a 2K expansion ROM that just records `$C800` writes
/// (regs[0]) and counts expansion-strobe accesses.
struct ExpCard {
    exp: Box<[u8; apple2_core::card::EXPANSION_ROM_SIZE]>,
    slot_rom: [u8; SLOT_ROM_SIZE],
    last_exp_write: u8,
}

impl Card for ExpCard {
    fn name(&self) -> &'static str {
        "Exp"
    }
    fn io_read(&mut self, lo: u8, _ctx: &mut CardCtx) -> u8 {
        if lo == 0 {
            self.last_exp_write
        } else {
            0
        }
    }
    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.slot_rom[offset as usize]
    }
    fn expansion_rom(&self) -> Option<&[u8; apple2_core::card::EXPANSION_ROM_SIZE]> {
        Some(&self.exp)
    }
    fn expansion_rom_write(&mut self, _offset: u16, value: u8, _ctx: &mut CardCtx) {
        self.last_exp_write = value;
    }
}

fn exp_card() -> Box<ExpCard> {
    Box::new(ExpCard {
        exp: Box::new([0xE5; apple2_core::card::EXPANSION_ROM_SIZE]),
        slot_rom: [0xA5; SLOT_ROM_SIZE],
        last_exp_write: 0,
    })
}

// ---------------------------------------------------------------- #164

/// #164 "MMU no good when no cards in slot-4 & 5": INTCXROM e SLOTC3ROM tem de funcionar na
/// //e e na //e Enhanced sem nenhum cartao. C++ Memory.cpp:1041-1086 (`IO_Cxxx`, GH#149/#164) e
/// UTAIIe:5-28; o a2audit (softswitch tests) confirma no port.
#[test]
fn issue_164_mmu_switches_work_with_empty_slots() {
    for model in [Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = machine(model, &[]);
        let internal = |m: &Machine, a: u16| m.board().memory.internal_cx(a).unwrap();
        // Default: INTCXROM off, SLOTC3ROM off.
        assert_eq!(rd(&mut m, 0xC015) & 0x80, 0, "{model:?} INTCXROM status");
        assert_eq!(rd(&mut m, 0xC017) & 0x80, 0, "{model:?} SLOTC3ROM status");
        assert_eq!(rd(&mut m, 0xC300), internal(&m, 0xC300), "C3xx is internal");
        let fb = m.board().floating_bus();
        assert_eq!(rd(&mut m, 0xC400), fb, "empty slot 4 floats");
        assert_eq!(rd(&mut m, 0xC500), m.board().floating_bus(), "empty slot 5");
        // INTCXROM on: internal ROM everywhere.
        wr(&mut m, 0xC007, 0);
        assert_eq!(rd(&mut m, 0xC015) & 0x80, 0x80);
        for a in [0xC100u16, 0xC400, 0xC500, 0xC7FF] {
            assert_eq!(rd(&mut m, a), internal(&m, a), "{model:?} {a:04X}");
        }
        wr(&mut m, 0xC006, 0);
        assert_eq!(rd(&mut m, 0xC015) & 0x80, 0);
        // SLOTC3ROM on with slot 3 empty: slot space floats, not the internal ROM.
        wr(&mut m, 0xC00B, 0);
        assert_eq!(rd(&mut m, 0xC017) & 0x80, 0x80);
        assert_eq!(rd(&mut m, 0xC300), m.board().floating_bus());
        wr(&mut m, 0xC00A, 0);
        assert_eq!(rd(&mut m, 0xC300), internal(&m, 0xC300));
    }
}

// ---------------------------------------------------------------- #392

/// #392 "a2audit errors": sequencia LDA $C080, LDA $C081, INC $D17B, INC $FE1F executada pela
/// CPU na //e Enhanced. Com so um acesso impar a LC continua protegida e a ROM/RAM nao mudam;
/// com dois ($C081 x2) o INC grava na RAM sob a ROM (a2audit langcard.asm).
#[test]
fn issue_392_a2audit_lc_sequence() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let rom_d17b = m.peek(0xD17B);
    let rom_fe1f = m.peek(0xFE1F);
    rd(&mut m, 0xC080); // RAM read, write protect, bank 2
    let ram_d17b = m.peek(0xD17B);
    let ram_fe1f = m.peek(0xFE1F);
    rd(&mut m, 0xC082); // ROM read, protected
                        // LDA $C080; LDA $C081; INC $D17B; INC $FE1F; JMP *
    let code = [
        0xAD, 0x80, 0xC0, 0xAD, 0x81, 0xC0, 0xEE, 0x7B, 0xD1, 0xEE, 0x1F, 0xFE, 0x4C, 0x0C, 0x03,
    ];
    load(&mut m, 0x300, &code);
    m.cpu_mut().pc = 0x300;
    steps(&mut m, 6);
    assert_eq!(flags(&m) & mf::WRITERAM, 0, "one odd read does not enable");
    assert_eq!(m.peek(0xD17B), rom_d17b, "ROM read after C081");
    assert_eq!(m.peek(0xFE1F), rom_fe1f);
    rd(&mut m, 0xC080);
    assert_eq!(
        m.peek(0xD17B),
        ram_d17b,
        "RAM under the ROM was not written"
    );
    assert_eq!(m.peek(0xFE1F), ram_fe1f);

    // Two odd reads: the INC writes (ROM value + 1) into the RAM under the ROM.
    rd(&mut m, 0xC082);
    let code = [
        0xAD, 0x81, 0xC0, 0xAD, 0x81, 0xC0, 0xEE, 0x7B, 0xD1, 0xEE, 0x1F, 0xFE, 0x4C, 0x0C, 0x03,
    ];
    load(&mut m, 0x300, &code);
    m.cpu_mut().pc = 0x300;
    steps(&mut m, 6);
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
    assert_eq!(m.peek(0xD17B), rom_d17b, "still reading the ROM");
    rd(&mut m, 0xC080);
    assert_eq!(m.peek(0xD17B), rom_d17b.wrapping_add(1));
    assert_eq!(m.peek(0xFE1F), rom_fe1f.wrapping_add(1));
}

// ---------------------------------------------------------------- #395

/// #395 "LC write-enabled state is not maintained correctly (1.26.2.0)": duas leituras de $C081
/// habilitam a escrita e quatro `STA $C083` seguidos nao a desligam (LanguageCard.cpp:134-150: um
/// acesso impar de escrita deixa WRITERAM como esta).
#[test]
fn issue_395_sta_c083_keeps_the_lc_writable() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model, &[]);
        rd(&mut m, 0xC081);
        rd(&mut m, 0xC081);
        assert_ne!(flags(&m) & mf::WRITERAM, 0, "{model:?}");
        for _ in 0..4 {
            wr(&mut m, 0xC083, 0);
            assert_ne!(flags(&m) & mf::WRITERAM, 0, "{model:?} after STA $C083");
        }
        wr(&mut m, 0xD000, 0x5A);
        wr(&mut m, 0xE000, 0xA5);
        assert_eq!(rd(&mut m, 0xD000), 0x5A, "{model:?} LC RAM read back");
        assert_eq!(rd(&mut m, 0xE000), 0xA5);
    }
}

/// #395, parte 2: a escrita (nao RMW) depois de uma leitura impar nao habilita nada.
#[test]
fn issue_395_write_after_odd_read_does_not_enable() {
    let mut m = machine(Model::Apple2Plus, &[]);
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081);
    wr(&mut m, 0xC081, 0);
    assert_eq!(flags(&m) & mf::WRITERAM, 0);
}

// ---------------------------------------------------------------- #400 / #422

/// IRQ (mouse no slot 4) com a CPU na ROM interna do //e: a entrada do IRQ nao toca nas chaves
/// de paginacao (CPU.cpp `IRQ()` empilha e le o vetor; nada altera `g_memmode`), logo INTCXROM e
/// a LC voltam iguais depois do RTI. #400 e #422 sao so a manifestacao em software de terceiros
/// (Quark Catalyst, BeagleWrite, Bejeweled), sem fixture.
fn irq_in_internal_rom(lc_ram_read: bool) {
    let mut m = machine(Model::Apple2eEnhanced, &[]);
    load(&mut m, 0x6000, &[0x40]); // IRQ handler in RAM: RTI
    if lc_ram_read {
        rd(&mut m, 0xC083);
        rd(&mut m, 0xC083);
        // IRQ vector in the LC RAM: still $6000 through the LC (copy it).
        for (i, b) in [0x00u8, 0x60].iter().enumerate() {
            wr(&mut m, 0xFFFE + i as u16, *b);
        }
    }
    wr(&mut m, 0xC007, 0); // INTCXROM on
    let before = m.memmode();
    m.cpu_mut().pc = 0xCCE0; // inside the internal ROM
    m.cpu_mut().p &= !0x04; // CLI; the IRQ is taken at the first instruction boundary
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    let cycles = m.step();
    assert_eq!(cycles, 7, "IRQ entry");
    assert_eq!(m.cpu().pc, 0x6000);
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, false);
    m.step(); // RTI
    assert_eq!(m.cpu().pc, 0xCCE0, "back in the internal ROM");
    assert_eq!(m.memmode(), before, "paging state untouched by the IRQ");
    assert_eq!(rd(&mut m, 0xC015) & 0x80, 0x80, "INTCXROM still on");
    assert_eq!(
        rd(&mut m, 0xC100),
        m.board().memory.internal_cx(0xC100).unwrap()
    );
}

#[test]
fn issue_400_mouse_irq_in_rom_keeps_paging() {
    irq_in_internal_rom(false);
    irq_in_internal_rom(true);
}

#[test]
fn issue_422_irq_from_internal_rom_keeps_intcxrom() {
    irq_in_internal_rom(false);
}

// ---------------------------------------------------------------- #700

/// #700 "ProDOS8 2.5.0 alpha6 not working": `INC $C08B` habilita a escrita da LC na 65C02
/// (opcode $EE) e `INC $C08B,X` com X=0 na 65C02 e na NMOS ($FE); o `INC abs` NMOS nao
/// (LanguageCard.cpp:173-202, `IsOpcodeRMWabs`). Executado pela CPU, nao pela chamada direta.
#[test]
fn issue_700_inc_c08b_by_rmw() {
    let run = |model: Model, code: &[u8], x: u8| -> bool {
        let mut m = machine(model, &[]);
        rd(&mut m, 0xC080); // a read pair is needed: the RMW's own read is the first odd access
        load(&mut m, 0x300, code);
        m.cpu_mut().pc = 0x300;
        m.cpu_mut().x = x;
        steps(&mut m, 1);
        flags(&m) & mf::WRITERAM != 0
    };
    // 65C02 (//e Enhanced): INC abs. The RMW reads first (arms the latch), then writes.
    assert!(
        run(Model::Apple2eEnhanced, &[0xEE, 0x8B, 0xC0], 0),
        "INC abs, 65C02"
    );
    // NMOS (][+): INC abs does not, INC abs,X with X=0 does.
    assert!(
        !run(Model::Apple2Plus, &[0xEE, 0x8B, 0xC0], 0),
        "INC abs, NMOS"
    );
    assert!(
        run(Model::Apple2Plus, &[0xFE, 0x8B, 0xC0], 0),
        "INC abs,X NMOS, X=0"
    );
    assert!(run(Model::Apple2eEnhanced, &[0xFE, 0x8B, 0xC0], 0));
    // X != 0: INC $C08A,X with X=1 touches $C08B, but the opcode operand is not $C08B.
    assert!(
        !run(Model::Apple2Plus, &[0xFE, 0x8A, 0xC0], 1),
        "operand != address"
    );
}

// ---------------------------------------------------------------- #1285

/// #1285: leitura de 16 bits em $FFFF (operando de `CMP $2000` em $FFFE, com a LC em HIGH RAM):
/// sem panic, o byte alto vem de $0000 (a pagina nao "dobra" para fora da RAM) e Z=1.
#[test]
fn issue_1285_operand_crossing_ffff() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model, &[]);
        rd(&mut m, 0xC083);
        rd(&mut m, 0xC083); // LC RAM read + write
        wr(&mut m, 0xFFFE, 0xCD); // CMP abs
        wr(&mut m, 0xFFFF, 0x00); // low byte of $2000
        m.poke_main(0x0000, 0x20); // high byte, wrapped to $0000
        m.poke_main(0x2000, 0x77);
        m.cpu_mut().a = 0x77;
        m.cpu_mut().pc = 0xFFFE;
        let c = m.step();
        assert_eq!(c, 4, "{model:?} CMP abs");
        assert_ne!(m.cpu().p & 0x02, 0, "{model:?} Z=1");
        assert_ne!(m.cpu().p & 0x01, 0, "C=1 (A >= M)");
        assert_eq!(m.cpu().pc, 0x0001, "PC wrapped past $FFFF");
    }
}

// ---------------------------------------------------------------- #1486 / #1495 / #1520

fn saturn_machine(model: Model, slot: usize, banks: u8) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.slots = [None; 8];
    cfg.slots[slot] = Some(CardKind::Saturn128K);
    cfg.saturn_banks[slot] = banks;
    cfg.custom_rom = Some(if model.is_apple2() { rom_ii() } else { rom_e() });
    Machine::new(cfg)
}

/// Selector offset ($C0n4..$C0nF) of Saturn bank `b` (LanguageCard.cpp:418-422).
fn bank_select(b: u8) -> u8 {
    4 | ((b & 4) << 1) | (b & 3)
}

fn saturn_ram_probe(m: &mut Machine, slot: usize, banks: u8) -> Vec<u8> {
    let base = 0xC080 + slot as u16 * 16;
    rd(m, base + 3);
    rd(m, base + 3); // write enabled, RAM read, bank 2
    for b in 0..banks.min(8) {
        rd(m, base + u16::from(bank_select(b)));
        wr(m, 0xD000, 0x10 + b);
    }
    (0..8)
        .map(|b| {
            rd(m, base + u16::from(bank_select(b)));
            rd(m, 0xD000)
        })
        .collect()
}

/// #1486 "Saturn 128K card in slot 0 does not use configured size": o tamanho configurado
/// (4 ou 8 bancos) chega a carta no slot 0 (e em qualquer slot), com aliasing a la
/// LanguageCard.cpp:424-430 acima da capacidade.
#[test]
fn issue_1486_saturn_size_reaches_the_card_in_slot_0_and_others() {
    for slot in [0usize, 4] {
        let m8 = &mut saturn_machine(Model::Apple2Plus, slot, 8);
        assert_eq!(
            saturn_ram_probe(m8, slot, 8),
            [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17],
            "slot {slot}: 8 independent banks"
        );
        let m4 = &mut saturn_machine(Model::Apple2Plus, slot, 4);
        assert_eq!(
            saturn_ram_probe(m4, slot, 4),
            [0x10, 0x11, 0x12, 0x13, 0x13, 0x13, 0x13, 0x13],
            "slot {slot}: banks >= 4 alias bank 3 (C++ :430)"
        );
    }
}

/// #1495 "Saturn128 in AppleWin": Saturn em qualquer slot (0-7) sem panic, e o cenario do
/// GH#1495 em `LanguageCardManager::SetMemMode` (slot 0 LC habilitado para escrita + Saturn do
/// slot 4 protegido, ROM em leitura): a escrita em $D000 cai na LC do slot 0.
#[test]
fn issue_1495_saturn_in_any_slot_and_dos_copy_to_slot0_lc() {
    for slot in 0..8usize {
        let m = &mut saturn_machine(Model::Apple2Plus, slot, 8);
        let probe = saturn_ram_probe(m, slot, 8);
        assert_eq!(probe[7], 0x17, "slot {slot}");
        // all 16 switches, read and write, never panic
        for lo in 0..16u16 {
            rd(m, 0xC080 + slot as u16 * 16 + lo);
            wr(m, 0xC080 + slot as u16 * 16 + lo, 0);
        }
    }
    let mut m = machine(Model::Apple2Plus, &[(4, CardKind::Saturn128K)]);
    rd(&mut m, 0xC081);
    rd(&mut m, 0xC081); // slot 0 LC: ROM read, write enabled
    rd(&mut m, 0xC0C2); // slot 4 Saturn: ROM read, write protect (effectively disabled)
    wr(&mut m, 0xD000, 0x77); // "DOS is copied to LC"
    rd(&mut m, 0xC080); // slot 0: RAM read
    assert_eq!(rd(&mut m, 0xD000), 0x77, "the write reached the slot-0 LC");
}

/// #1520 "Can't start any Merlin version": estado inicial da LC e da Saturn. LC (slot 0 do ][+
/// e a embutida da //e) nasce com BANK2|WRITERAM; a Saturn nasce protegida contra escrita, no
/// power-on e no reset da //e; o do ][+ nao muda no reset (LanguageCard.cpp:89-100).
#[test]
fn issue_1520_initial_lc_and_saturn_state() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model, &[(4, CardKind::Saturn128K)]);
        assert_eq!(
            m.board().lc.flags(4).unwrap() & mf::WRITERAM,
            0,
            "{model:?} Saturn protected"
        );
        if model.is_apple2() {
            assert_eq!(
                m.board().lc.flags(0),
                Some(mf::BANK2 | mf::WRITERAM),
                "slot-0 LC"
            );
        }
        // Unlock the Saturn, then reset/power-cycle.
        rd(&mut m, 0xC0C3);
        rd(&mut m, 0xC0C3);
        assert_ne!(m.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
        m.reset();
        let after_reset = m.board().lc.flags(4).unwrap() & mf::WRITERAM;
        if model.is_apple2() {
            assert_ne!(after_reset, 0, "][+ reset leaves the Saturn alone");
        } else {
            assert_eq!(after_reset, 0, "//e reset write-protects the Saturn");
        }
        m.power_cycle();
        assert_eq!(m.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
    }
    // Writing with the initial state: ProDOS-style copy into the built-in LC works at once.
    let mut e = machine(Model::Apple2eEnhanced, &[(4, CardKind::Saturn128K)]);
    wr(&mut e, 0xC08B, 0); // RAM read bank 1 (writes go to the RAM anyway)
    assert_eq!(
        flags(&e) & mf::WRITERAM,
        mf::WRITERAM,
        "built-in LC is write enabled at power-on"
    );
}

// ---------------------------------------------------------------- RamWorks

fn ramworks(banks: u16) -> Machine {
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks };
    cfg.custom_rom = Some(rom_e());
    Machine::new(cfg)
}

/// #158 "RAM inflation not working?": RamWorks com 127 bancos, um padrao por banco via $C073,
/// relido sem aliasing. (O valor 127 e o `-r 127` do relato.)
#[test]
fn issue_158_127_ramworks_banks_are_addressable() {
    let mut m = ramworks(127);
    wr(&mut m, 0xC005, 0); // RAMWRT
    wr(&mut m, 0xC003, 0); // RAMRD
    for b in 0..127u8 {
        wr(&mut m, 0xC073, b);
        wr(&mut m, 0x0300, b ^ 0x5A);
        wr(&mut m, 0xBFFF, b.wrapping_mul(3));
    }
    for b in 0..127u8 {
        wr(&mut m, 0xC073, b);
        assert_eq!(rd(&mut m, 0x0300), b ^ 0x5A, "bank {b}");
        assert_eq!(rd(&mut m, 0xBFFF), b.wrapping_mul(3), "bank {b}");
    }
    // Out of range: the bank register ignores it (Memory.cpp:2557), still bank 126.
    wr(&mut m, 0xC073, 126);
    wr(&mut m, 0xC073, 200);
    assert_eq!(m.board().memory.aux_bank(), 126);
}

/// #520 e #1414: o Reset (Ctrl-Reset) nao muda o banco do RamWorks (manual do RamWorks III,
/// pg 45: "initialized to zero on a power-up, but not after a reset"; Memory.cpp so zera em
/// `MemReset`, :2314); o power-cycle (F2) volta ao banco 0.
#[test]
fn issue_520_and_1414_ramworks_bank_vs_reset_and_power_cycle() {
    let mut m = ramworks(4);
    wr(&mut m, 0xC073, 3);
    assert_eq!(m.board().memory.aux_bank(), 3);
    m.reset();
    assert_eq!(m.board().memory.aux_bank(), 3, "Reset keeps the bank (C++)");
    m.power_cycle();
    assert_eq!(
        m.board().memory.aux_bank(),
        0,
        "power-cycle returns to bank 0 (GH#1414)"
    );
}

/// #1414 pelo caminho do relato: `C073:1`, F2.
#[test]
fn issue_1414_c073_1_then_f2() {
    let mut m = ramworks(16);
    wr(&mut m, 0xC073, 1);
    m.power_cycle();
    assert_eq!(m.board().memory.aux_bank(), 0);
    wr(&mut m, 0xC005, 0); // RAMWRT
    wr(&mut m, 0xC003, 0); // RAMRD
    wr(&mut m, 0x0300, 0x42);
    wr(&mut m, 0xC073, 1);
    wr(&mut m, 0x0300, 0x43);
    wr(&mut m, 0xC073, 0);
    assert_eq!(rd(&mut m, 0x0300), 0x42);
}

/// #1428 "Enhanced //e RAM issue?": a //e Enhanced padrao tem 128K (64K principal + 64K aux
/// independentes) e a RAM pode ser ajustada (RamWorks, ver apple2-config/tests/audit_memoria.rs).
#[test]
fn issue_1428_enhanced_iie_default_has_128k() {
    let mut m = Machine::new(MachineConfig::default());
    assert_eq!(m.config().model, Model::Apple2eEnhanced);
    assert_eq!(m.config().aux_card, AuxCardKind::Extended80Col);
    for a in (0x200u16..0xC000).step_by(0x101) {
        m.poke_main(a, 0xAA);
        m.poke_aux(a, 0x55);
    }
    for a in (0x200u16..0xC000).step_by(0x101) {
        assert_eq!((m.peek_main(a), m.peek_aux(a)), (0xAA, 0x55), "{a:04X}");
    }
    // 80STORE-independent aux access via RAMRD/RAMWRT
    wr(&mut m, 0xC005, 0);
    wr(&mut m, 0x1000, 0x99);
    assert_eq!(m.peek_aux(0x1000), 0x99);
    assert_ne!(m.peek_main(0x1000), 0x99);
}

// ---------------------------------------------------------------- slots: #149, #190, #418, #423

/// #149 "Card slot ROM issues with copy protection routines": a ROM de um slot vazio e o
/// barramento flutuante (nao RAM de cartao), e escritas nela sao ignoradas (Memory.cpp:1076-1080).
#[test]
fn issue_149_empty_slot_rom_is_floating_and_not_writable() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model, &[]);
        for slot in [1u16, 4, 5, 7] {
            let a = 0xC000 + slot * 0x100 + 0x20;
            wr(&mut m, a, 0x6B);
            let fb = m.board().floating_bus();
            assert_eq!(rd(&mut m, a), fb, "{model:?} slot {slot}");
            assert_ne!(rd(&mut m, a), 0x6B);
        }
    }
}

/// #190 "Writes to $C800-CFFF should match reads": um STA em $C800-$CFFF aciona o IO_STROBE
/// como a leitura (a expansao fica selecionada e a escrita chega ao cartao). C++ Memory.cpp:990
/// (a verificacao e a mesma para leitura e escrita: `IO_Cxxx` e comum).
#[test]
fn issue_190_write_to_c800_strobes_like_a_read() {
    // Read path: $C500 then a read of $C800 selects slot 5's expansion ROM.
    let mut r = machine(Model::Apple2Plus, &[]);
    assert!(r.insert_card(5, exp_card()));
    rd(&mut r, 0xC500);
    assert_eq!(rd(&mut r, 0xC800), 0xE5);
    // Write path: $C500 then only a WRITE to $C800: the strobe selects the expansion ROM, so a
    // following read shows it and the write reached the card.
    let mut w = machine(Model::Apple2Plus, &[]);
    assert!(w.insert_card(5, exp_card()));
    rd(&mut w, 0xC500);
    wr(&mut w, 0xC800, 0x3C);
    assert_eq!(
        rd(&mut w, 0xC800),
        0xE5,
        "expansion ROM selected by the write"
    );
    assert_eq!(rd(&mut w, 0xC0D0), 0x3C, "and the write reached the card");
}

/// #418 "Slot 3 ROM space does not float when Uthernet is enabled": //e com SLOTC3ROM ligado e
/// um cartao sem ROM no slot 3 (Uthernet): $C3xx e o barramento flutuante, nao $00 (o cartao
/// responde com `IO_Null`; o port repassa `floating_bus`). Com SLOTC3ROM desligado a ROM interna
/// do 80 colunas aparece (UTAIIe:5-28).
#[test]
fn issue_418_slot3_rom_space_floats_with_a_romless_card() {
    let mut m = machine(Model::Apple2e, &[]);
    assert!(m.insert_card(3, Box::new(FloatingRomCard)));
    assert_eq!(
        rd(&mut m, 0xC315),
        0x33,
        "SLOTC3ROM off: internal 80-col firmware"
    );
    wr(&mut m, 0xC00B, 0); // SLOTC3ROM on: firmware switched out
    let fb = m.board().floating_bus();
    assert_eq!(rd(&mut m, 0xC315), fb);
    assert_eq!(rd(&mut m, 0xC3FF), m.board().floating_bus());
    // ProDOS 1.1's test: the value changes with the video scanner (it floats).
    for a in 0x400..0x800u16 {
        m.poke_main(a, a as u8);
    }
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..3000 {
        m.step();
        seen.insert(rd(&mut m, 0xC315));
    }
    assert!(
        seen.len() > 1,
        "$C315 follows the floating bus: {seen:02X?}"
    );
}

/// #423 "Support INTC8ROM soft switch for //e": acessar $C3xx com SLOTC3ROM desligado seleciona
/// a ROM interna em $C800-$CFFF (INTC8ROM) ate o acesso a $CFFF (Memory.cpp:982-988).
#[test]
fn issue_423_intc8rom_is_implied_by_c3xx_access() {
    let mut m = machine(Model::Apple2eEnhanced, &[]);
    assert!(m.insert_card(5, exp_card()));
    rd(&mut m, 0xC500);
    assert_eq!(rd(&mut m, 0xC800), 0xE5, "slot 5 expansion ROM selected");
    rd(&mut m, 0xC300); // SLOTC3ROM off: INTC8ROM
    assert!(m.board().slots.intc8rom());
    assert_eq!(rd(&mut m, 0xC800), 0xCC, "internal ROM at $C800");
    rd(&mut m, 0xC500); // cannot override INTC8ROM
    assert_eq!(rd(&mut m, 0xC800), 0xCC);
    rd(&mut m, 0xCFFF);
    assert!(!m.board().slots.intc8rom());
    rd(&mut m, 0xC500);
    assert_eq!(rd(&mut m, 0xC800), 0xE5, "released by $CFFF");
}
