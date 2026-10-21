//! The complete user-facing settings model.
//!
//! Field names are ours; each field's doc comment gives the AppleWin registry value it maps to
//! (`Software\AppleWin\CurrentVersion\<section>\<value>`, Common.h:63-139). The same table in
//! machine-readable form is [`crate::registry_map::KEYS`].

use crate::card::{CardType, DriveSpec, SlotCardSpec};
use apple2_core::Model;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `eCpuType` (Cpu.h): the main CPU. "CPU Type" (Utilities.cpp:145).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuType {
    /// Follow the model (65C02 only on the Enhanced //e, TK3000 and //c).
    #[default]
    Auto,
    Cpu6502,
    Cpu65C02,
}

/// `VideoType_e` (Video.h:20-30), values as saved in "Video Emulation".
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoType {
    MonoCustom = 0,
    ColorIdealized,
    ColorVideocardRgb,
    ColorMonitorNtsc,
    ColorTv,
    MonoTv,
    MonoAmber,
    MonoGreen,
    MonoWhite,
}

impl VideoType {
    pub fn from_value(v: u32) -> Option<VideoType> {
        use VideoType::*;
        [
            MonoCustom,
            ColorIdealized,
            ColorVideocardRgb,
            ColorMonitorNtsc,
            ColorTv,
            MonoTv,
            MonoAmber,
            MonoGreen,
            MonoWhite,
        ]
        .get(v as usize)
        .copied()
    }
}

/// `VideoRefreshRate_e` (Video.h:46-49). "Video Refresh Rate".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefreshRate {
    Hz50,
    #[default]
    Hz60,
}

/// RGB video card flavour (`RGB_Videocard_e`, CmdLine.cpp:682-698).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RgbCardType {
    #[default]
    Video7Sl7,
    LeChatMauveEve,
    LeChatMauveFeline,
}

/// Joystick 0 device (`JOY0CHOICE`, Joystick.h:6). "Joystick0 Emu Type v3".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Joystick0Kind {
    Disabled = 0,
    HostJoystick1,
    KeyboardCursors,
    KeyboardNumpad,
    Mouse,
}

/// Joystick 1 device (`JOY1CHOICE`, Joystick.h:7). "Joystick1 Emu Type v3".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Joystick1Kind {
    Disabled = 0,
    HostJoystick2,
    KeyboardCursors,
    KeyboardNumpad,
    Mouse,
    HostJoystick1Thumbstick2,
}

impl Joystick0Kind {
    pub fn from_value(v: u32) -> Option<Self> {
        use Joystick0Kind::*;
        [
            Disabled,
            HostJoystick1,
            KeyboardCursors,
            KeyboardNumpad,
            Mouse,
        ]
        .get(v as usize)
        .copied()
    }
}
impl Joystick1Kind {
    pub fn from_value(v: u32) -> Option<Self> {
        use Joystick1Kind::*;
        [
            Disabled,
            HostJoystick2,
            KeyboardCursors,
            KeyboardNumpad,
            Mouse,
            HostJoystick1Thumbstick2,
        ]
        .get(v as usize)
        .copied()
    }
}

/// Copy-protection dongle in the game I/O connector (`DONGLETYPE`, CopyProtectionDongles.h:6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DongleType {
    #[default]
    Empty = 0,
    SdsSpeedStar,
    Codewriter,
    Robocom500,
    Robocom1000,
    Robocom1500,
    HaydenCompiler,
}

impl DongleType {
    pub fn from_value(v: u32) -> Option<Self> {
        use DongleType::*;
        [
            Empty,
            SdsSpeedStar,
            Codewriter,
            Robocom500,
            Robocom1000,
            Robocom1500,
            HaydenCompiler,
        ]
        .get(v as usize)
        .copied()
    }
}

