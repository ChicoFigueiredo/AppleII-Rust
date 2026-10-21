//! Pure logic behind the Settings dialog: which cards are offered, how form values map to
//! [`Settings`], what needs a restart and what is invalid. No egui in here, so it is all tested.
//!
//! Sources: AppleWin `CardManager::GetCardChoicesForSlot` / `GetCardChoicesForAuxSlot`
//! (CardManager.cpp:367-516), `CPageSlots::CardTypeHasOptions`, `CPageInput::InitJoystickChoices`
//! and `CPropertySheetHelper::HardwareConfigChanged` (PropertySheetHelper.cpp:497-560).

use apple2_config::card::{HdcFirmware, SlotCardSpec};
use apple2_config::settings::{Joystick0Kind, Joystick1Kind};
use apple2_config::{CardType, Settings};
use apple2_core::Model;

pub const VOLUME_MAX: u32 = 59;

// ---------------------------------------------------------------------------------------------
// Model / clone

/// The "Model:" drop-down of the Configuration page (PageConfig.cpp:49).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelChoice {
    Apple2,
    Apple2Plus,
    Apple2JPlus,
    Apple2e,
    Apple2eEnhanced,
    Clone,
}

impl ModelChoice {
    pub const ALL: [ModelChoice; 6] = [
        ModelChoice::Apple2,
        ModelChoice::Apple2Plus,
        ModelChoice::Apple2JPlus,
        ModelChoice::Apple2e,
        ModelChoice::Apple2eEnhanced,
        ModelChoice::Clone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ModelChoice::Apple2 => "Apple II (original)",
            ModelChoice::Apple2Plus => "Apple II Plus",
            ModelChoice::Apple2JPlus => "Apple II J-Plus",
            ModelChoice::Apple2e => "Apple //e (original)",
            ModelChoice::Apple2eEnhanced => "Apple //e (enhanced)",
            ModelChoice::Clone => "Clone",
        }
    }

    pub fn of(m: Model) -> ModelChoice {
        match m {
            Model::Apple2 => ModelChoice::Apple2,
            Model::Apple2Plus => ModelChoice::Apple2Plus,
            Model::Apple2JPlus => ModelChoice::Apple2JPlus,
            Model::Apple2e => ModelChoice::Apple2e,
            Model::Apple2eEnhanced | Model::Apple2c => ModelChoice::Apple2eEnhanced,
            _ => ModelChoice::Clone,
        }
    }
}

/// The "Clone:" drop-down of the Advanced page (PageAdvanced.cpp:41).
pub const CLONES: [(Model, &str); 5] = [
    (Model::Pravets82, "Pravets 82"),
    (Model::Pravets8M, "Pravets 8M"),
    (Model::Pravets8A, "Pravets 8A"),
    (Model::Tk3000e, "TK3000 //e"),
    (Model::Base64A, "Base 64A"),
];

/// A model whose family matches `choice`; for [`ModelChoice::Clone`] keeps `current` when it is a
/// clone already (AppleWin keeps the previously chosen clone), else Pravets 82.
pub fn model_for_choice(choice: ModelChoice, current: Model) -> Model {
    match choice {
        ModelChoice::Apple2 => Model::Apple2,
        ModelChoice::Apple2Plus => Model::Apple2Plus,
        ModelChoice::Apple2JPlus => Model::Apple2JPlus,
        ModelChoice::Apple2e => Model::Apple2e,
        ModelChoice::Apple2eEnhanced => Model::Apple2eEnhanced,
        ModelChoice::Clone => {
            if CLONES.iter().any(|(m, _)| *m == current) {
                current
            } else {
                Model::Pravets82
            }
        }
    }
}

/// Switches the model and fixes everything that depends on it, as AppleWin does when the
/// Configuration page changes the computer: slot 0 and the aux slot return to the model's default,
/// and the Freeze's F8 ROM only exists for the ][ / ][+.
pub fn set_model(s: &mut Settings, model: Model) {
    if s.machine.model == model {
        return;
    }
    let was_e = !s.machine.model.is_apple2();
    s.machine.model = model;
    if was_e != !model.is_apple2() {
        s.reset_slot0_for_model();
    }
    if !matches!(model, Model::Apple2 | Model::Apple2Plus) {
        s.machine.freeze_f8_rom = false;
    }
    normalize(s);
}

// ---------------------------------------------------------------------------------------------
// Cards per slot

/// Name shown in the slot drop-downs (AppleWin `Card::GetCardName`).
pub fn card_name(ty: CardType) -> &'static str {
    use CardType::*;
    match ty {
        Empty => "Empty",
        Disk2 => "Disk II",
        Ssc => "Super Serial Card",
        MockingboardC => "Mockingboard C",
        GenericPrinter => "Printer",
        GenericHdd => "Hard Disk Controller",
        GenericClock => "Generic Clock",
        MouseInterface => "Mouse Interface",
        Z80 => "Z80 SoftCard",
        Phasor => "Phasor",
        Echo => "Echo II",
        Sam => "SAM",
        Col80 => "80 Column",
        Extended80Col => "Extended 80 Column",
        RamWorksIII => "RamWorks III",
        Uthernet => "Uthernet",
        LanguageCard => "Language Card",
        LanguageCardIIe => "Language Card (//e)",
        Saturn128K => "Saturn 128K",
        FourPlay => "4Play",
        SnesMax => "SNES MAX",
        VidHd => "VidHD",
        Uthernet2 => "Uthernet II",
        MegaAudio => "MegaAudio",
        SdMusic => "SD Music",
        BreakpointCard => "Breakpoint card",
    }
}

