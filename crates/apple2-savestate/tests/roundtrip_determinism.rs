//! HU-0402: save -> restart -> load -> step N gives the same state hash; caminhos com espaço,
//! Unicode e contrabarra; valores extremos.

mod common;
use apple2_core::{Machine, MachineConfig, MachineState, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use common::*;

/// Answers every image request with the DOS 3.3 master and records what was asked.
#[derive(Default)]
struct AnyImage {
    asked: Vec<(String, String)>,
}

impl LoadHost for AnyImage {
    fn open_image(&mut self, req: &ImageRequest) -> Result<DiskImage, String> {
        self.asked
            .push((req.filename.to_string(), req.absolute_path.to_string()));
        let name = if req.absolute_path.is_empty() {
            req.filename
        } else {
            req.absolute_path
        };
        DiskImage::from_bytes(name, disk(DOS33)).map_err(|e| e.to_string())
    }
}

fn hash(m: &Machine) -> Vec<u8> {
    serde_json::to_vec(&m.save_state()).unwrap()
}

fn machine_with_image(name: &str) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    let mut card = Disk2Card::new();
    card.insert_disk(0, DiskImage::from_bytes(name, disk(DOS33)).unwrap());
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    m.run_cycles(400_000); // mid-boot: the Disk II is spinning
    m
}

#[test]
fn save_restart_load_step_gives_the_same_state_hash() {
    for (i, name) in [
        "plain.dsk",
        r"C:\Users\Zoë Müller\Games we Play #1\día 日本.dsk",
        r"\\srv\share\disk.dsk",
        r"C:\a\\b\c d.dsk",
        "/home/usuário/with space/and#hash.dsk",
    ]
    .iter()
    .enumerate()
    {
        let mut original = machine_with_image(name);
        let text = save_aws_yaml(&original, &SaveOptions::default())
            .unwrap()
            .text;
        // "restart": a brand new machine from the file alone.
        let mut host = AnyImage::default();
        let mut restored = load_aws_yaml(&text, &mut host).unwrap().machine;
        assert_eq!(
            hash(&original),
            hash(&restored),
            "{name}: logo após carregar"
        );
        original.run_cycles(120_000);
        restored.run_cycles(120_000);
        assert_eq!(
            hash(&original),
            hash(&restored),
            "{name}: após 120k ciclos (#{i})"
        );
        // The path survives: the host was asked for what the file holds.
        assert!(
            host.asked
                .iter()
                .any(|(f, a)| f.contains(name) || a == name || name.ends_with(f.as_str())),
            "{name}: pedido {:?}",
            host.asked
        );
        // Saving the restored machine again is stable (no backslash growth, GH#1510).
        let again = save_aws_yaml(&restored, &SaveOptions::default())
            .unwrap()
            .text;
        let mut h2 = AnyImage::default();
        let twice = load_aws_yaml(&again, &mut h2).unwrap().machine;
        assert_eq!(hash(&restored), hash(&twice), "{name}: segunda ida e volta");
        assert_eq!(
            host.asked, h2.asked,
            "{name}: caminho muda a cada ida e volta"
        );
    }
}

#[test]
fn extreme_counters_survive_save_load_and_step_deterministically() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.run_cycles(50_000);
    let mut st: MachineState = m.save_state();
    st.cycles = u64::MAX - 5_000;
    st.cpu.cycles = st.cycles;
    st.frames = u64::MAX - 1;
    st.cycles_in_frame = u32::MAX;
    st.opcode_ticks = u32::MAX;
    st.pending = u32::MAX;
    let mut a = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    a.load_state(&st).unwrap();
    let mut b = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    b.load_state(&st).unwrap();
    a.run_cycles(60_000);
    b.run_cycles(60_000);
    assert_eq!(hash(&a), hash(&b));
    // Through the AppleWin file as well.
    let text = save_aws_yaml(&a, &SaveOptions::default()).unwrap().text;
    let c = load_aws_yaml(&text, &mut AnyImage::default())
        .unwrap()
        .machine;
    assert_eq!(hash(&a), hash(&c));
}
