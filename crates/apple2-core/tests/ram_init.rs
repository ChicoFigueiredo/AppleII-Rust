//! Power-on RAM pattern in a whole machine (`-memclear`, AppleWin `MemReset`).

use apple2_core::{Machine, MachineConfig, Model, RamInit, RandomSource};

fn machine(model: Model, init: RamInit) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.ram_init = init;
    Machine::new(cfg)
}

#[test]
fn for_model_keeps_ram_blank() {
    let m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert_eq!((m.peek_main(0x0100), m.peek_main(0x2000)), (0, 0));
}

#[test]
fn applewin_default_pattern_and_oracle_deterministic_draw() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let m = machine(model, RamInit::APPLEWIN_DETERMINISTIC);
        let main: Vec<u8> = (0x2000..0x2008).map(|a| m.peek_main(a)).collect();
        assert_eq!(main, [0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
        // Oracle: xx28/xx29/xx68/xx69 of $00-$BF are 00, RNDL/RNDH are $20.
        assert_eq!(
            [0x2028, 0x2029, 0x2068, 0x2069].map(|a| m.peek_main(a)),
            [0; 4]
        );
        assert_eq!((m.peek_main(0x4E), m.peek_main(0x4F)), (0x20, 0x20));
        // Only main RAM below $C000 is patterned; aux RAM stays zero as in AppleWin.
        assert_eq!(m.peek_main(0xC000), 0);
        assert!((0..0x10000u32).all(|a| m.peek_aux(a as u16) == 0));
    }
}

#[test]
fn power_cycle_applies_the_pattern_again() {
    let mut m = machine(Model::Apple2Plus, RamInit::APPLEWIN_DETERMINISTIC);
    m.poke_main(0x2000, 0x55);
    m.poke_main(0x2002, 0x66);
    m.power_cycle();
    assert_eq!((m.peek_main(0x2000), m.peek_main(0x2002)), (0xFF, 0));
}

#[test]
fn seed_makes_the_random_bytes_reproducible() {
    let init = |seed| RamInit {
        pattern: 2,
        random: RandomSource::Seed(seed),
    };
    let a = machine(Model::Apple2Plus, init(1));
    let b = machine(Model::Apple2Plus, init(1));
    let c = machine(Model::Apple2Plus, init(2));
    let dump = |m: &Machine| {
        (0..0xC000u32)
            .map(|a| m.peek_main(a as u16))
            .collect::<Vec<_>>()
    };
    assert_eq!(dump(&a), dump(&b));
    assert_ne!(dump(&a), dump(&c));
    assert!(a.peek_main(0x4E) >= 0x20 && a.peek_main(0x4F) >= 0x20);
}