/// `CardManager::IsSingleInstanceCard` (CardManager.cpp:367).
pub fn is_single_instance(ty: CardType) -> bool {
    use CardType::*;
    matches!(
        ty,
        MouseInterface
            | Ssc
            | GenericPrinter
            | Z80
            | FourPlay
            | SnesMax
            | Uthernet
            | Uthernet2
            | VidHd
    )
}

/// `CPageSlots::CardTypeHasOptions`: the "..." button next to the slot is enabled.
pub fn has_options(ty: CardType) -> bool {
    use CardType::*;
    matches!(
        ty,
        Disk2
            | GenericHdd
            | Ssc
            | GenericPrinter
            | MockingboardC
            | MouseInterface
            | Phasor
            | Saturn128K
            | Uthernet
            | Uthernet2
            | RamWorksIII
    )
}

/// Cards for slot 0 on a ][ / ][+ (`cardsOnlyInSlot0`).
const SLOT0_CARDS: [CardType; 3] = [
    CardType::Empty,
    CardType::LanguageCard,
    CardType::Saturn128K,
];

/// Cards for slots 1-7 in drop-down order (`cardsInSlots1to7`).
const SLOT1_7_CARDS: [CardType; 16] = {
    use CardType::*;
    [
        Empty,
        Disk2,
        GenericHdd,
        GenericPrinter,
        MouseInterface,
        Saturn128K,
        Ssc,
        FourPlay,
        SnesMax,
        MockingboardC,
        Phasor,
        Sam,
        Uthernet,
        Uthernet2,
        VidHd,
        Z80,
    ]
};

/// Cards offered in `slot` given the cards currently chosen in all slots (None when the slot has
/// no drop-down: slot 0 of a //e family machine). Single-instance cards already placed elsewhere
/// are hidden, Uthernet and Uthernet II exclude each other, and a card outside the list (from the
/// command line) stays selectable in the slot that holds it.
pub fn slot_choices(slot: usize, model: Model, current: &[CardType; 8]) -> Option<Vec<CardType>> {
    if slot == 0 {
        if !model.is_apple2() {
            return None;
        }
        let mut v = SLOT0_CARDS.to_vec();
        if !v.contains(&current[0]) {
            v.push(current[0]);
        }
        return Some(v);
    }
    let elsewhere = |ty: CardType| (1..8).any(|i| i != slot && current[i] == ty);
    let mut v: Vec<CardType> = SLOT1_7_CARDS
        .iter()
        .copied()
        .filter(|&ty| {
            if ty == CardType::Uthernet && elsewhere(CardType::Uthernet2)
                || ty == CardType::Uthernet2 && elsewhere(CardType::Uthernet)
            {
                return false;
            }
            !(is_single_instance(ty) && elsewhere(ty))
        })
        .collect();
    // "advanced cards": whatever the configuration already holds in slots 1-7 and is not listed
    for &c in &current[1..] {
        if !SLOT1_7_CARDS.contains(&c) && !v.contains(&c) {
            v.push(c);
        }
    }
    Some(v)
}

/// `GetCardChoicesForAuxSlot`; only offered on a //e.
pub fn aux_choices(model: Model) -> Option<Vec<CardType>> {
    (!model.is_apple2()).then(|| {
        vec![
            CardType::Empty,
            CardType::Col80,
            CardType::Extended80Col,
            CardType::RamWorksIII,
        ]
    })
}

pub fn current_types(s: &Settings) -> [CardType; 8] {
    std::array::from_fn(|i| s.slots.0[i].card_type())
}

/// Puts `ty` in `slot` (keeping the options when the type does not change).
pub fn set_slot_card(s: &mut Settings, slot: usize, ty: CardType) {
    if s.slots.0[slot].card_type() != ty {
        s.slots.0[slot] = SlotCardSpec::new(ty);
    }
}

pub fn set_aux_card(s: &mut Settings, ty: CardType) {
    if s.aux.card.card_type() != ty {
        s.aux.card = SlotCardSpec::new(ty);
    }
}

/// "Reset to Default" of the Slots page: new-install cards for the current model.
pub fn reset_slots_to_default(s: &mut Settings) {
    let m = s.machine.model;
    for i in 0..8 {
        s.slots.0[i] = SlotCardSpec::default_for_slot(i, m);
    }
    s.aux.card = if m.is_apple2() {
        SlotCardSpec::Empty
    } else {
        SlotCardSpec::Extended80Col
    };
    normalize(s);
}

/// Repairs combinations the dialog cannot produce but a loaded file or command line can.
pub fn normalize(s: &mut Settings) {
    let model = s.machine.model;
    let types = current_types(s);
    if slot_choices(0, model, &types).is_none()
        && !matches!(types[0], CardType::LanguageCardIIe | CardType::Empty)
    {
        // //e family: slot 0 holds the //e language card
        s.slots.0[0] = SlotCardSpec::default_for_slot(0, model);
    }
    // a mouse card takes over the mouse; the joystick cannot use it too (InitJoystickChoices)
    if has_mouse_card(s) {
        if s.joystick.joystick0 == Joystick0Kind::Mouse {
            s.joystick.joystick0 = Joystick0Kind::Disabled;
        }
        if s.joystick.joystick1 == Joystick1Kind::Mouse {
            s.joystick.joystick1 = Joystick1Kind::Disabled;
        }
    }
    s.machine.speed = s.machine.speed.min(40);
}