/// Machine section ("Configuration" registry section).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSettings {
    /// "Apple2 Type" (Utilities.cpp:97), values = `eApple2Type`.
    pub model: Model,
    /// "CPU Type".
    pub cpu: CpuType,
    /// "Video Refresh Rate" decides NTSC (60 Hz) vs PAL (50 Hz) timing.
    pub refresh_rate: RefreshRate,
    /// "Emulation Speed", tenths of 1.0 MHz-relative speed: 10 = normal, 0 = min, 40 = unbounded
    /// (`SPEED_NORMAL`/`SPEED_MAX`, Common.h:29-31).
    pub speed: u32,
    /// `-clock-multiplier` (CmdLine.cpp:650), overrides `speed` when set.
    pub clock_multiplier: Option<f64>,
    /// "The Freeze's F8 Rom".
    pub freeze_f8_rom: bool,
    /// "No-Slot clock" (default on, Memory.cpp:2033).
    pub no_slot_clock: bool,
    /// `-rom` custom system ROM (16K or 12K), `-f8rom` (2K), `-videorom`.
    pub custom_rom: Option<PathBuf>,
    pub custom_f8_rom: Option<PathBuf>,
    pub video_rom: Option<PathBuf>,
    /// `-memclear` pattern index (`g_nMemoryClearType`); AppleWin's default is 2 (`MIP_FF_FF_00_00`).
    pub memory_clear_type: u32,
    /// Seed of the "random" power-on RAM bytes (`getRandomTime`). `None`: the host picks one from
    /// the clock when it builds the machine (the core itself never reads the clock, ADR 0001);
    /// `to_machine_config` alone then gives the oracle's deterministic draw.
    pub memory_seed: Option<u32>,
    /// `-bootsector` custom boot sector file.
    pub boot_sector: Option<PathBuf>,
}

impl Default for MachineSettings {
    fn default() -> Self {
        MachineSettings {
            model: Model::Apple2eEnhanced,
            cpu: CpuType::Auto,
            refresh_rate: RefreshRate::Hz60,
            speed: 10,
            clock_multiplier: None,
            freeze_f8_rom: false,
            no_slot_clock: true,
            custom_rom: None,
            custom_f8_rom: None,
            video_rom: None,
            memory_clear_type: 2,
            memory_seed: None,
            boot_sector: None,
        }
    }
}

/// Video and window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// "Video Emulation" (default `VT_COLOR_TV`).
    pub mode: VideoType,
    /// "Video Style" bit 0 `VS_HALF_SCANLINES` (default on).
    pub half_scanlines: bool,
    /// "Video Style" bit 1 `VS_COLOR_VERTICAL_BLEND`.
    pub vertical_blend: bool,
    /// "Monochrome Color", 0xRRGGBB (registry stores COLORREF 0x00BBGGRR; the importer swaps).
    pub monochrome_rgb: u32,
    pub rgb_card: RgbCardType,
    /// `-rgb-card-foreground/-background` (SL7 text colours 0..15).
    pub rgb_card_foreground: u8,
    pub rgb_card_background: u8,
    /// `-rgb-card-invert-bit7`, `-mac-lc-card-dlgr`.
    pub rgb_card_invert_bit7: bool,
    pub mac_lc_card_dlgr: bool,
    /// Not in the registry (cmd line only: `-f`).
    pub fullscreen: bool,
    pub fullscreen_width: Option<u32>,
    pub fullscreen_height: Option<u32>,
    /// `-fs-height=best`.
    pub fullscreen_best_resolution: bool,
    /// "Window Scale" (default 2).
    pub window_scale: u32,
    /// "Show Disk II Status" (windowed LEDs).
    pub show_disk_leds: bool,
    /// "Full-screen show subunit status".
    pub fullscreen_show_subunit_status: bool,
    /// Preferences "Window X-Position"/"Window Y-Position".
    pub window_pos: Option<(i32, i32)>,
    /// `-multimon`.
    pub multi_monitor: bool,
    /// `-printscreen` / `-no-printscreen-dlg` / `-no-printscreen-key`.
    pub show_printscreen_filename: bool,
    pub show_printscreen_warning_dialog: bool,
    pub capture_printscreen_key: bool,
    /// `-alt-enter=toggle-full-screen` (default) or `open-apple-enter`.
    pub alt_enter_toggles_fullscreen: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        VideoSettings {
            mode: VideoType::ColorTv,
            half_scanlines: true,
            vertical_blend: false,
            monochrome_rgb: 0xC0C0C0,
            rgb_card: RgbCardType::Video7Sl7,
            rgb_card_foreground: 15,
            rgb_card_background: 0,
            rgb_card_invert_bit7: false,
            mac_lc_card_dlgr: false,
            fullscreen: false,
            fullscreen_width: None,
            fullscreen_height: None,
            fullscreen_best_resolution: false,
            window_scale: 2,
            show_disk_leds: true,
            fullscreen_show_subunit_status: true,
            window_pos: None,
            multi_monitor: false,
            show_printscreen_filename: false,
            show_printscreen_warning_dialog: true,
            capture_printscreen_key: true,
            alt_enter_toggles_fullscreen: true,
        }
    }
}

