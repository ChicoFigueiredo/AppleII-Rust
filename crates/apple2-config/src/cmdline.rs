//! AppleWin's command line (`ProcessCmdLine`, CmdLine.cpp:106-845), with its single-dash spellings.
//!
//! [`CmdLine::parse`] turns `argv` (without the program name) into ordered [`Opt`]s plus the
//! runtime-only actions; [`CmdLine::apply`] overlays the options on a [`Settings`] (apply to a
//! clone if the overrides must not be persisted). As in AppleWin, card insertions take effect
//! before per-card options (`-s6 diskii -s6d1 x.dsk` works in any order).

use crate::card::*;
use crate::settings::*;
use crate::Settings;
use apple2_core::Model;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CmdLineError {
    /// Option needs a value and `argv` ended (CmdLine.cpp `GetCurrArg` on an empty tail).
    MissingValue(String),
}

impl std::fmt::Display for CmdLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmdLineError::MissingValue(o) => write!(f, "option {o} requires a value"),
        }
    }
}

impl std::error::Error for CmdLineError {}

/// One settings override, in command-line order.
#[derive(Clone, Debug, PartialEq)]
pub enum Opt {
    Model(Model),
    RefreshRate(RefreshRate),
    /// `-sN <card>` / `-s0 ...` (phase 1).
    InsertCard {
        slot: usize,
        card: SlotCardSpec,
    },
    /// `-aux`, `-r` (phase 1).
    Aux(SlotCardSpec),
    /// `-d1/-d2/-sNdD`.
    DiskImage {
        slot: usize,
        drive: usize,
        path: PathBuf,
    },
    DriveDisconnected {
        drive: usize,
    },
    /// `-h1/-h2/-sNhD`.
    HdImage {
        slot: usize,
        unit: usize,
        path: PathBuf,
    },
    HdcFirmware {
        slot: usize,
        mode: HdcFirmware,
    },
    Bad6522 {
        slot: usize,
        b: bool,
    },
    AySocket {
        slot: usize,
        socket: usize,
        chip: AyChip,
    },
    Ssi263Socket {
        slot: usize,
        socket: usize,
        chip: Ssi263Chip,
    },
    Sc01 {
        slot: usize,
        present: bool,
    },
    Slot7EmptyOnExit,
    HarddiskNumBlocks(u32),
    Fullscreen(bool),
    FsWidth(u32),
    FsHeight(u32),
    FsHeightBest,
    NoDirectInput,
    DisableSound,
    NoMockingboard,
    MemClear(u32),
    CustomRom(PathBuf),
    CustomF8Rom(PathBuf),
    VideoRom(PathBuf),
    PrintScreenFilename,
    NoPrintScreenKey,
    NoPrintScreenDialog,
    NoHookSystemKey,
    HookAltTab,
    HookAltGrControl,
    AltGrSendsChar,
    CapsLockOff,
    NoHookAlt,
    AltControlButtons(AltControlButtons),
    SwapButtons,
    NoJoystick(usize),
    SpkrInc(i32),
    SpkrMax(i32),
    UseRealPrinter,
    Speech,
    MultiMon,
    Dcd,
    AltEnter(bool),
    VideoMode(VideoType),
    VerticalBlend(bool),
    RgbCardInvertBit7,
    MacLcCardDlgr,
    ClockMultiplier(f64),
    RgbCardType(RgbCardType),
    RgbCardForeground(u8),
    RgbCardBackground(u8),
    NoSlotClock,
    SnesMaxAlt {
        joy: usize,
    },
    SnesMaxUser {
        joy: usize,
        path: PathBuf,
    },
    WavSpeaker(PathBuf),
    WavMockingboard(PathBuf),
    NoDisk2StepperDefer,
    HdcFirmwareV1,
    HdcFirmwareV2,
    BootSector(PathBuf),
    AltCpuEmu,
    DebuggerAutoRun(PathBuf),
    SscTcpPort(u16),
    Log,
}

impl Opt {
    fn phase(&self) -> u8 {
        match self {
            Opt::Model(_) => 0,
            Opt::InsertCard { .. } | Opt::Aux(_) => 1,
            _ => 2,
        }
    }
}

/// Parsed command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CmdLine {
    /// Settings overrides in command-line order.
    pub opts: Vec<Opt>,
    /// `-load-state <file>`.
    pub load_state: Option<PathBuf>,
    /// `-load-state-ignore-hdc-fw` (testing).
    pub snapshot_ignore_hdc_firmware: bool,
    /// `-screenshot-and-exit <file>` (testing; use with `-load-state`).
    pub screenshot_and_exit: Option<PathBuf>,
    /// `-power-on`: boot immediately.
    pub power_on: bool,
    /// `-current-dir <dir>`.
    pub current_dir: Option<PathBuf>,
    /// `-conf <file>`: use this INI configuration instead of the default store.
    pub conf_file: Option<PathBuf>,
    /// `-noreg`: do not register file types (Windows).
    pub no_register_file_types: bool,
    /// `-mb-audit`, `-sN megaaudio|sdmusic`: expose the extra Mockingboard-family card types.
    pub support_extra_mb_card_types: bool,
    /// Bare (non-option) arguments, e.g. disk images / save-states opened by file association.
    /// AppleWin handles this through DDE; here the frontend decides what to do with them.
    pub positional: Vec<PathBuf>,
    /// `--help` / `-help`.
    pub show_help: bool,
    /// `--version`.
    pub show_version: bool,
    /// Options AppleWin would list in its "Unsupported commands" dialog.
    pub unsupported: Vec<String>,
    /// Non-fatal diagnostics AppleWin only logs (unsupported card/chip names, ...).
    pub warnings: Vec<String>,
}

/// C `atoi`: leading integer, 0 when none.
fn atoi(s: &str) -> i64 {
    let t = s.trim_start();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let n: i64 = t
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    if neg {
        -n
    } else {
        n
    }
}

/// C `atof` for plain decimals.
fn atof(s: &str) -> f64 {
    let t = s.trim_start();
    let end = t
        .char_indices()
        .take_while(|(i, c)| {
            c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+'))
        })
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    t[..end].parse().unwrap_or(0.0)
}