pub fn has_mouse_card(s: &Settings) -> bool {
    s.slots
        .0
        .iter()
        .any(|c| c.card_type() == CardType::MouseInterface)
}

// ---------------------------------------------------------------------------------------------
// Joysticks

pub fn joystick0_label(k: Joystick0Kind) -> &'static str {
    match k {
        Joystick0Kind::Disabled => "Disabled",
        Joystick0Kind::HostJoystick1 => "PC Joystick #1",
        Joystick0Kind::KeyboardCursors => "Keyboard (cursors)",
        Joystick0Kind::KeyboardNumpad => "Keyboard (numpad)",
        Joystick0Kind::Mouse => "Mouse",
    }
}

pub fn joystick1_label(k: Joystick1Kind) -> &'static str {
    match k {
        Joystick1Kind::Disabled => "Disabled",
        Joystick1Kind::HostJoystick2 => "PC Joystick #2",
        Joystick1Kind::KeyboardCursors => "Keyboard (cursors)",
        Joystick1Kind::KeyboardNumpad => "Keyboard (numpad)",
        Joystick1Kind::Mouse => "Mouse",
        Joystick1Kind::HostJoystick1Thumbstick2 => "PC Joystick #1 Thumbstick 2",
    }
}

fn is_kbd0(k: Joystick0Kind) -> bool {
    matches!(
        k,
        Joystick0Kind::KeyboardCursors | Joystick0Kind::KeyboardNumpad
    )
}
fn is_kbd1(k: Joystick1Kind) -> bool {
    matches!(
        k,
        Joystick1Kind::KeyboardCursors | Joystick1Kind::KeyboardNumpad
    )
}

/// Joystick 1 choices: the other port's keyboard/mouse mode and the mouse (when a mouse card is
/// present) are removed (`InitJoystickChoices`, PageInput.cpp:272-316).
pub fn joystick0_choices(other: Joystick1Kind, mouse_card: bool) -> Vec<Joystick0Kind> {
    use Joystick0Kind::*;
    [
        Disabled,
        HostJoystick1,
        KeyboardCursors,
        KeyboardNumpad,
        Mouse,
    ]
    .into_iter()
    .filter(|&k| {
        if is_kbd0(k) && is_kbd1(other) {
            return false;
        }
        if k == Mouse && (mouse_card || other == Joystick1Kind::Mouse) {
            return false;
        }
        true
    })
    .collect()
}

pub fn joystick1_choices(other: Joystick0Kind, mouse_card: bool) -> Vec<Joystick1Kind> {
    use Joystick1Kind::*;
    [
        Disabled,
        HostJoystick2,
        KeyboardCursors,
        KeyboardNumpad,
        Mouse,
        HostJoystick1Thumbstick2,
    ]
    .into_iter()
    .filter(|&k| {
        if is_kbd1(k) && is_kbd0(other) {
            return false;
        }
        if k == Mouse && (mouse_card || other == Joystick0Kind::Mouse) {
            return false;
        }
        true
    })
    .collect()
}

/// "Allow cursor keys to be read from keyboard" is only enabled with a keyboard joystick on the
/// cursors; "Keyboard auto-centering" with any keyboard joystick (PageInput.cpp:179-180).
pub fn cursor_control_enabled(s: &Settings) -> bool {
    s.joystick.joystick0 == Joystick0Kind::KeyboardCursors
        || s.joystick.joystick1 == Joystick1Kind::KeyboardCursors
}
pub fn centering_enabled(s: &Settings) -> bool {
    is_kbd0(s.joystick.joystick0) || is_kbd1(s.joystick.joystick1)
}

// ---------------------------------------------------------------------------------------------
// Form values that are not 1:1 with a Settings field

/// Configuration page speed block: "Use Authentic Machine Speed" / "Select Custom Speed" + slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeedForm {
    pub authentic: bool,
    /// Slider value in tenths of MHz-relative speed (5 = 0.5, 10 = 1.0, 40 = "Fastest").
    pub custom: u32,
}

pub const SPEED_MIN: u32 = 5;
pub const SPEED_NORMAL: u32 = 10;
pub const SPEED_MAX: u32 = 40;

impl SpeedForm {
    pub fn from_settings(s: &Settings) -> Self {
        let tenths = match s.machine.clock_multiplier {
            Some(m) => (m * 10.0).round().clamp(SPEED_MIN as f64, SPEED_MAX as f64) as u32,
            None => s.machine.speed.clamp(SPEED_MIN, SPEED_MAX),
        };
        SpeedForm {
            authentic: s.machine.clock_multiplier.is_none() && s.machine.speed == SPEED_NORMAL,
            custom: tenths,
        }
    }

    pub fn apply(&self, s: &mut Settings) {
        s.machine.clock_multiplier = None;
        s.machine.speed = if self.authentic {
            SPEED_NORMAL
        } else {
            self.custom.clamp(SPEED_MIN, SPEED_MAX)
        };
    }
}

