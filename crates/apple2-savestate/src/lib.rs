//! AppleWin `.aws.yaml` save-states (SaveState.cpp, YamlHelper.cpp).
//!
//! [`save_aws_yaml`] writes the document AppleWin's `Snapshot_SaveState` writes (file header,
//! `Apple2`, `Auxiliary Slot`, `Slots`, `Game I/O Connector`, `Miscellaneous` units), with its
//! key names, unit versions and hex conventions. [`load_aws_yaml`] reads any version AppleWin
//! reads (`File_hdr` version 2; unit versions as far back as AppleWin supports) and builds a
//! [`Machine`] from it.
//!
//! # Fidelity
//! AppleWin's units do not hold everything our machine does (the scheduler queue, card registers
//! of cards AppleWin saves only in part, ...). The writer therefore adds an `AppleII-Rust
//! Extension` block to the `Apple2` unit (see [`ext`]): AppleWin ignores it, our loader uses it to
//! resume bit-exactly. A file from real AppleWin has no such block and is rebuilt from the
//! AppleWin units alone, which reproduces the machine as faithfully as AppleWin itself does.
//!
//! # Cards
//! A card is written in the AppleWin view only when it has a mapping (see [`cards`]); the others
//! are listed in [`Saved::warnings`] and travel in the extension block alone, so AppleWin loads
//! the file without them instead of rejecting it.

#![forbid(unsafe_code)]

pub mod apple2;
pub mod cards;
pub mod ext;
pub mod host;
pub mod yaml;

pub use host::{default_card, ImageRequest, LoadHost};

use apple2_core::{CardKind, Machine, MachineConfig};
use cards::LoadCtx;
use yaml::{Map, Node, Writer};

/// `SS_FILE_VER` (SaveState.cpp:56).
pub const SS_FILE_VER: u32 = 2;
const UNIT_SLOTS_VER: u32 = 1;
const UNIT_GAME_IO_CONNECTOR_VER: u32 = 4;

/// Save/load failure.
#[derive(Debug)]
pub enum Error {
    /// Malformed or unsupported document (AppleWin shows the same text in its message box).
    Format(String),
    /// The machine cannot be saved or the state cannot be loaded into one.
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Format(s) | Error::Unsupported(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// Header text and options for [`save_aws_yaml`].
#[derive(Clone, Debug)]
pub struct SaveOptions {
    /// Goes in the `# Date-stamp:` comment (the core has no clock; the caller supplies it).
    pub date_stamp: String,
    /// Goes in the second comment line (AppleWin writes its version and build).
    pub app_version: String,
    /// Add the `AppleII-Rust Extension` block (exact resume in our loader). Off gives a file made
    /// only of AppleWin units.
    pub extension: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        SaveOptions { date_stamp: String::new(), app_version: format!("AppleII-Rust {}", env!("CARGO_PKG_VERSION")), extension: true }
    }
}

/// Result of [`save_aws_yaml`].
#[derive(Debug)]
pub struct Saved {
    pub text: String,
    /// Things the AppleWin view could not hold (cards without a mapping, ...).
    pub warnings: Vec<String>,
}

/// Result of [`load_aws_yaml`].
pub struct Loaded {
    pub machine: Machine,
    /// Non-fatal problems: missing images, cards the host could not build, unknown units...
    pub warnings: Vec<String>,
}

/// Writes `machine` as an AppleWin save-state (`Snapshot_SaveState`).
pub fn save_aws_yaml(machine: &Machine, opts: &SaveOptions) -> Result<Saved, Error> {
    let st = machine.save_state();
    let lcs = st.lc.infos();
    let mut warnings = Vec::new();
    let mut w = Writer::new(&opts.date_stamp, &opts.app_version);
    w.file_hdr(SS_FILE_VER);

    // Every card in a slot (for the extension's manifest).
    let manifest: Vec<(u8, String)> =
        (0..8u8).filter_map(|s| machine.board().slots.card(s).map(|c| (s, c.name().to_string()))).collect();

    let mut ext_err = None;
    apple2::save_apple2(&mut w, &st, &lcs, |w| {
        if opts.extension {
            if let Err(e) = ext::write(w, &st, &manifest) {
                ext_err = Some(e);
            }
        }
    });
    if let Some(e) = ext_err {
        return Err(e);
    }
    apple2::save_aux(&mut w, &st);

    // Unit: Slots
    enum Entry<'a> {
        Lc(&'a apple2_core::language_card::LcInfo),
        Card(u8, &'a dyn apple2_core::Card),
    }
    let mut entries = Vec::new();
    for slot in 0..8u8 {
        if let Some(lc) = lcs.iter().find(|c| c.slot == slot) {
            if !matches!(lc.kind, apple2_core::language_card::LcKind::Builtin) {
                entries.push(Entry::Lc(lc));
            }
        } else if let Some(card) = machine.board().slots.card(slot) {
            if cards::by_our_name(card.name()).is_some() {
                entries.push(Entry::Card(slot, card));
            } else {
                warnings.push(format!("Slot {slot}: '{}' has no AppleWin unit; saved in the extension only", card.name()));
            }
        }
    }
    if !entries.is_empty() {
        let env = cards::SaveEnv {
            model: st.config.model,
            aux_empty: st.config.model.is_apple2e_family() && st.config.aux_card == apple2_core::AuxCardKind::Empty,
        };
        cards::with_save_env(env, || {
            w.unit("Slots", UNIT_SLOTS_VER, |w| {
                for e in &entries {
                    match e {
                        Entry::Lc(lc) => apple2::save_lc_card(w, &st, lc),
                        Entry::Card(slot, card) => {
                            cards::save_card(w, *slot, *card);
                        }
                    }
                }
            })
        });
    }

    // Unit: Game I/O Connector (CopyProtectionDongles.cpp:307). The shift-key mod is a frontend
    // setting; dongles travel in the extension.
    w.unit("Game I/O Connector", UNIT_GAME_IO_CONNECTOR_VER, |w| w.bool("Shift Key Mod", false));
    if st.game_io.is_some() {
        warnings.push("Game I/O Connector: the dongle is saved in the extension only".into());
    }
    if st.rom_tap.is_some() {
        warnings.push("Miscellaneous: the No-Slot Clock is saved in the extension only".into());
    }
    Ok(Saved { text: w.finish(), warnings })
}

fn unit_maps<'a>(top: &'a [(String, Node)]) -> Vec<Map<'a>> {
    top.iter()
        .filter_map(|(k, n)| match (k.as_str(), n) {
            ("Unit", Node::Map(m)) => Some(Map { name: "Unit", entries: m }),
            _ => None,
        })
        .collect()
}