/// Sound.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    /// "Master volume": attenuation, 0 = loudest .. 59 = silence (PageConfig.cpp:326).
    /// Also covers the older "Speaker Volume".
    pub master_volume: u32,
    /// `-m`: disable all sound.
    pub disable_sound: bool,
    /// `-no-mb`: disable Mockingboard-family output.
    pub disable_mockingboard: bool,
    /// `-spkr-inc` / `-spkr-max` (SoundCore error increment / maximum).
    pub speaker_error_inc: Option<i32>,
    pub speaker_error_max: Option<i32>,
    /// `-speech`.
    pub enable_speech: bool,
    /// `-wav-speaker` / `-wav-mockingboard` recording targets.
    pub wav_speaker: Option<PathBuf>,
    pub wav_mockingboard: Option<PathBuf>,
}

/// One joystick port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JoystickSettings {
    pub joystick0: Joystick0Kind,
    pub joystick1: Joystick1Kind,
    /// "PDL X-Trim" / "PDL Y-Trim".
    pub pdl_x_trim: i16,
    pub pdl_y_trim: i16,
    /// "Joystick Cursor Control".
    pub cursor_control: bool,
    /// "Joystick Centering Control": true = centering, false = floating (`JOYSTICK_MODE_*`).
    pub centering: bool,
    /// "Autofire" bitmap of buttons 0-2 (7 = all).
    pub autofire: u8,
    /// "Swap buttons 0 and 1" / `-swap-buttons`.
    pub swap_buttons: bool,
    /// "Shift Key Mod".
    pub shift_key_mod: bool,
    /// `-no-joystick1`, `-no-joystick2`.
    pub disabled_host_devices: [bool; 2],
    /// `-no-hook-alt`: do not use Alt as joystick buttons (inverted, default hooks).
    pub hook_alt_keys: bool,
    /// `-left-alt-control-buttons` / `-right-alt-control-buttons`: None = default.
    pub alt_control_buttons: Option<AltControlButtons>,
    /// "Game I/O type": dongle in the game connector.
    pub dongle: DongleType,
}

/// Which side's Alt/Control keys are buttons 0/1 (GH#743).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AltControlButtons {
    Left,
    Right,
}

impl Default for JoystickSettings {
    fn default() -> Self {
        JoystickSettings {
            joystick0: Joystick0Kind::HostJoystick1,
            joystick1: Joystick1Kind::Disabled,
            pdl_x_trim: 0,
            pdl_y_trim: 0,
            cursor_control: false,
            centering: false,
            autofire: 0,
            swap_buttons: false,
            shift_key_mod: false,
            disabled_host_devices: [false; 2],
            hook_alt_keys: true,
            alt_control_buttons: None,
            dongle: DongleType::Empty,
        }
    }
}

/// Keyboard layout of the clones (Pravets 8A/C, TK3000 //e).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloneKeyboard {
    #[default]
    Auto,
    Latin,
    Cyrillic,
}

/// Keyboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardSettings {
    /// "ScrollLock Toggle": Scroll Lock toggles full speed instead of being held.
    pub scroll_lock_toggle: bool,
    /// Initial Caps Lock state (`-capslock=off`).
    pub caps_lock: bool,
    /// Pravets/TK3000 layout selection (host-side; AppleWin derives it from the model).
    pub clone_layout: CloneKeyboard,
    /// `-altgr-sends-wmchar`.
    pub altgr_sends_char: bool,
    /// Windows-only low-level hook switches: `-no-hook-system-key`, `-hook-alt-tab`, `-hook-altgr-control`.
    pub hook_system_keys: bool,
    pub hook_alt_tab: bool,
    pub hook_altgr_control: bool,
    /// "Mouse crosshair" / "Mouse restrict to window" (global mouse-interface options).
    pub mouse_show_crosshair: bool,
    pub mouse_restrict_to_window: bool,
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        KeyboardSettings {
            scroll_lock_toggle: false,
            caps_lock: true,
            clone_layout: CloneKeyboard::Auto,
            altgr_sends_char: false,
            hook_system_keys: true,
            hook_alt_tab: false,
            hook_altgr_control: false,
            mouse_show_crosshair: false,
            mouse_restrict_to_window: false,
        }
    }
}

/// Aux-slot card ("Slot Auxiliary" section; Utilities.cpp:262-285).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuxSettings {
    pub card: SlotCardSpec,
}

impl Default for AuxSettings {
    fn default() -> Self {
        AuxSettings {
            card: SlotCardSpec::Extended80Col,
        }
    }
}

/// Disks, directories, save-states and other storage choices.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    /// "Enhance Disk Speed" (default on).
    pub enhance_disk_speed: bool,
    /// Preferences "Starting Directory" (last disk image dir).
    pub disk_dir: Option<PathBuf>,
    /// Preferences "HDV Starting Directory".
    pub hdv_dir: Option<PathBuf>,
    /// "Save State Filename".
    pub savestate_filename: Option<PathBuf>,
    /// "Save State On Exit".
    pub save_state_on_exit: bool,
    /// "CiderPress Location".
    pub ciderpress_path: Option<PathBuf>,
    /// Preferences "NewDiskCopy*" (BitsyBoot, BitsyBye, BASIC, ProDOS).
    pub new_disk_copy_bitsy_boot: bool,
    pub new_disk_copy_bitsy_bye: bool,
    pub new_disk_copy_basic: bool,
    pub new_disk_copy_prodos: bool,
    /// `-harddisknumblocks`.
    pub harddisk_num_blocks: Option<u32>,
    /// `-s7-empty-on-exit`.
    pub slot7_empty_on_exit: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        StorageSettings {
            enhance_disk_speed: true,
            disk_dir: None,
            hdv_dir: None,
            savestate_filename: None,
            save_state_on_exit: false,
            ciderpress_path: None,
            new_disk_copy_bitsy_boot: false,
            new_disk_copy_bitsy_bye: false,
            new_disk_copy_basic: false,
            new_disk_copy_prodos: false,
            harddisk_num_blocks: None,
            slot7_empty_on_exit: false,
        }
    }
}

/// Debugger and diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DebuggerSettings {
    /// `-debugger-auto-run <script>`.
    pub auto_run_script: Option<PathBuf>,
    /// `-l`/`-log`.
    pub log: bool,
    /// `-alt-cpu-emu` (debug).
    pub alt_cpu_emulation: bool,
}

/// Misc behaviour.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MiscSettings {
    /// "Confirm Reboot" (default on).
    pub confirm_reboot: bool,
    /// "Version": last AppleWin version that wrote the configuration (import only).
    pub imported_applewin_version: Option<String>,
    /// `-use-real-printer`.
    pub enable_dump_to_real_printer: bool,
    /// `-no-di` (no DirectInput), meaningful only on Windows.
    pub disable_direct_input: bool,
    /// Hidden debug switches: `-no-disk2-stepper-defer`, `-hdc-firmware-v1/v2`, `-load-state-ignore-hdc-fw`, `-no-nsc`.
    pub no_disk2_stepper_defer: bool,
    pub hdc_firmware_v1: bool,
    pub hdc_firmware_v2: bool,
}

impl Default for MiscSettings {
    fn default() -> Self {
        MiscSettings {
            confirm_reboot: true,
            imported_applewin_version: None,
            enable_dump_to_real_printer: false,
            disable_direct_input: false,
            no_disk2_stepper_defer: false,
            hdc_firmware_v1: false,
            hdc_firmware_v2: false,
        }
    }
}

/// Slots 0-7 ("Slot N" sections).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots(pub [SlotCardSpec; 8]);

