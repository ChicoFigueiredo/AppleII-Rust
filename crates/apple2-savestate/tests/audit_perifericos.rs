//! P23 (gate G0): #810 (cartão SAM/DAC no arquivo de estado) e #1080 (Mockingboard e mouse ao
//! mesmo tempo). Oráculo: AppleWin a8f0d1d (`SAMCard::SaveSnapshot`, SAM.cpp:94-109).
//! Resultados em `docs/conformance/results/p23-perifericos.csv`.

use apple2_audio::{AudioBus, MockingboardCard, SamCard, Variant};
use apple2_cards::MouseCard;
use apple2_core::{Card, Machine, MachineConfig, Model, VideoStandard};
use apple2_savestate::{
    default_card, load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions,
};
use apple2_storage::image::DiskImage;

struct Host {
    bus: AudioBus,
}

impl LoadHost for Host {
    fn open_image(&mut self, _req: &ImageRequest) -> Result<DiskImage, String> {
        Err("sem imagens".into())
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        match name {
            "SAM" => Some(Box::new(SamCard::new(&self.bus))),
            "Phasor" => Some(Box::new(MockingboardCard::new(Variant::Phasor, &self.bus))),
            _ => default_card(name, slot, model),
        }
    }
}

/// #810: o cartão SAM/DAC não tinha suporte a arquivo de estado ("it cannot even be stored to a
/// snapshot file"). O AppleWin grava `SAM: ... State: null` (nenhum estado) e relê sem erro; o
/// port grava a entrada do slot e a recarrega no mesmo slot.
#[test]
fn gh810_sam_card_is_saved_and_loaded_in_the_state_file() {
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(5, Box::new(SamCard::new(&bus))));
    let saved = save_aws_yaml(&m, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    assert!(
        saved.text.contains("SAM") && saved.text.contains("State: null"),
        "entrada do cartao SAM sem estado (SAM.cpp:101)"
    );
    let loaded = load_aws_yaml(&saved.text, &mut Host { bus: bus.clone() }).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let again = save_aws_yaml(&loaded.machine, &SaveOptions::default()).unwrap();
    assert!(
        again.text.contains("5: SAM") && again.text.contains("State: null"),
        "o cartao SAM voltou ao slot 5"
    );
}

/// #1080: Phasor (slot 4) e mouse (slot 5) convivem na mesma máquina e no arquivo de estado; o
/// AppleWin 1.30.x aceita qualquer cartão em qualquer slot (nota de tomcw na issue).
#[test]
fn gh1080_phasor_and_mouse_coexist_and_survive_a_state_round_trip() {
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(4, Box::new(MockingboardCard::new(Variant::Phasor, &bus))));
    assert!(m.insert_card(5, Box::new(MouseCard::new())));
    let saved = save_aws_yaml(&m, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    let loaded = load_aws_yaml(&saved.text, &mut Host { bus }).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let m2 = loaded.machine;
    assert!(m2.card::<MockingboardCard>(4).is_some());
    assert!(m2.card::<MouseCard>(5).is_some());
}