pub fn speed_label(tenths: u32) -> String {
    if tenths >= SPEED_MAX {
        "Fastest".into()
    } else {
        format!("{:.1} MHz", tenths as f32 / 10.0)
    }
}

/// The volume slider shows loudness (right = louder); settings store attenuation (0 = loudest).
pub fn volume_to_slider(master_volume: u32) -> u32 {
    VOLUME_MAX - master_volume.min(VOLUME_MAX)
}
pub fn slider_to_volume(slider: u32) -> u32 {
    VOLUME_MAX - slider.min(VOLUME_MAX)
}

/// Autofire is one bit per button in the settings; the dialog has one check box for all three.
pub fn autofire_all(bits: u8) -> bool {
    bits & 7 == 7
}
pub fn set_autofire_all(s: &mut Settings, on: bool) {
    s.joystick.autofire = if on { 7 } else { 0 };
}

/// 0xRRGGBB <-> egui colour triple.
pub fn rgb_to_array(rgb: u32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}
pub fn array_to_rgb(c: [u8; 3]) -> u32 {
    (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2])
}

/// SSC options dialog: serial port or TCP, as one form value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SscForm {
    pub tcp: bool,
    pub serial_port: String,
    pub tcp_port: u16,
    pub support_dcd: bool,
}

/// `-ssc-tcp-port` default (AppleWin's "Telnet-like" port).
pub const SSC_TCP_PORT: u16 = 1977;

impl SscForm {
    pub fn from_spec(spec: &SlotCardSpec) -> Option<Self> {
        match spec {
            SlotCardSpec::Ssc {
                serial_port,
                tcp_port,
                support_dcd,
            } => Some(SscForm {
                tcp: tcp_port.is_some(),
                serial_port: serial_port.clone(),
                tcp_port: tcp_port.unwrap_or(SSC_TCP_PORT),
                support_dcd: *support_dcd,
            }),
            _ => None,
        }
    }

    pub fn to_spec(&self) -> SlotCardSpec {
        SlotCardSpec::Ssc {
            serial_port: if self.tcp {
                String::new()
            } else {
                self.serial_port.clone()
            },
            tcp_port: self.tcp.then_some(self.tcp_port),
            support_dcd: self.support_dcd,
        }
    }
}

/// The warning the SSC dialog shows in TCP mode (docs/pendencias.md).
pub const SSC_TCP_WARNING: &str = "Warning: TCP mode listens on ALL network interfaces, port 1977 by default, with no authentication. \
Anyone who can reach this machine can connect to the emulated serial port.";

/// RamWorks III size slider: banks of 64K, 1..=256 (1 MB .. 16 MB in AppleWin's UI is 16..256).
pub const RW3_BANK_STEPS: [u16; 9] = [1, 2, 4, 8, 16, 32, 64, 128, 256];
/// Saturn: 16K banks, 16K to 128K.
pub const SATURN_BANK_STEPS: [u8; 4] = [1, 2, 4, 8];

// ---------------------------------------------------------------------------------------------
// Restart detection and validation

/// Why a restart is needed, in AppleWin's wording (`HardwareConfigChanged`). Empty = no restart.
pub fn restart_reasons(old: &Settings, new: &Settings) -> Vec<String> {
    let mut v = Vec::new();
    if old.machine.model != new.machine.model {
        v.push("Emulated computer has changed".to_string());
    }
    if old.machine.cpu != new.machine.cpu {
        v.push("Emulated main CPU has changed".into());
    }
    if old.machine.refresh_rate != new.machine.refresh_rate {
        v.push("Video refresh rate has changed".into());
    }
    for i in 0..8 {
        let (a, b) = (&old.slots.0[i], &new.slots.0[i]);
        if a.card_type() != b.card_type() {
            v.push(format!(
                "Slot {i}: {} -> {}",
                card_name(a.card_type()),
                card_name(b.card_type())
            ));
        } else if a != b {
            v.push(format!(
                "Slot {i}: {} configuration has changed",
                card_name(b.card_type())
            ));
        }
    }
    if old.aux.card.card_type() != new.aux.card.card_type() {
        v.push(format!(
            "Slot Aux: {} -> {}",
            card_name(old.aux.card.card_type()),
            card_name(new.aux.card.card_type())
        ));
    } else if old.aux.card != new.aux.card {
        v.push("RamWorks III memory size changed".into());
    }
    if old.machine.freeze_f8_rom != new.machine.freeze_f8_rom {
        v.push("F8 ROM changed (The Freeze's F8 Rom)".into());
    }
    if old.machine.no_slot_clock != new.machine.no_slot_clock {
        v.push("No-Slot clock has changed".into());
    }
    if old.machine.custom_rom != new.machine.custom_rom
        || old.machine.custom_f8_rom != new.machine.custom_f8_rom
        || old.machine.video_rom != new.machine.video_rom
    {
        v.push("Custom ROM has changed".into());
    }
    v
}

pub fn needs_restart(old: &Settings, new: &Settings) -> bool {
    !restart_reasons(old, new).is_empty()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub message: String,
}

fn bad(v: &mut Vec<Problem>, m: impl Into<String>) {
    v.push(Problem { message: m.into() });
}