impl Default for Slots {
    fn default() -> Self {
        Slots(std::array::from_fn(|i| {
            SlotCardSpec::default_for_slot(i, Model::Apple2eEnhanced)
        }))
    }
}

// TOML-friendly: a table "0".."7"; missing entries get the new-install default.
impl Serialize for Slots {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(8))?;
        for (i, c) in self.0.iter().enumerate() {
            m.serialize_entry(&i.to_string(), c)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for Slots {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Per-slot tolerance: a card unknown to this version leaves the slot at its default.
        let map = std::collections::BTreeMap::<String, toml::Value>::deserialize(d)?;
        let mut slots = Slots::default();
        for (k, v) in map {
            if let (Some(i), Ok(card)) = (
                k.parse::<usize>().ok().filter(|i| *i < 8),
                v.try_into::<SlotCardSpec>(),
            ) {
                slots.0[i] = card;
            }
        }
        Ok(slots)
    }
}

/// Everything the user can configure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub machine: MachineSettings,
    /// Optional `[roms]` table: user ROM files (see [`crate::roms`]).
    pub roms: crate::roms::RomSettings,
    pub video: VideoSettings,
    pub sound: SoundSettings,
    pub joystick: JoystickSettings,
    pub keyboard: KeyboardSettings,
    pub slots: Slots,
    pub aux: AuxSettings,
    pub storage: StorageSettings,
    pub debugger: DebuggerSettings,
    pub misc: MiscSettings,
}

impl Settings {
    /// Slot `n` (0-7).
    pub fn slot(&self, n: usize) -> &SlotCardSpec {
        &self.slots.0[n]
    }

    pub fn slot_mut(&mut self, n: usize) -> &mut SlotCardSpec {
        &mut self.slots.0[n]
    }

    /// Slot of the first card of `ty`, if any.
    pub fn find_card(&self, ty: CardType) -> Option<usize> {
        self.slots.0.iter().position(|c| c.card_type() == ty)
    }

    /// Slot 6 Disk II drives, creating the card when absent (target of `-d1`/`-d2`).
    pub fn disk_drives_mut(&mut self, slot: usize) -> &mut [DriveSpec; 2] {
        if !matches!(self.slots.0[slot], SlotCardSpec::Disk2 { .. }) {
            self.slots.0[slot] = SlotCardSpec::new(CardType::Disk2);
        }
        match &mut self.slots.0[slot] {
            SlotCardSpec::Disk2 { drives, .. } => drives,
            _ => unreachable!(),
        }
    }

    /// Drive `drive` of the first Disk II card now holds `image` (`None` = ejected). AppleWin
    /// writes it to the registry at once (`SaveLastDiskImage`, Disk.cpp:227-258), so a restart
    /// from the configuration dialog or a new run finds the same disks (GH#98, GH#511, GH#960);
    /// 'Starting Directory' follows slot 6 drive 1 only.
    pub fn remember_floppy(&mut self, drive: usize, image: Option<PathBuf>) {
        let Some(slot) = self.find_card(CardType::Disk2) else {
            return;
        };
        if slot == 6 && drive == 0 {
            if let Some(dir) = image.as_deref().and_then(std::path::Path::parent) {
                self.storage.disk_dir = Some(dir.to_path_buf());
            }
        }
        if let SlotCardSpec::Disk2 { drives, .. } = &mut self.slots.0[slot] {
            if let Some(d) = drives.get_mut(drive) {
                d.image = image;
            }
        }
    }

    /// The drives of the first Disk II card swapped (F5; `Disk2InterfaceCard::SwapDisks`).
    pub fn swap_floppies(&mut self) {
        if let Some(SlotCardSpec::Disk2 { drives, .. }) = self
            .find_card(CardType::Disk2)
            .map(|s| &mut self.slots.0[s])
        {
            let a = drives[0].image.take();
            drives[0].image = drives[1].image.take();
            drives[1].image = a;
        }
    }

    /// Re-apply new-install slot-0 default after a model change (slot 0 depends on the model).
    pub fn reset_slot0_for_model(&mut self) {
        self.slots.0[0] = SlotCardSpec::default_for_slot(0, self.machine.model);
    }
}
