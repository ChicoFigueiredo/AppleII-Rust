//! Trace v1 of a boot from a disk image, in the same conditions as the AppleWin oracle
//! (`tools/conformance/oracle/run-audetto-oracle.sh --disk ... --memclear 2`): same RAM
//! power-on pattern, Disk II alone in slot 6, drive 1 holding the image, no input. The result is
//! compared with the oracle's trace by `conformance diff`.
//!
//! usage: disk_trace --model apple2plus|apple2e-enhanced --disk IMAGE --cycles N --out FILE
//!        [--wp] [--write-test]    (--write-test: not used for traces, see the tests)

use apple2_conformance::case::parse_model;
use apple2_conformance::trace::{TraceRow, TraceWriter};
use apple2_core::{Machine, MachineConfig};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use std::io::Write;

/// `MemReset()` with `MIP_FF_FF_00_00` and the oracle patch (Memory.cpp:2342-2435): `FF FF 00 00`
/// in $0000-$BFFF, the "random" exceptions fixed at 00, RNDL/RNDH = $20, and three patched spots.
pub fn applewin_ram_pattern(m: &mut Machine, memclear: u32) {
    if memclear == 0 {
        m.poke_main(0x4E, 0x20);
        m.poke_main(0x4F, 0x20);
        return;
    }
    for a in (0..0xC000u32).step_by(4) {
        m.poke_main(a as u16, 0xFF);
        m.poke_main(a as u16 + 1, 0xFF);
        m.poke_main(a as u16 + 2, 0);
        m.poke_main(a as u16 + 3, 0);
    }
    for base in (0..0xC000u32).step_by(512) {
        for off in [0x28u32, 0x29, 0x68, 0x69] {
            m.poke_main((base + off) as u16, 0);
        }
    }
    m.poke_main(0x4E, 0x20);
    m.poke_main(0x4F, 0x20);
    m.poke_main(0x620B, 0);
    for a in [0xBFFDu16, 0xBFFE, 0xBFFF] {
        m.poke_main(a, 0);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .unwrap_or_else(|| panic!("{k} required"))
            .clone()
    };
    let model = parse_model(&get("--model")).expect("model");
    let disk = get("--disk");
    let cycles: u64 = get("--cycles").parse().unwrap();
    let out = get("--out");

    let cfg = MachineConfig::for_model(model);
    let mut m = Machine::new(cfg);
    let mut card = Disk2Card::new();
    let bytes = std::fs::read(&disk).expect("disk");
    let img = DiskImage::from_bytes(&disk, bytes).expect("image");
    card.insert_disk(0, img);
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    let memclear = args
        .iter()
        .position(|a| a == "--memclear")
        .map_or(2, |i| args[i + 1].parse().unwrap());
    applewin_ram_pattern(&mut m, memclear);
    // Video scanner phase at power-on (cycles into the frame); the oracle may differ from 0.
    if let Some(i) = args.iter().position(|a| a == "--phase") {
        let mut st = m.save_state();
        st.cycles_in_frame = args[i + 1].parse().unwrap();
        m.load_state(&st).expect("phase");
    }

    let mut w = TraceWriter::new(
        std::io::BufWriter::new(std::fs::File::create(&out).unwrap()),
        &[
            ("trace_format", "v1"),
            ("oracle", "rust"),
            ("cycles", &cycles.to_string()),
            ("disk", &disk),
        ],
    )
    .unwrap();
    while m.cycles() < cycles {
        let mut row = TraceRow::from_machine(&m);
        row.p |= 0x30; // the oracle keeps bits 5 and 4 fixed at 1 (EF_TO_AF)
        w.record_row(&row).unwrap();
        m.run_cycles(1); // like a 1 ms slice: also runs Card::update (motor spin-down)
    }
    w.finish().unwrap().flush().unwrap();
}