/// Errors that must block OK/Apply.
pub fn validate(s: &Settings) -> Vec<Problem> {
    let mut v = Vec::new();
    let model = s.machine.model;
    if model == Model::Apple2c {
        bad(
            &mut v,
            "The Apple //c has no ROM here and cannot be emulated",
        );
    }
    let types = current_types(s);
    for slot in 0..8 {
        let ty = types[slot];
        if slot == 0 && !model.is_apple2() {
            if !matches!(ty, CardType::LanguageCardIIe | CardType::Empty) {
                bad(
                    &mut v,
                    format!("Slot 0: {} is not allowed on this model", card_name(ty)),
                );
            }
            continue;
        }
        if slot_choices(slot, model, &types).is_some_and(|c| !c.contains(&ty)) {
            bad(
                &mut v,
                format!("Slot {slot}: {} is not allowed here", card_name(ty)),
            );
        }
        if slot > 0 && is_single_instance(ty) && (1..8).any(|j| j != slot && types[j] == ty) {
            bad(
                &mut v,
                format!("{} can be plugged in only one slot", card_name(ty)),
            );
        }
        match &s.slots.0[slot] {
            SlotCardSpec::Ssc {
                tcp_port: Some(0), ..
            } => bad(&mut v, format!("Slot {slot}: SSC TCP port must be 1-65535")),
            SlotCardSpec::RamWorksIII { banks } if !(1..=256).contains(banks) => {
                bad(&mut v, "RamWorks III size must be 1-256 banks")
            }
            SlotCardSpec::Saturn128K { banks } if !(1..=8).contains(banks) => bad(
                &mut v,
                format!("Slot {slot}: Saturn size must be 1-8 banks"),
            ),
            SlotCardSpec::GenericPrinter(p) if p.idle_limit == 0 => bad(
                &mut v,
                format!("Slot {slot}: printer idle time must be at least 1 second"),
            ),
            _ => {}
        }
    }
    if (types.contains(&CardType::Uthernet)) && types.contains(&CardType::Uthernet2) {
        bad(&mut v, "Uthernet and Uthernet II cannot both be plugged in");
    }
    if let SlotCardSpec::RamWorksIII { banks } = &s.aux.card {
        if !(1..=256).contains(banks) {
            bad(&mut v, "RamWorks III size must be 1-256 banks");
        }
    }
    if !matches!(
        s.aux.card.card_type(),
        CardType::Empty | CardType::Col80 | CardType::Extended80Col | CardType::RamWorksIII
    ) {
        bad(&mut v, "Aux slot: card not allowed");
    }
    if s.machine.freeze_f8_rom && !matches!(model, Model::Apple2 | Model::Apple2Plus) {
        bad(
            &mut v,
            "The Freeze's F8 ROM is only for the Apple ][ and ][+",
        );
    }
    if s.joystick.joystick0 == Joystick0Kind::Mouse && has_mouse_card(s) {
        bad(
            &mut v,
            "Joystick 1 cannot use the mouse while a Mouse Interface card is plugged in",
        );
    }
    if s.joystick.joystick1 == Joystick1Kind::Mouse && has_mouse_card(s) {
        bad(
            &mut v,
            "Joystick 2 cannot use the mouse while a Mouse Interface card is plugged in",
        );
    }
    if is_kbd0(s.joystick.joystick0) && is_kbd1(s.joystick.joystick1) {
        bad(&mut v, "Both joysticks cannot use the keyboard");
    }
    if s.machine.speed > SPEED_MAX {
        bad(&mut v, "Speed out of range");
    }
    if s.video.window_scale == 0 || s.video.window_scale > 8 {
        bad(&mut v, "Window scale must be 1-8");
    }
    v
}

/// Non-blocking remarks (missing files).
pub fn warnings(s: &Settings) -> Vec<String> {
    let mut v = Vec::new();
    let missing = |p: &std::path::Path| !p.as_os_str().is_empty() && !p.exists();
    for (slot, c) in s.slots.0.iter().enumerate() {
        match c {
            SlotCardSpec::Disk2 { drives, .. } => {
                for (d, spec) in drives.iter().enumerate() {
                    if spec.image.as_deref().is_some_and(missing) {
                        v.push(format!("Slot {slot} drive {}: image file not found", d + 1));
                    }
                }
            }
            SlotCardSpec::GenericHdd { images, .. } => {
                for (d, img) in images.iter().enumerate() {
                    if img.as_deref().is_some_and(missing) {
                        v.push(format!("Slot {slot} HDD {}: image file not found", d + 1));
                    }
                }
            }
            _ => {}
        }
    }
    for (name, p) in [
        ("Custom ROM", &s.machine.custom_rom),
        ("Custom F8 ROM", &s.machine.custom_f8_rom),
    ] {
        if p.as_deref().is_some_and(missing) {
            v.push(format!("{name}: file not found"));
        }
    }
    v
}

pub fn hdc_firmware_label(f: HdcFirmware) -> &'static str {
    match f {
        HdcFirmware::Default => "Default",
        HdcFirmware::SmartPort => "SmartPort",
        HdcFirmware::BlockMode2Devices => "Block mode (2 devices)",
        HdcFirmware::BlockMode4Devices => "Block mode (4 devices)",
    }
}