impl CmdLine {
    /// The checks AppleWin makes on `-rom`, `-f8rom` and `-videorom` (CmdLine.cpp:480-519,
    /// AppleWin.cpp:959-968), to run before the machine is created.
    ///
    /// * `Err(message)`: AppleWin shows the message and shuts down; do the same (exit with an
    ///   error, no window). `-f8rom` must be a file of exactly 0x800 bytes, `-rom` of exactly
    ///   0x3000 or 0x4000 bytes, and the two cannot be used together.
    /// * `Ok(warnings)`: a bad `-videorom` (not found or not exactly 2/4/8/16K) only shows a
    ///   message; the built-in video ROM stays in use.
    ///
    /// ROMs from `settings.toml` are not checked here: they fall back to the embedded ROM with a
    /// warning ([`Settings::resolve_roms`](crate::Settings::resolve_roms)).
    pub fn check_roms(&self) -> Result<Vec<String>, String> {
        let (mut rom, mut f8, mut video) = (None, None, None);
        for o in &self.opts {
            match o {
                Opt::CustomRom(p) => rom = Some(p),
                Opt::CustomF8Rom(p) => f8 = Some(p),
                Opt::VideoRom(p) => video = Some(p),
                _ => {}
            }
        }
        // Size of a file that can be opened; `None` = not found / not a file.
        let size = |p: &PathBuf| {
            std::fs::metadata(p)
                .ok()
                .filter(|m| m.is_file())
                .map(|m| m.len())
        };
        let f8_size = f8.map(size);
        let rom_size = rom.map(size);
        if matches!(f8_size, Some(s) if s != Some(0x800)) {
            return Err("Failed to load custom F8 rom (not found or not exactly 2KiB)".into());
        }
        if matches!(rom_size, Some(s) if s != Some(0x4000) && s != Some(0x3000)) {
            return Err(
                "Failed to load custom rom (not found or not exactly 12KiB or 16KiB)".into(),
            );
        }
        // Both handles are open (the files exist and passed the checks above).
        if f8_size.is_some() && rom_size.is_some() {
            return Err("Unsupported -rom and -f8rom being used at the same time".into());
        }
        let mut warnings = Vec::new();
        if let Some(p) = video {
            if !matches!(size(p), Some(0x800 | 0x1000 | 0x2000 | 0x4000)) {
                warnings
                    .push("Failed to load video rom (not found or not exactly 2/4/8/16KiB)".into());
            }
        }
        Ok(warnings)
    }

    pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<CmdLine, CmdLineError> {
        let mut c = CmdLine::default();
        let mut cmd_cards = [CardType::Empty; 8]; // `CardInstanceExists` (CmdLine.cpp:94) looks at the cmd line only
        let mut it = args.iter().map(AsRef::as_ref);
        while let Some(a) = it.next() {
            let mut val = |name: &str| {
                it.next()
                    .map(str::to_string)
                    .ok_or_else(|| CmdLineError::MissingValue(name.to_string()))
            };
            match a {
                "-l" | "-log" => {
                    c.opts.push(Opt::Log);
                }
                "-noreg" => c.no_register_file_types = true,
                "-conf" => c.conf_file = Some(val(a)?.into()),
                "-d1" | "-d2" => {
                    let drive = (a.as_bytes()[2] - b'1') as usize;
                    c.opts.push(Opt::DiskImage {
                        slot: 6,
                        drive,
                        path: val(a)?.into(),
                    });
                }
                "-d1-disconnected" => c.opts.push(Opt::DriveDisconnected { drive: 0 }),
                "-d2-disconnected" => c.opts.push(Opt::DriveDisconnected { drive: 1 }),
                "-h1" | "-h2" => {
                    let unit = (a.as_bytes()[2] - b'1') as usize;
                    c.opts.push(Opt::HdImage {
                        slot: 7,
                        unit,
                        path: val(a)?.into(),
                    });
                }
                "-s0" => {
                    let v = val(a)?;
                    let card = match v.as_str() {
                        "empty" => Some(SlotCardSpec::Empty),
                        "saturn" | "saturn128" => Some(SlotCardSpec::Saturn128K { banks: 8 }),
                        "saturn64" => Some(SlotCardSpec::Saturn128K { banks: 4 }),
                        "languagecard" | "lc" => Some(SlotCardSpec::LanguageCard),
                        _ => None, // AppleWin silently ignores other values for -s0
                    };
                    if let Some(card) = card {
                        c.opts.push(Opt::InsertCard { slot: 0, card });
                    }
                }
                "-s7-empty-on-exit" => c.opts.push(Opt::Slot7EmptyOnExit),
                _ if a.len() >= 3
                    && a.starts_with("-s")
                    && matches!(a.as_bytes()[2], b'1'..=b'7') =>
                {
                    let slot = (a.as_bytes()[2] - b'0') as usize;
                    let rest = &a[3..];
                    let rb = rest.as_bytes();
                    if rest.is_empty() {
                        let v = val(a)?;
                        c.slot_card(slot, &v, &mut cmd_cards);
                    } else if rb.len() == 2 && rb[0] == b'd' && matches!(rb[1], b'1' | b'2') {
                        c.opts.push(Opt::DiskImage {
                            slot,
                            drive: (rb[1] - b'1') as usize,
                            path: val(a)?.into(),
                        });
                    } else if rb.len() == 2 && rb[0] == b'h' && matches!(rb[1], b'1'..=b'8') {
                        c.opts.push(Opt::HdImage {
                            slot,
                            unit: (rb[1] - b'1') as usize,
                            path: val(a)?.into(),
                        });
                    } else {
                        c.unsupported.push(a.to_string());
                    }
                }
                "-aux" => {
                    let v = val(a)?;
                    match v.as_str() {
                        "empty" => c.opts.push(Opt::Aux(SlotCardSpec::Empty)),
                        "std80" => c.opts.push(Opt::Aux(SlotCardSpec::Col80)),
                        "ext80" => c.opts.push(Opt::Aux(SlotCardSpec::Extended80Col)),
                        // banks: keep a previous `-r`, else the real RW3 default (CmdLine.cpp:369-374)
                        "rw3" => {
                            let banks = c
                                .opts
                                .iter()
                                .rev()
                                .find_map(|o| {
                                    if let Opt::Aux(SlotCardSpec::RamWorksIII { banks }) = o {
                                        Some(*banks)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(16);
                            c.opts.push(Opt::Aux(SlotCardSpec::RamWorksIII { banks }));
                        }
                        _ => c.warnings.push(format!("Unsupported aux slot card: {v}")),
                    }
                }
                "-harddisknumblocks" => c.opts.push(Opt::HarddiskNumBlocks(
                    atoi(&val(a)?).clamp(0, 0x007F_FFFF) as u32,
                )),
                "-load-state" => c.load_state = Some(val(a)?.into()),
                "-load-state-ignore-hdc-fw" => c.snapshot_ignore_hdc_firmware = true,
                "-f" | "-full-screen" => c.opts.push(Opt::Fullscreen(true)),
                "-no-full-screen" => c.opts.push(Opt::Fullscreen(false)),
                _ if a.starts_with("-fs-width=") => {
                    c.implicit_fullscreen();
                    match atoi(&a[10..]) {
                        0 => c
                            .warnings
                            .push("Invalid cmd-line parameter for -fs-width=x switch".into()),
                        w => c
                            .opts
                            .push(Opt::FsWidth(w.clamp(0, u32::MAX as i64) as u32)),
                    }
                }
                _ if a.starts_with("-fs-height=") => {
                    c.implicit_fullscreen();
                    if &a[11..] == "best" {
                        c.opts.push(Opt::FsHeightBest);
                    } else {
                        match atoi(&a[11..]) {
                            0 => c
                                .warnings
                                .push("Invalid cmd-line parameter for -fs-height=x switch".into()),
                            h => c
                                .opts
                                .push(Opt::FsHeight(h.clamp(0, u32::MAX as i64) as u32)),
                        }
                    }
                }
                "-no-di" => c.opts.push(Opt::NoDirectInput),
                "-m" => c.opts.push(Opt::DisableSound),
                "-no-mb" => c.opts.push(Opt::NoMockingboard),
                "-memclear" => c
                    .opts
                    .push(Opt::MemClear(atoi(&val(a)?).clamp(0, 7) as u32)),
                "-r" => {
                    let banks = atoi(&val(a)?).clamp(1, 256) as u16;
                    c.opts.push(Opt::Aux(SlotCardSpec::RamWorksIII { banks }));
                }
                "-f8rom" => c.opts.push(Opt::CustomF8Rom(val(a)?.into())),
                "-rom" => c.opts.push(Opt::CustomRom(val(a)?.into())),
                "-videorom" => c.opts.push(Opt::VideoRom(val(a)?.into())),
                "-printscreen" => c.opts.push(Opt::PrintScreenFilename),
                "-no-printscreen-key" => c.opts.push(Opt::NoPrintScreenKey),
                "-no-printscreen-dlg" => c.opts.push(Opt::NoPrintScreenDialog),
                "-no-hook-system-key" => c.opts.push(Opt::NoHookSystemKey),
                "-hook-alt-tab" => c.opts.push(Opt::HookAltTab),
                "-hook-altgr-control" => c.opts.push(Opt::HookAltGrControl),
                "-altgr-sends-wmchar" => c.opts.push(Opt::AltGrSendsChar),
                "-capslock=off" => c.opts.push(Opt::CapsLockOff),
                "-no-hook-alt" => c.opts.push(Opt::NoHookAlt),
                "-left-alt-control-buttons" => {
                    c.opts.push(Opt::AltControlButtons(AltControlButtons::Left))
                }
                "-right-alt-control-buttons" => c
                    .opts
                    .push(Opt::AltControlButtons(AltControlButtons::Right)),
                "-swap-buttons" => c.opts.push(Opt::SwapButtons),
                _ if a.starts_with("-no-joystick") => match a.as_bytes().get(12) {
                    Some(b'1') if a.len() == 13 => c.opts.push(Opt::NoJoystick(0)),
                    Some(b'2') if a.len() == 13 => c.opts.push(Opt::NoJoystick(1)),
                    _ => c
                        .warnings
                        .push(format!("-no-joystick: unsupported device: {a}")),
                },
                "-spkr-inc" => c.opts.push(Opt::SpkrInc(atoi(&val(a)?) as i32)),
                "-spkr-max" => c.opts.push(Opt::SpkrMax(atoi(&val(a)?) as i32)),
                "-use-real-printer" => c.opts.push(Opt::UseRealPrinter),
                "-speech" => c.opts.push(Opt::Speech),
                "-multimon" => c.opts.push(Opt::MultiMon),
                "-dcd" | "-modem" => c.opts.push(Opt::Dcd),
                "-alt-enter=toggle-full-screen" => c.opts.push(Opt::AltEnter(true)),
                "-alt-enter=open-apple-enter" => c.opts.push(Opt::AltEnter(false)),
                "-video-mode=idealized" => c.opts.push(Opt::VideoMode(VideoType::ColorIdealized)),
                "-video-mode=rgb-videocard" => {
                    c.opts.push(Opt::VideoMode(VideoType::ColorVideocardRgb))
                }
                "-video-mode=composite-monitor" => {
                    c.opts.push(Opt::VideoMode(VideoType::ColorMonitorNtsc))
                }
                "-video-style=vertical-blend" => c.opts.push(Opt::VerticalBlend(true)),
                "-video-style=no-vertical-blend" => c.opts.push(Opt::VerticalBlend(false)),
                "-rgb-card-invert-bit7" => c.opts.push(Opt::RgbCardInvertBit7),
                "-mac-lc-card-dlgr" => c.opts.push(Opt::MacLcCardDlgr),
                "-screenshot-and-exit" => c.screenshot_and_exit = Some(val(a)?.into()),
                "-clock-multiplier" => c.opts.push(Opt::ClockMultiplier(atof(&val(a)?))),
                "-model" => {
                    let v = val(a)?;
                    match v.as_str() {
                        "apple2" => c.opts.push(Opt::Model(Model::Apple2)),
                        "apple2p" => c.opts.push(Opt::Model(Model::Apple2Plus)),
                        "apple2jp" => c.opts.push(Opt::Model(Model::Apple2JPlus)),
                        "apple2e" => c.opts.push(Opt::Model(Model::Apple2e)),
                        "apple2ee" => c.opts.push(Opt::Model(Model::Apple2eEnhanced)),
                        _ => c.warnings.push(format!("-model: unsupported type: {v}")),
                    }
                }
                _ if a.eq_ignore_ascii_case("-50hz") => {
                    c.opts.push(Opt::RefreshRate(RefreshRate::Hz50))
                }
                _ if a.eq_ignore_ascii_case("-60hz") => {
                    c.opts.push(Opt::RefreshRate(RefreshRate::Hz60))
                }
                "-rgb-card-type" => {
                    let v = val(a)?;
                    match v.as_str() {
                        "apple" | "sl7" => c.opts.push(Opt::RgbCardType(RgbCardType::Video7Sl7)),
                        "eve" => c.opts.push(Opt::RgbCardType(RgbCardType::LeChatMauveEve)),
                        "feline" => c
                            .opts
                            .push(Opt::RgbCardType(RgbCardType::LeChatMauveFeline)),
                        _ => c
                            .warnings
                            .push(format!("-rgb-card-type: unsupported type: {v}")),
                    }
                }
                "-rgb-card-foreground" => c
                    .opts
                    .push(Opt::RgbCardForeground(atoi(&val(a)?).clamp(0, 15) as u8)),
                "-rgb-card-background" => c
                    .opts
                    .push(Opt::RgbCardBackground(atoi(&val(a)?).clamp(0, 15) as u8)),
                "-power-on" => c.power_on = true,
                "-current-dir" => c.current_dir = Some(val(a)?.into()),
                "-no-nsc" => c.opts.push(Opt::NoSlotClock),
                "-snes-max-alt-joy1" => c.opts.push(Opt::SnesMaxAlt { joy: 0 }),
                "-snes-max-alt-joy2" => c.opts.push(Opt::SnesMaxAlt { joy: 1 }),
                "-snes-max-user-joy1" | "-snes-max-user-joy2" => {
                    let joy = (a.as_bytes()[a.len() - 1] - b'1') as usize;
                    c.opts.push(Opt::SnesMaxUser {
                        joy,
                        path: val(a)?.into(),
                    });
                }
                "-wav-speaker" => c.opts.push(Opt::WavSpeaker(val(a)?.into())),
                "-wav-mockingboard" => c.opts.push(Opt::WavMockingboard(val(a)?.into())),
                "-mb-audit" => c.support_extra_mb_card_types = true,
                "-no-disk2-stepper-defer" => c.opts.push(Opt::NoDisk2StepperDefer),
                "-hdc-firmware-v1" => c.opts.push(Opt::HdcFirmwareV1),
                "-hdc-firmware-v2" => c.opts.push(Opt::HdcFirmwareV2),
                "-bootsector" => c.opts.push(Opt::BootSector(val(a)?.into())),
                "-alt-cpu-emu" => c.opts.push(Opt::AltCpuEmu),
                "-debugger-auto-run" => c.opts.push(Opt::DebuggerAutoRun(val(a)?.into())),
                "-ssc-tcp-port" => {
                    let v = val(a)?;
                    match u16::try_from(atoi(&v)) {
                        Ok(p) => c.opts.push(Opt::SscTcpPort(p)),
                        Err(_) => c.warnings.push(format!("SSC TCP Port out of range: {v}")),
                    }
                }
                "--help" | "-help" | "-?" => c.show_help = true,
                "--version" | "-version" => c.show_version = true,
                _ if !a.starts_with('-') => c.positional.push(a.into()),
                _ => c.unsupported.push(a.to_string()),
            }
        }
        Ok(c)
    }

    /// `-fs-width/-fs-height` imply `-f` unless one was already given (CmdLine.cpp:416-417).
    fn implicit_fullscreen(&mut self) {
        if !self.opts.iter().any(|o| matches!(o, Opt::Fullscreen(_))) {
            self.opts.push(Opt::Fullscreen(true));
        }
    }

    /// `-sN <name>` for N in 1..=7 (CmdLine.cpp:200-327).
    fn slot_card(&mut self, slot: usize, name: &str, cmd_cards: &mut [CardType; 8]) {
        let (has_printer, has_ssc, has_vidhd) = (
            cmd_cards.contains(&CardType::GenericPrinter),
            cmd_cards.contains(&CardType::Ssc),
            cmd_cards.contains(&CardType::VidHd),
        );
        let mut insert = |c: &mut CmdLine, card: SlotCardSpec| {
            cmd_cards[slot] = card.card_type();
            c.opts.push(Opt::InsertCard { slot, card });
        };
        match name {
            "empty" => insert(self, SlotCardSpec::Empty),
            "diskii" => insert(self, SlotCardSpec::new(CardType::Disk2)),
            "diskii13" => insert(
                self,
                SlotCardSpec::Disk2 {
                    firmware_13_sector: true,
                    drives: Default::default(),
                },
            ),
            "hdc" | "hdc-sp" | "hdc-bm2" | "hdc-bm4" => {
                let firmware = match name {
                    "hdc-sp" => HdcFirmware::SmartPort,
                    "hdc-bm2" => HdcFirmware::BlockMode2Devices,
                    "hdc-bm4" => HdcFirmware::BlockMode4Devices,
                    _ => HdcFirmware::Default,
                };
                insert(self, SlotCardSpec::new(CardType::GenericHdd));
                if firmware != HdcFirmware::Default {
                    self.opts.push(Opt::HdcFirmware {
                        slot,
                        mode: firmware,
                    });
                }
            }
            "saturn" | "saturn128" => insert(self, SlotCardSpec::Saturn128K { banks: 8 }),
            "megaaudio" => {
                self.support_extra_mb_card_types = true;
                insert(self, SlotCardSpec::new(CardType::MegaAudio));
            }
            "sdmusic" => {
                self.support_extra_mb_card_types = true;
                insert(self, SlotCardSpec::new(CardType::SdMusic));
            }
            "breakpointcard" => insert(self, SlotCardSpec::BreakpointCard),
            "6522a-bad" => self.opts.push(Opt::Bad6522 { slot, b: false }),
            "6522b-bad" => self.opts.push(Opt::Bad6522 { slot, b: true }),
            "parallel" if !has_printer => insert(self, SlotCardSpec::new(CardType::GenericPrinter)),
            "ssc" if !has_ssc => insert(self, SlotCardSpec::new(CardType::Ssc)),
            "vidhd" if !has_vidhd => insert(self, SlotCardSpec::VidHd),
            "parallel" | "ssc" | "vidhd" => {} // single-instance card already on the command line
            "sc01" => self.opts.push(Opt::Sc01 {
                slot,
                present: true,
            }),
            "no-sc01" => self.opts.push(Opt::Sc01 {
                slot,
                present: false,
            }),
            n if n.starts_with("ay-socket")
                && matches!(n.as_bytes().get(9), Some(b'0'..=b'3'))
                && n.as_bytes().get(10) == Some(&b'=') =>
            {
                let socket = (n.as_bytes()[9] - b'0') as usize;
                let chip = match &n[11..] {
                    "empty" => Some(AyChip::Empty),
                    "ay8910" => Some(AyChip::Ay8910),
                    "ay8912" => Some(AyChip::Ay8912),
                    "ay8913" => Some(AyChip::Ay8913),
                    "ym2149" => Some(AyChip::Ym2149),
                    _ => None,
                };
                match chip {
                    Some(chip) => self.opts.push(Opt::AySocket { slot, socket, chip }),
                    None => self
                        .warnings
                        .push(format!("Unsupported AY type: {}", &n[11..])),
                }
            }
            n if n.starts_with("socket")
                && matches!(n.as_bytes().get(6), Some(b'0' | b'1'))
                && n.as_bytes().get(7) == Some(&b'=') =>
            {
                let socket = (n.as_bytes()[6] - b'0') as usize;
                let chip = match &n[8..] {
                    "empty" => Some(Ssi263Chip::Empty),
                    "ssi263p" => Some(Ssi263Chip::Ssi263P),
                    "ssi263ap" => Some(Ssi263Chip::Ssi263AP),
                    _ => None,
                };
                match chip {
                    Some(chip) => self.opts.push(Opt::Ssi263Socket { slot, socket, chip }),
                    None => self
                        .warnings
                        .push(format!("Unsupported SSI263 type: {}", &n[8..])),
                }
            }
            _ => self
                .warnings
                .push(format!("Unsupported slot-{slot} card: {name}")),
        }
    }

    /// Overlays the options on `s`.
    pub fn apply(&self, s: &mut Settings) {
        let slot0_given = self
            .opts
            .iter()
            .any(|o| matches!(o, Opt::InsertCard { slot: 0, .. }));
        let mut ordered: Vec<&Opt> = self.opts.iter().collect();
        ordered.sort_by_key(|o| o.phase()); // stable: keeps command-line order within a phase
        for o in ordered {
            apply_one(o, s);
            if let Opt::Model(_) = o {
                if !slot0_given {
                    s.reset_slot0_for_model();
                }
            }
        }
    }
}

fn sound_sockets(c: &mut SlotCardSpec) -> Option<&mut SoundSockets> {
    match c {
        SlotCardSpec::MockingboardC(k)
        | SlotCardSpec::Phasor(k)
        | SlotCardSpec::MegaAudio(k)
        | SlotCardSpec::SdMusic(k) => Some(k),
        _ => None,
    }
}

fn apply_one(o: &Opt, s: &mut Settings) {
    match o {
        Opt::Model(m) => s.machine.model = *m,
        Opt::RefreshRate(r) => s.machine.refresh_rate = *r,
        Opt::InsertCard { slot, card } => s.slots.0[*slot] = card.clone(),
        Opt::Aux(c) => s.aux.card = c.clone(),
        Opt::DiskImage { slot, drive, path } => {
            s.disk_drives_mut(*slot)[*drive].image = Some(path.clone())
        }
        Opt::DriveDisconnected { drive } => s.disk_drives_mut(6)[*drive].connected = false,
        Opt::HdImage { slot, unit, path } => {
            if !matches!(s.slots.0[*slot], SlotCardSpec::GenericHdd { .. }) {
                s.slots.0[*slot] = SlotCardSpec::new(CardType::GenericHdd);
            }
            if let SlotCardSpec::GenericHdd { images, .. } = &mut s.slots.0[*slot] {
                images[*unit] = Some(path.clone());
            }
        }
        Opt::HdcFirmware { slot, mode } => {
            if let SlotCardSpec::GenericHdd { firmware, .. } = &mut s.slots.0[*slot] {
                *firmware = *mode;
            }
        }
        Opt::Bad6522 { slot, b } => {
            if let Some(k) = sound_sockets(&mut s.slots.0[*slot]) {
                k.bad_6522[*b as usize] = true;
            }
        }
        Opt::AySocket { slot, socket, chip } => {
            if let Some(k) = sound_sockets(&mut s.slots.0[*slot]) {
                k.ay[*socket] = *chip;
            }
        }
        Opt::Ssi263Socket { slot, socket, chip } => {
            if let Some(k) = sound_sockets(&mut s.slots.0[*slot]) {
                k.ssi263[*socket] = *chip;
            }
        }
        Opt::Sc01 { slot, present } => {
            if let Some(k) = sound_sockets(&mut s.slots.0[*slot]) {
                k.sc01 = *present;
            }
        }
        Opt::Slot7EmptyOnExit => s.storage.slot7_empty_on_exit = true,
        Opt::HarddiskNumBlocks(n) => s.storage.harddisk_num_blocks = Some(*n),
        Opt::Fullscreen(b) => s.video.fullscreen = *b,
        Opt::FsWidth(w) => s.video.fullscreen_width = Some(*w),
        Opt::FsHeight(h) => s.video.fullscreen_height = Some(*h),
        Opt::FsHeightBest => s.video.fullscreen_best_resolution = true,
        Opt::NoDirectInput => s.misc.disable_direct_input = true,
        Opt::DisableSound => s.sound.disable_sound = true,
        Opt::NoMockingboard => s.sound.disable_mockingboard = true,
        Opt::MemClear(n) => s.machine.memory_clear_type = *n,
        Opt::CustomRom(p) => s.machine.custom_rom = Some(p.clone()),
        Opt::CustomF8Rom(p) => s.machine.custom_f8_rom = Some(p.clone()),
        Opt::VideoRom(p) => s.machine.video_rom = Some(p.clone()),
        Opt::PrintScreenFilename => s.video.show_printscreen_filename = true,
        Opt::NoPrintScreenKey => s.video.capture_printscreen_key = false,
        Opt::NoPrintScreenDialog => s.video.show_printscreen_warning_dialog = false,
        Opt::NoHookSystemKey => s.keyboard.hook_system_keys = false,
        Opt::HookAltTab => s.keyboard.hook_alt_tab = true,
        Opt::HookAltGrControl => s.keyboard.hook_altgr_control = true,
        Opt::AltGrSendsChar => s.keyboard.altgr_sends_char = true,
        Opt::CapsLockOff => s.keyboard.caps_lock = false,
        Opt::NoHookAlt => s.joystick.hook_alt_keys = false,
        Opt::AltControlButtons(a) => s.joystick.alt_control_buttons = Some(*a),
        Opt::SwapButtons => s.joystick.swap_buttons = true,
        Opt::NoJoystick(n) => s.joystick.disabled_host_devices[*n] = true,
        Opt::SpkrInc(v) => s.sound.speaker_error_inc = Some(*v),
        Opt::SpkrMax(v) => s.sound.speaker_error_max = Some(*v),
        Opt::UseRealPrinter => s.misc.enable_dump_to_real_printer = true,
        Opt::Speech => s.sound.enable_speech = true,
        Opt::MultiMon => s.video.multi_monitor = true,
        Opt::Dcd => {
            for c in s.slots.0.iter_mut() {
                if let SlotCardSpec::Ssc { support_dcd, .. } = c {
                    *support_dcd = true;
                }
            }
        }
        Opt::AltEnter(b) => s.video.alt_enter_toggles_fullscreen = *b,
        Opt::VideoMode(t) => s.video.mode = *t,
        Opt::VerticalBlend(b) => s.video.vertical_blend = *b,
        Opt::RgbCardInvertBit7 => s.video.rgb_card_invert_bit7 = true,
        Opt::MacLcCardDlgr => s.video.mac_lc_card_dlgr = true,
        Opt::ClockMultiplier(m) => s.machine.clock_multiplier = Some(*m),
        Opt::RgbCardType(t) => s.video.rgb_card = *t,
        Opt::RgbCardForeground(v) => s.video.rgb_card_foreground = *v,
        Opt::RgbCardBackground(v) => s.video.rgb_card_background = *v,
        Opt::NoSlotClock => s.machine.no_slot_clock = false,
        Opt::SnesMaxAlt { joy } => {
            for c in s.slots.0.iter_mut() {
                if let SlotCardSpec::SnesMax { alt_controller, .. } = c {
                    alt_controller[*joy] = true;
                }
            }
        }
        Opt::SnesMaxUser { joy, path } => {
            for c in s.slots.0.iter_mut() {
                if let SlotCardSpec::SnesMax { user_mapping, .. } = c {
                    user_mapping[*joy] = Some(path.clone());
                }
            }
        }
        Opt::WavSpeaker(p) => s.sound.wav_speaker = Some(p.clone()),
        Opt::WavMockingboard(p) => s.sound.wav_mockingboard = Some(p.clone()),
        Opt::NoDisk2StepperDefer => s.misc.no_disk2_stepper_defer = true,
        Opt::HdcFirmwareV1 => s.misc.hdc_firmware_v1 = true,
        Opt::HdcFirmwareV2 => s.misc.hdc_firmware_v2 = true,
        Opt::BootSector(p) => s.machine.boot_sector = Some(p.clone()),
        Opt::AltCpuEmu => s.debugger.alt_cpu_emulation = true,
        Opt::DebuggerAutoRun(p) => s.debugger.auto_run_script = Some(p.clone()),
        Opt::SscTcpPort(p) => {
            for c in s.slots.0.iter_mut() {
                if let SlotCardSpec::Ssc { tcp_port, .. } = c {
                    *tcp_port = Some(*p);
                }
            }
        }
        Opt::Log => s.debugger.log = true,
    }
}

/// Splits a Windows-style command line like `GetNextArg` (CmdLine.cpp:61-90): spaces separate
/// arguments except inside double quotes, which are removed.
pub fn split_command_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut in_q, mut any) = (false, false);
    for ch in line.chars() {
        match ch {
            '"' => {
                in_q = !in_q;
                any = true;
            }
            ' ' if !in_q => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `--help` text.
pub const HELP: &str = "\
Usage: applewin [options]

Disks:
  -d1 <img> | -d2 <img>            Disk II drive 1/2 image in slot 6
  -d1-disconnected | -d2-disconnected   Disconnect drive 1/2
  -h1 <img> | -h2 <img>            Hard disk 1/2 image in slot 7
  -sN <card>                       Card in slot N (1-7): empty diskii diskii13 hdc hdc-sp hdc-bm2 hdc-bm4
                                   parallel ssc vidhd saturn(128) megaaudio sdmusic breakpointcard
  -s0 <card>                       Slot 0 (][ and ][+): empty lc|languagecard saturn|saturn128 saturn64
  -sNdD <img>                      Disk II image (N = 1-7, D = 1|2)
  -sNhD <img>                      Hard disk image (N = 1-7, D = 1-8)
  -sN ay-socketK=<chip>            K = 0-3: empty ay8910 ay8912 ay8913 ym2149
  -sN socketK=<chip>               K = 0|1: empty ssi263p ssi263ap
  -sN sc01 | no-sc01               SC-01 speech chip on the Mockingboard
  -sN 6522a-bad | 6522b-bad        Emulate a faulty 6522
  -s7-empty-on-exit                Remove the slot-7 card when exiting
  -harddisknumblocks <n>           Blocks reported to ProDOS
  -aux <empty|std80|ext80|rw3>     //e auxiliary slot card
  -r <1-256>                       RamWorks III with N 64K banks (in the aux slot)

Machine:
  -model <apple2|apple2p|apple2jp|apple2e|apple2ee>
  -50hz | -60hz                    PAL / NTSC timing
  -rom <file> | -f8rom <file> | -videorom <file>
  -memclear <0-7>                  Power-on memory pattern
  -clock-multiplier <x>            CPU speed multiplier
  -no-nsc                          Remove the No-Slot Clock
  -bootsector <file>               Custom boot sector
  -power-on                        Boot immediately

State and files:
  -load-state <file>               Load a save-state
  -load-state-ignore-hdc-fw        (testing) ignore HDC firmware in the save-state
  -screenshot-and-exit <file>      (testing) save a screenshot and exit
  -conf <file>                     Use this INI configuration file
  -current-dir <dir>               Starting directory
  -noreg                           Do not register file types
  -l | -log                        Write a log

Video:
  -f | -full-screen | -no-full-screen | -fs-width=<w> | -fs-height=<h|best>
  -video-mode=<idealized|rgb-videocard|composite-monitor>
  -video-style=<vertical-blend|no-vertical-blend>
  -rgb-card-type <apple|sl7|eve|feline> | -rgb-card-foreground <n> | -rgb-card-background <n>
  -rgb-card-invert-bit7 | -mac-lc-card-dlgr | -multimon
  -printscreen | -no-printscreen-key | -no-printscreen-dlg
  -alt-enter=<toggle-full-screen|open-apple-enter>

Sound:
  -m (no sound) | -no-mb | -spkr-inc <n> | -spkr-max <n> | -speech | -mb-audit
  -wav-speaker <file> | -wav-mockingboard <file>

Input:
  -swap-buttons | -no-joystick1 | -no-joystick2 | -no-hook-alt
  -left-alt-control-buttons | -right-alt-control-buttons | -capslock=off
  -no-hook-system-key | -hook-alt-tab | -hook-altgr-control | -altgr-sends-wmchar | -no-di
  -snes-max-alt-joy1|2 | -snes-max-user-joy1|2 <mapping-file>

Serial / printer:
  -ssc-tcp-port <port> | -dcd | -modem | -use-real-printer

Debugging:
  -debugger-auto-run <script> | -alt-cpu-emu | -no-disk2-stepper-defer | -hdc-firmware-v1 | -hdc-firmware-v2

  --help, --version
";

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (CmdLine, Settings) {
        let c = CmdLine::parse(args).unwrap();
        let mut s = Settings::default();
        c.apply(&mut s);
        (c, s)
    }

    #[test]
    fn disks_in_any_order() {
        let (c, s) = run(&[
            "-d1",
            "a.dsk",
            "-s6",
            "diskii13",
            "-d2",
            "b.dsk",
            "-d2-disconnected",
            "-h1",
            "h.hdv",
        ]);
        assert!(c.unsupported.is_empty());
        match s.slot(6) {
            SlotCardSpec::Disk2 {
                firmware_13_sector,
                drives,
            } => {
                assert!(*firmware_13_sector);
                assert_eq!(drives[0].image, Some("a.dsk".into()));
                assert_eq!(drives[1].image, Some("b.dsk".into()));
                assert!(drives[0].connected && !drives[1].connected);
            }
            x => panic!("{x:?}"),
        }
        assert!(
            matches!(s.slot(7), SlotCardSpec::GenericHdd { images, .. } if images[0] == Some("h.hdv".into()))
        );
    }

    #[test]
    fn slots_and_sub_options() {
        let (c, s) = run(&[
            "-s3",
            "vidhd",
            "-s2",
            "vidhd",
            "-s5",
            "hdc-sp",
            "-s5h3",
            "x.hdv",
            "-s4",
            "megaaudio",
            "-s4",
            "ay-socket1=ym2149",
            "-s4",
            "no-sc01",
            "-s0",
            "saturn64",
        ]);
        assert!(c.support_extra_mb_card_types);
        assert_eq!(s.slot(3), &SlotCardSpec::VidHd);
        assert_eq!(s.slot(2).card_type(), CardType::Ssc); // 2nd vidhd ignored
        assert!(
            matches!(s.slot(5), SlotCardSpec::GenericHdd { firmware: HdcFirmware::SmartPort, images } if images[2].is_some())
        );
        assert!(
            matches!(s.slot(4), SlotCardSpec::MegaAudio(k) if k.ay[1] == AyChip::Ym2149 && !k.sc01)
        );
        assert_eq!(s.slot(0), &SlotCardSpec::Saturn128K { banks: 4 });
    }

    #[test]
    fn model_resets_slot0_unless_given() {
        let (_, s) = run(&["-model", "apple2"]);
        assert_eq!(s.slot(0), &SlotCardSpec::Empty);
        let (_, s) = run(&["-model", "apple2p", "-s0", "empty"]);
        assert_eq!(s.slot(0), &SlotCardSpec::Empty);
        let (_, s) = run(&["-model", "apple2p"]);
        assert_eq!(s.slot(0), &SlotCardSpec::LanguageCard);
    }

    #[test]
    fn aux_ramworks() {
        let (_, s) = run(&["-r", "512"]);
        assert_eq!(s.aux.card, SlotCardSpec::RamWorksIII { banks: 256 });
        let (_, s) = run(&["-r", "0"]);
        assert_eq!(s.aux.card, SlotCardSpec::RamWorksIII { banks: 1 });
        let (_, s) = run(&["-aux", "rw3"]);
        assert_eq!(s.aux.card, SlotCardSpec::RamWorksIII { banks: 16 });
        let (_, s) = run(&["-r", "8", "-aux", "ext80"]);
        assert_eq!(s.aux.card, SlotCardSpec::Extended80Col);
    }

    #[test]
    fn video_and_runtime_flags() {
        let (c, s) = run(&[
            "-fs-width=1600",
            "-fs-height=best",
            "-50HZ",
            "-load-state",
            "x.aws",
            "-power-on",
            "-video-mode=rgb-videocard",
            "-no-printscreen-dlg",
            "-ssc-tcp-port",
            "1977",
            "-dcd",
        ]);
        assert!(
            s.video.fullscreen
                && s.video.fullscreen_best_resolution
                && s.video.fullscreen_width == Some(1600)
        );
        assert_eq!(s.machine.refresh_rate, RefreshRate::Hz50);
        assert_eq!(c.load_state, Some("x.aws".into()));
        assert!(c.power_on);
        assert_eq!(s.video.mode, VideoType::ColorVideocardRgb);
        assert!(!s.video.show_printscreen_warning_dialog);
        assert!(matches!(
            s.slot(2),
            SlotCardSpec::Ssc {
                tcp_port: Some(1977),
                support_dcd: true,
                ..
            }
        ));
        let (_, s) = run(&["-fs-width=800", "-no-full-screen"]);
        assert!(!s.video.fullscreen);
    }

    #[test]
    fn errors_and_unknowns() {
        assert_eq!(
            CmdLine::parse(&["-d1"]),
            Err(CmdLineError::MissingValue("-d1".into()))
        );
        let c = CmdLine::parse(&[
            "-bogus", "-s9x", "game.dsk", "-model", "foo", "-s1", "teapot",
        ])
        .unwrap();
        assert_eq!(c.unsupported, ["-bogus", "-s9x"]);
        assert_eq!(c.positional, [PathBuf::from("game.dsk")]);
        assert_eq!(c.warnings.len(), 2);
        assert!(CmdLine::parse(&["--help"]).unwrap().show_help);
    }

    #[test]
    fn numeric_clamps_like_atoi() {
        let (_, s) = run(&[
            "-memclear",
            "99",
            "-harddisknumblocks",
            "99999999",
            "-clock-multiplier",
            "2.5x",
            "-spkr-inc",
            "abc",
        ]);
        assert_eq!(s.machine.memory_clear_type, 7);
        assert_eq!(s.storage.harddisk_num_blocks, Some(0x7FFFFF));
        assert_eq!(s.machine.clock_multiplier, Some(2.5));
        assert_eq!(s.sound.speaker_error_inc, Some(0));
    }

    #[test]
    fn rom_options_set_paths_and_need_a_value() {
        let (c, s) = run(&[
            "-rom",
            "my sys.rom",
            "-f8rom",
            "f8.bin",
            "-videorom",
            "v.rom",
        ]);
        assert!(c.unsupported.is_empty());
        assert_eq!(s.machine.custom_rom, Some("my sys.rom".into()));
        assert_eq!(s.machine.custom_f8_rom, Some("f8.bin".into()));
        assert_eq!(s.machine.video_rom, Some("v.rom".into()));
        // the last occurrence wins, as AppleWin replaces the earlier handle
        let (_, s) = run(&["-rom", "a.rom", "-rom", "b.rom"]);
        assert_eq!(s.machine.custom_rom, Some("b.rom".into()));
        assert!(matches!(
            CmdLine::parse(&["-rom"]),
            Err(CmdLineError::MissingValue(o)) if o == "-rom"
        ));
        assert!(matches!(
            CmdLine::parse(&["-f8rom"]),
            Err(CmdLineError::MissingValue(o)) if o == "-f8rom"
        ));
    }

    #[test]
    fn rom_checks_follow_applewin_and_shut_down() {
        let d = std::env::temp_dir().join(format!("a2cl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mk = |name: &str, n: usize| {
            let p = d.join(name);
            std::fs::write(&p, vec![0u8; n]).unwrap();
            p.display().to_string()
        };
        let check = |args: &[&str]| CmdLine::parse(args).unwrap().check_roms();
        let (r12, r16, r20) = (mk("r12", 0x3000), mk("r16", 0x4000), mk("r20", 20 * 1024));
        let (f2, f3) = (mk("f2", 0x800), mk("f3", 0x801));
        let missing = d.join("missing").display().to_string();
        let f8_msg = "Failed to load custom F8 rom (not found or not exactly 2KiB)";
        let rom_msg = "Failed to load custom rom (not found or not exactly 12KiB or 16KiB)";
        // valid
        assert_eq!(check(&["-rom", &r12]), Ok(vec![]));
        assert_eq!(check(&["-rom", &r16]), Ok(vec![]));
        assert_eq!(check(&["-f8rom", &f2]), Ok(vec![]));
        assert_eq!(check(&[]), Ok(vec![]));
        // wrong size / missing: error (the 20K file is rejected on the command line)
        assert_eq!(check(&["-rom", &r20]), Err(rom_msg.into()));
        assert_eq!(check(&["-rom", &missing]), Err(rom_msg.into()));
        assert_eq!(check(&["-f8rom", &f3]), Err(f8_msg.into()));
        assert_eq!(check(&["-f8rom", &missing]), Err(f8_msg.into()));
        // F8 failure wins over ROM failure
        assert_eq!(check(&["-rom", &r20, "-f8rom", &f3]), Err(f8_msg.into()));
        // both together
        assert_eq!(
            check(&["-rom", &r16, "-f8rom", &f2]),
            Err("Unsupported -rom and -f8rom being used at the same time".into())
        );
        // -videorom: only a warning
        let v = mk("v", 3000);
        assert_eq!(
            check(&["-videorom", &v]),
            Ok(vec![
                "Failed to load video rom (not found or not exactly 2/4/8/16KiB)".into()
            ])
        );
        assert_eq!(check(&["-videorom", &f2]), Ok(vec![]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn splits_quoted_command_line() {
        assert_eq!(
            split_command_line(r#"-d1 "C:\my disks\a.dsk"  -f"#),
            ["-d1", r"C:\my disks\a.dsk", "-f"]
        );
    }
}
