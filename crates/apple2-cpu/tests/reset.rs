//! Reset and power-on state, following AppleWin `CPU.cpp` (`CpuInitialize`, `CpuReset`).
use apple2_cpu::*;

struct Ram(Box<[u8; 65536]>);

impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.0[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.0[a as usize] = v;
    }
}

fn ram() -> Ram {
    let mut r = Ram(Box::new([0; 65536]));
    r.0[0xFFFC] = 0x62;
    r.0[0xFFFD] = 0xFA;
    r
}

#[test]
fn reset_follows_applewin_cpureset() {
    // I set, D cleared on the 65C02 only, other flags and A/X/Y kept, S decremented by 3 (the
    // three suppressed pushes), PC from the reset vector, jam cleared.
    for v in [Variant::Nmos6502, Variant::Cmos65C02] {
        let mut r = ram();
        let mut c = Cpu::new(v);
        (c.a, c.x, c.y, c.sp, c.p) = (1, 2, 3, 0xF0, FLAG_N | FLAG_D | FLAG_C);
        c.jammed = true;
        c.reset(&mut r);
        assert_eq!((c.a, c.x, c.y, c.sp, c.pc), (1, 2, 3, 0xED, 0xFA62), "{v:?}");
        assert!(c.p & (FLAG_I | FLAG_N | FLAG_C) == FLAG_I | FLAG_N | FLAG_C);
        assert_eq!(c.p & FLAG_D != 0, v == Variant::Nmos6502);
        assert!(!c.jammed);
    }
}

#[test]
fn power_on_registers_are_ff_and_stack_ends_at_fc() {
    // CpuInitialize(): A = X = Y = $FF, S = $1FF, then CpuReset().
    let mut r = ram();
    let mut c = Cpu::new(Variant::Nmos6502);
    assert_eq!((c.a, c.x, c.y, c.sp), (0xFF, 0xFF, 0xFF, 0xFF));
    c.reset(&mut r);
    assert_eq!(c.sp, 0xFC);
}