/// HDD 1 <-> HDD 2 ("Swap 1 and 2").
pub fn swap_hdd_1_2(s: &mut Settings, slot: usize) {
    if let SlotCardSpec::GenericHdd { images, .. } = &mut s.slots.0[slot] {
        images.swap(0, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_config::card::{PrinterSpec, SoundSockets};
    use apple2_config::settings::RefreshRate;

    fn base() -> Settings {
        Settings::default()
    }

    #[test]
    fn model_combo_matches_applewin_list() {
        let labels: Vec<_> = ModelChoice::ALL.iter().map(|c| c.label()).collect();
        assert_eq!(
            labels,
            [
                "Apple II (original)",
                "Apple II Plus",
                "Apple II J-Plus",
                "Apple //e (original)",
                "Apple //e (enhanced)",
                "Clone"
            ]
        );
        for m in [
            Model::Pravets82,
            Model::Pravets8M,
            Model::Pravets8A,
            Model::Tk3000e,
            Model::Base64A,
        ] {
            assert_eq!(ModelChoice::of(m), ModelChoice::Clone);
        }
        assert_eq!(
            CLONES.iter().map(|c| c.1).collect::<Vec<_>>(),
            [
                "Pravets 82",
                "Pravets 8M",
                "Pravets 8A",
                "TK3000 //e",
                "Base 64A"
            ]
        );
    }

    #[test]
    fn clone_choice_keeps_previous_clone() {
        assert_eq!(
            model_for_choice(ModelChoice::Clone, Model::Tk3000e),
            Model::Tk3000e
        );
        assert_eq!(
            model_for_choice(ModelChoice::Clone, Model::Apple2e),
            Model::Pravets82
        );
        assert_eq!(
            model_for_choice(ModelChoice::Apple2, Model::Tk3000e),
            Model::Apple2
        );
    }

    #[test]
    fn slot0_choices_by_model() {
        let mut t = current_types(&base());
        t[0] = CardType::LanguageCard;
        assert_eq!(
            slot_choices(0, Model::Apple2Plus, &t).unwrap(),
            [
                CardType::Empty,
                CardType::LanguageCard,
                CardType::Saturn128K
            ]
        );
        assert!(slot_choices(0, Model::Apple2eEnhanced, &t).is_none());
    }

    #[test]
    fn slots_1_to_7_follow_card_manager_order() {
        use CardType::*;
        let t = [Empty; 8];
        assert_eq!(
            slot_choices(3, Model::Apple2Plus, &t).unwrap(),
            [
                Empty,
                Disk2,
                GenericHdd,
                GenericPrinter,
                MouseInterface,
                Saturn128K,
                Ssc,
                FourPlay,
                SnesMax,
                MockingboardC,
                Phasor,
                Sam,
                Uthernet,
                Uthernet2,
                VidHd,
                Z80
            ]
        );
    }

    #[test]
    fn single_instance_cards_hidden_in_other_slots_but_kept_in_own() {
        use CardType::*;
        let mut t = [Empty; 8];
        t[2] = Ssc;
        t[5] = MouseInterface;
        let c3 = slot_choices(3, Model::Apple2e, &t).unwrap();
        assert!(!c3.contains(&Ssc) && !c3.contains(&MouseInterface));
        assert!(
            c3.contains(&Disk2) && c3.contains(&Phasor),
            "multi-instance cards stay"
        );
        assert!(slot_choices(2, Model::Apple2e, &t).unwrap().contains(&Ssc));
    }

    #[test]
    fn uthernet_and_uthernet2_exclude_each_other() {
        use CardType::*;
        let mut t = [Empty; 8];
        t[4] = Uthernet;
        let c = slot_choices(5, Model::Apple2e, &t).unwrap();
        assert!(!c.contains(&Uthernet) && !c.contains(&Uthernet2));
        assert!(slot_choices(4, Model::Apple2e, &t)
            .unwrap()
            .contains(&Uthernet));
        assert!(slot_choices(4, Model::Apple2e, &t)
            .unwrap()
            .contains(&Uthernet2));
    }

    #[test]
    fn advanced_cards_stay_selectable_where_they_are() {
        let mut t = [CardType::Empty; 8];
        t[3] = CardType::MegaAudio;
        assert!(slot_choices(3, Model::Apple2e, &t)
            .unwrap()
            .contains(&CardType::MegaAudio));
        assert!(!slot_choices(4, Model::Apple2e, &[CardType::Empty; 8])
            .unwrap()
            .contains(&CardType::MegaAudio));
    }

    #[test]
    fn aux_choices_only_on_the_e() {
        assert!(aux_choices(Model::Apple2Plus).is_none());
        assert_eq!(
            aux_choices(Model::Apple2e).unwrap(),
            [
                CardType::Empty,
                CardType::Col80,
                CardType::Extended80Col,
                CardType::RamWorksIII
            ]
        );
    }

    #[test]
    fn options_button_cards() {
        use CardType::*;
        for ty in [
            Disk2,
            GenericHdd,
            Ssc,
            GenericPrinter,
            MockingboardC,
            MouseInterface,
            Phasor,
            Saturn128K,
            Uthernet,
            Uthernet2,
            RamWorksIII,
        ] {
            assert!(has_options(ty), "{ty:?}");
        }
        for ty in [
            Empty,
            Sam,
            Z80,
            VidHd,
            FourPlay,
            SnesMax,
            LanguageCard,
            Extended80Col,
        ] {
            assert!(!has_options(ty), "{ty:?}");
        }
    }

    #[test]
    fn changing_between_ii_and_iie_resets_slot0() {
        let mut s = base();
        s.machine.model = Model::Apple2Plus;
        s.reset_slot0_for_model();
        assert_eq!(s.slot(0).card_type(), CardType::LanguageCard);
        set_model(&mut s, Model::Apple2e);
        assert_eq!(s.slot(0).card_type(), CardType::LanguageCardIIe);
        set_model(&mut s, Model::Apple2Plus);
        assert_eq!(s.slot(0).card_type(), CardType::LanguageCard);
    }

    #[test]
    fn freeze_f8_cleared_off_apple_ii() {
        let mut s = base();
        set_model(&mut s, Model::Apple2Plus);
        s.machine.freeze_f8_rom = true;
        set_model(&mut s, Model::Apple2e);
        assert!(!s.machine.freeze_f8_rom);
    }

    #[test]
    fn joystick_choices_exclude_other_port() {
        let c = joystick0_choices(Joystick1Kind::KeyboardCursors, false);
        assert!(
            !c.contains(&Joystick0Kind::KeyboardCursors)
                && !c.contains(&Joystick0Kind::KeyboardNumpad)
        );
        assert!(c.contains(&Joystick0Kind::Mouse));
        let c = joystick0_choices(Joystick1Kind::Disabled, true);
        assert!(!c.contains(&Joystick0Kind::Mouse));
        let c = joystick1_choices(Joystick0Kind::Mouse, false);
        assert!(!c.contains(&Joystick1Kind::Mouse));
        assert!(c.contains(&Joystick1Kind::HostJoystick1Thumbstick2));
        assert_eq!(joystick1_choices(Joystick0Kind::Disabled, false).len(), 6);
    }

    #[test]
    fn mouse_card_removes_mouse_joystick() {
        let mut s = base();
        s.joystick.joystick0 = Joystick0Kind::Mouse;
        s.slots.0[4] = SlotCardSpec::new(CardType::MouseInterface);
        normalize(&mut s);
        assert_eq!(s.joystick.joystick0, Joystick0Kind::Disabled);
    }

    #[test]
    fn cursor_and_centering_enablement() {
        let mut s = base();
        s.joystick.joystick0 = Joystick0Kind::HostJoystick1;
        assert!(!cursor_control_enabled(&s) && !centering_enabled(&s));
        s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
        assert!(!cursor_control_enabled(&s) && centering_enabled(&s));
        s.joystick.joystick1 = Joystick1Kind::Disabled;
        s.joystick.joystick0 = Joystick0Kind::KeyboardCursors;
        assert!(cursor_control_enabled(&s));
    }

    #[test]
    fn speed_form_round_trip() {
        let mut s = base();
        assert_eq!(
            SpeedForm::from_settings(&s),
            SpeedForm {
                authentic: true,
                custom: 10
            }
        );
        for tenths in [5, 10, 17, 40] {
            let f = SpeedForm {
                authentic: false,
                custom: tenths,
            };
            f.apply(&mut s);
            assert_eq!(SpeedForm::from_settings(&s).custom, tenths);
        }
        s.machine.speed = 20;
        assert!(!SpeedForm::from_settings(&s).authentic);
        SpeedForm {
            authentic: true,
            custom: 20,
        }
        .apply(&mut s);
        assert_eq!(s.machine.speed, 10);
        s.machine.clock_multiplier = Some(3.0);
        let f = SpeedForm::from_settings(&s);
        assert!(!f.authentic && f.custom == 30);
        f.apply(&mut s);
        assert!(s.machine.clock_multiplier.is_none());
        assert_eq!(speed_label(40), "Fastest");
        assert_eq!(speed_label(5), "0.5 MHz");
    }

    #[test]
    fn volume_and_colour_and_autofire_round_trips() {
        for v in 0..=VOLUME_MAX {
            assert_eq!(slider_to_volume(volume_to_slider(v)), v);
        }
        assert_eq!(volume_to_slider(0), VOLUME_MAX);
        assert_eq!(array_to_rgb(rgb_to_array(0x12AB9F)), 0x12AB9F);
        let mut s = base();
        set_autofire_all(&mut s, true);
        assert!(autofire_all(s.joystick.autofire));
        set_autofire_all(&mut s, false);
        assert!(!autofire_all(s.joystick.autofire));
        assert!(!autofire_all(3));
    }

    #[test]
    fn ssc_form_round_trip() {
        let serial = SlotCardSpec::Ssc {
            serial_port: "/dev/ttyUSB0".into(),
            tcp_port: None,
            support_dcd: true,
        };
        let f = SscForm::from_spec(&serial).unwrap();
        assert!(!f.tcp && f.tcp_port == 1977);
        assert_eq!(f.to_spec(), serial);
        let tcp = SlotCardSpec::Ssc {
            serial_port: String::new(),
            tcp_port: Some(2023),
            support_dcd: false,
        };
        let f = SscForm::from_spec(&tcp).unwrap();
        assert!(f.tcp && f.tcp_port == 2023);
        assert_eq!(f.to_spec(), tcp);
        assert!(SscForm::from_spec(&SlotCardSpec::Empty).is_none());
        assert!(
            SSC_TCP_WARNING.contains("ALL network interfaces") && SSC_TCP_WARNING.contains("1977")
        );
    }

    #[test]
    fn restart_only_for_hardware_changes() {
        let old = base();
        let mut n = old.clone();
        n.video.half_scanlines = !n.video.half_scanlines;
        n.sound.master_volume = 20;
        n.joystick.pdl_x_trim = 3;
        n.machine.speed = 30;
        n.storage.enhance_disk_speed = false;
        n.misc.confirm_reboot = false;
        assert!(!needs_restart(&old, &n), "{:?}", restart_reasons(&old, &n));

        let mut n = old.clone();
        n.machine.model = Model::Apple2Plus;
        assert!(restart_reasons(&old, &n)[0].contains("computer"));
        let mut n = old.clone();
        n.slots.0[5] = SlotCardSpec::new(CardType::Phasor);
        assert!(restart_reasons(&old, &n)
            .iter()
            .any(|r| r.contains("Slot 5")));
        let mut n = old.clone();
        n.machine.refresh_rate = RefreshRate::Hz50;
        assert!(needs_restart(&old, &n));
        let mut n = old.clone();
        n.aux.card = SlotCardSpec::RamWorksIII { banks: 64 };
        assert!(needs_restart(&old, &n));
        let mut n = old.clone();
        n.machine.no_slot_clock = false;
        assert!(needs_restart(&old, &n));
    }

    #[test]
    fn card_option_changes_need_restart() {
        let old = base();
        let mut n = old.clone();
        n.slots.0[2] = SlotCardSpec::Ssc {
            serial_port: "COM3".into(),
            tcp_port: None,
            support_dcd: false,
        };
        assert!(restart_reasons(&old, &n)[0].contains("configuration has changed"));
        let mut n = old.clone();
        n.slots.0[4] = SlotCardSpec::MockingboardC(SoundSockets {
            sc01: false,
            ..Default::default()
        });
        assert!(needs_restart(&old, &n));
    }

    #[test]
    fn default_settings_are_valid() {
        assert!(validate(&base()).is_empty(), "{:?}", validate(&base()));
    }

    #[test]
    fn validation_catches_bad_slots() {
        let mut s = base();
        s.slots.0[3] = SlotCardSpec::new(CardType::Ssc);
        let v = validate(&s);
        assert!(
            v.iter().any(|p| p.message.contains("only one slot")),
            "{v:?}"
        );

        let mut s = base();
        s.slots.0[0] = SlotCardSpec::new(CardType::Disk2);
        assert!(validate(&s).iter().any(|p| p.message.contains("Slot 0")));

        let mut s = base();
        s.slots.0[3] = SlotCardSpec::new(CardType::Uthernet);
        s.slots.0[4] = SlotCardSpec::new(CardType::Uthernet2);
        assert!(validate(&s).iter().any(|p| p.message.contains("Uthernet")));

        let mut s = base();
        s.slots.0[2] = SlotCardSpec::Ssc {
            serial_port: String::new(),
            tcp_port: Some(0),
            support_dcd: false,
        };
        assert!(!validate(&s).is_empty());

        let mut s = base();
        s.aux.card = SlotCardSpec::RamWorksIII { banks: 0 };
        assert!(!validate(&s).is_empty());

        let mut s = base();
        s.slots.0[1] = SlotCardSpec::GenericPrinter(PrinterSpec {
            idle_limit: 0,
            ..Default::default()
        });
        assert!(!validate(&s).is_empty());
    }

    #[test]
    fn validation_catches_joystick_and_f8_conflicts() {
        let mut s = base();
        s.joystick.joystick0 = Joystick0Kind::KeyboardCursors;
        s.joystick.joystick1 = Joystick1Kind::KeyboardNumpad;
        assert!(validate(&s).iter().any(|p| p.message.contains("keyboard")));
        let mut s = base();
        s.machine.freeze_f8_rom = true;
        assert!(!validate(&s).is_empty());
        s.machine.model = Model::Apple2Plus;
        s.reset_slot0_for_model();
        assert!(validate(&s).is_empty(), "{:?}", validate(&s));
    }

    #[test]
    fn reset_slots_gives_new_install_cards() {
        let mut s = base();
        s.slots.0[3] = SlotCardSpec::new(CardType::Phasor);
        s.slots.0[6] = SlotCardSpec::Empty;
        reset_slots_to_default(&mut s);
        let t = current_types(&s);
        assert_eq!(t[6], CardType::Disk2);
        assert_eq!(t[3], CardType::Empty);
        assert_eq!(t[4], CardType::MockingboardC);
        assert_eq!(s.aux.card.card_type(), CardType::Extended80Col);
    }

    #[test]
    fn hdd_swap() {
        let mut s = base();
        s.slots.0[7] = SlotCardSpec::GenericHdd {
            firmware: HdcFirmware::Default,
            images: Default::default(),
        };
        if let SlotCardSpec::GenericHdd { images, .. } = &mut s.slots.0[7] {
            images[0] = Some("a.hdv".into());
        }
        swap_hdd_1_2(&mut s, 7);
        match &s.slots.0[7] {
            SlotCardSpec::GenericHdd { images, .. } => {
                assert_eq!(images[1].as_deref(), Some(std::path::Path::new("a.hdv")))
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn every_card_has_a_name() {
        for v in 0..26 {
            assert!(!card_name(CardType::from_value(v).unwrap()).is_empty());
        }
    }
}