/// Reads an AppleWin save-state and builds the machine (`Snapshot_LoadState_v2`).
///
/// `host` re-opens disk images and builds cards that need host objects. Problems that AppleWin
/// would also survive (a missing image) become [`Loaded::warnings`]; malformed or unsupported
/// documents are errors.
pub fn load_aws_yaml(text: &str, host: &mut dyn LoadHost) -> Result<Loaded, Error> {
    let top = yaml::parse(text)?;
    // File header (YamlHelper::ParseFileHdr).
    let hdr = top
        .iter()
        .find_map(|(k, n)| match (k.as_str(), n) {
            ("File_hdr", Node::Map(m)) => Some(Map { name: "File_hdr", entries: m }),
            _ => None,
        })
        .ok_or_else(|| Error::Format("Failed to find file header".into()))?;
    if hdr.string("Tag")? != "AppleWin Save State" {
        return Err(Error::Format("File_hdr: Bad tag".into()));
    }
    if hdr.u32("Version")? != SS_FILE_VER {
        return Err(Error::Format("Version mismatch".into()));
    }
    for (k, _) in &top {
        if k != "File_hdr" && k != "Unit" {
            return Err(Error::Format(format!("Unknown top-level scalar: {k}")));
        }
    }

    let mut warnings = Vec::new();
    let (mut a2, mut aux, mut slots_unit) = (None, None, None);
    for u in unit_maps(&top) {
        let kind = u.string("Type")?;
        let version = u.u32("Version")?;
        let state = u.need_sub("State").map_err(|_| Error::Format("Unit: Expected sub-map name: State".into()))?;
        match kind.as_str() {
            "Apple2" => a2 = Some(apple2::parse_apple2(version, state)?),
            "Auxiliary Slot" => aux = Some(apple2::parse_aux(version, state)?),
            "Slots" => {
                if version != UNIT_SLOTS_VER {
                    return Err(Error::Format("Unit: Slots: Version mismatch".into()));
                }
                slots_unit = Some(state);
            }
            "Game I/O Connector" => {
                if !(1..=UNIT_GAME_IO_CONNECTOR_VER).contains(&version) {
                    return Err(Error::Format(format!("Version {version} is not supported for game I/O device.")));
                }
            }
            "Miscellaneous" => warnings.push("Miscellaneous: the No-Slot Clock state is not loaded from the AppleWin unit".into()),
            other => warnings.push(format!("Unit: Unknown type: {other} (skipped)")),
        }
    }
    let a2 = a2.ok_or_else(|| Error::Format("Missing Apple2 unit".into()))?;
    let model = a2.model;
    let extension = ext::read(a2.state)?.filter(|e| e.state.config.model == model);

    // Machine configuration: RAM cards (slot 0 / Saturn) and the aux card are part of it.
    let mut cfg = MachineConfig::for_model(model);
    cfg.slots = [None; 8];
    cfg.video_standard = a2.video_standard;
    if let Some(a) = &aux {
        cfg.aux_card = a.kind;
    }
    let mut queued = Vec::new();
    let mut lc_loads = Vec::new();
    if let Some(s) = slots_unit {
        for (key, node) in s.entries {
            let Node::Map(entry) = node else { continue };
            let m = Map { name: key, entries: entry };
            let slot: u8 = key.parse().ok().filter(|&n| n < 8).ok_or_else(|| Error::Format(format!("Slots: Invalid slot #: {key}")))?;
            let card = m.string("Card")?;
            let version = m.u32("Version")?;
            let state = m.need_sub("State").map_err(|_| Error::Format("Unit: Expected sub-map name: State".into()))?;
            match card.as_str() {
                "Language Card" => {
                    cfg.slots[slot as usize] = Some(CardKind::LanguageCard16K);
                    lc_loads.push(apple2::parse_lc_card(slot, false, version, state)?);
                }
                "Saturn 128" => {
                    cfg.slots[slot as usize] = Some(CardKind::Saturn128K);
                    lc_loads.push(apple2::parse_lc_card(slot, true, version, state)?);
                }
                _ => queued.push((slot, card, version, state)),
            }
        }
    }
    if let Some(e) = &extension {
        cfg = e.state.config.clone();
    }
    let mut machine = Machine::try_new(cfg.clone()).map_err(|e| Error::Unsupported(e.to_string()))?;
    let mut inserted = [false; 8];

    // Cards (Unit: Slots).
    let mut ctx = LoadCtx { host, model, warnings: &mut warnings };
    for (slot, card, version, state) in queued {
        let Some(map) = cards::by_aw_name(&card) else {
            ctx.warnings.push(format!("Slot {slot}: card '{card}' is not supported (skipped)"));
            continue;
        };
        if let Some(c) = (map.load)(&mut ctx, slot, version, state)? {
            // `insert_card` power-on resets the card: put the loaded state back afterwards.
            let bytes = c.save();
            if machine.insert_card(slot, c) {
                machine.with_card(slot, |c, _| c.load(&bytes));
                inserted[slot as usize] = true;
            } else {
                ctx.warnings.push(format!("Slot {slot}: cannot insert '{card}' there"));
            }
        }
    }
    // Cards only the extension knows about.
    if let Some(e) = &extension {
        for (slot, name) in &e.cards {
            let lc_slot = cfg.slots.get(*slot as usize).is_some_and(|k| k.is_some());
            if inserted[*slot as usize] || lc_slot || machine.board().slots.card(*slot).is_some() {
                continue;
            }
            if let Some(c) = cards::make(&mut ctx, name, *slot) {
                if machine.insert_card(*slot, c) {
                    inserted[*slot as usize] = true;
                }
            }
        }
    }

    // State: AppleWin units first, then (if present) the exact extension state over it.
    let mut st = machine.save_state();
    // The Z80 SoftCard's `Active: 1` (z80.cpp:6575) makes the Z80 the running CPU.
    st.cpu_owner = (0..8u8).find(|&s| machine.card::<apple2_cards::SoftCard>(s).is_some_and(|c| c.is_active()));
    // Mouse card: a pending interrupt status asserts the IRQ line (MouseInterface.cpp:754, GH#677).
    for s in 0..8u8 {
        if machine.card::<apple2_cards::MouseCard>(s).is_some_and(|c| c.status() & apple2_cards::mouse::STAT_INT_ALL != 0) {
            st.interrupts.set_irq(apple2_core::IrqSource::Mouse, true);
        }
    }
    apple2::apply_apple2(&mut st, &a2, &lc_loads, ctx.warnings)?;
    if let Some(a) = &aux {
        apple2::load_aux_memory(&mut st, a)?;
    }
    let mut exact = false;
    if let Some(e) = &extension {
        match machine.load_state(&e.state) {
            Ok(()) => exact = true,
            Err(err) => ctx.warnings.push(format!("AppleII-Rust Extension ignored: {err}")),
        }
    }
    if !exact {
        machine.load_state(&st).map_err(|e| Error::Unsupported(e.to_string()))?;
        rearm_card_events(&mut machine);
    }
    Ok(Loaded { machine, warnings })
}

/// Re-creates the synchronous events a card keeps pending in AppleWin's save-state (the Disk II
/// deferred stepper, `InsertSyncEvent`).
fn rearm_card_events(m: &mut Machine) {
    for slot in 0..8u8 {
        m.with_card(slot, |c, ctx| {
            if let Some(d) = c.as_any().and_then(|a| a.downcast_ref::<apple2_storage::disk2::Disk2Card>()) {
                d.rearm_deferred_stepper(ctx);
            }
        });
    }
}
