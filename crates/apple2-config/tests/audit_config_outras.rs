//! Auditoria das issues upstream de categoria `config`/`outra` (gate G0, P23).
//!
//! Oraculo: AppleWin a8f0d1d (CmdLine.cpp, Utilities.cpp `LoadConfiguration`, Disk.cpp,
//! Harddisk.cpp, Configuration/Config.cpp). Resultados em
//! `docs/conformance/results/p23-config-outras.csv`.

use apple2_config::cmdline::{CmdLine, CmdLineError};
use apple2_config::import::import_ini;
use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_core::{Machine, Model};
use apple2_cpu::Bus;
use std::path::{Path, PathBuf};

fn apply(args: &[&str]) -> (CmdLine, Settings) {
    let c = CmdLine::parse(args).unwrap();
    let mut s = Settings::default();
    c.apply(&mut s);
    (c, s)
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("a2cfg-p23-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn floppy(s: &Settings, slot: usize, drive: usize) -> Option<PathBuf> {
    match s.slot(slot) {
        SlotCardSpec::Disk2 { drives, .. } => drives[drive].image.clone(),
        other => panic!("slot {slot}: {other:?}"),
    }
}

fn hdd(s: &Settings, slot: usize, unit: usize) -> Option<PathBuf> {
    match s.slot(slot) {
        SlotCardSpec::GenericHdd { images, .. } => images[unit].clone(),
        other => panic!("slot {slot}: {other:?}"),
    }
}

fn abs(p: &str) -> PathBuf {
    std::path::absolute(p).unwrap()
}

// ---- #1: "Enable hard disk in slot 7" must survive closing the dialog / restarting ----------

#[test]
fn gh1_hdd_in_slot7_survives_save_and_reload() {
    let d = tmp("gh1");
    let mut s = Settings::default();
    assert_eq!(s.slot(7).card_type(), CardType::Empty);
    *s.slot_mut(7) = SlotCardSpec::new(CardType::GenericHdd);
    let f = d.join("settings.toml");
    s.save(&f).unwrap();
    let back = Settings::load(&f).unwrap();
    assert_eq!(back.slot(7).card_type(), CardType::GenericHdd);
    // Utilities.cpp:255 (GH#1015): the pre-1.30.4 "Harddisk Enable" key still enables slot 7
    let legacy = import_ini("[Configuration]\nHarddisk Enable=1\n");
    assert_eq!(legacy.slot(7).card_type(), CardType::GenericHdd);
    let off = import_ini("[Configuration]\nHarddisk Enable=0\n");
    assert_eq!(off.slot(7).card_type(), CardType::Empty);
    let _ = std::fs::remove_dir_all(d);
}

// ---- #98 #110 #511 #960: images from different folders are kept, per drive, as full paths ----

#[test]
fn gh960_gh511_gh110_images_in_different_folders_persist_per_drive() {
    let d = tmp("gh960");
    let mut s = Settings::default();
    *s.slot_mut(7) = SlotCardSpec::new(CardType::GenericHdd);
    s.remember_floppy(0, Some("/disks/A1/a.dsk".into()));
    s.remember_floppy(1, Some("/disks/B/b.dsk".into()));
    if let SlotCardSpec::GenericHdd { images, .. } = s.slot_mut(7) {
        images[0] = Some("/hd/one/x.hdv".into());
        images[1] = Some("/hd/two/y.hdv".into());
    }
    let f = d.join("settings.toml");
    s.save(&f).unwrap();
    let back = Settings::load(&f).unwrap();
    assert_eq!(floppy(&back, 6, 0), Some("/disks/A1/a.dsk".into()));
    assert_eq!(floppy(&back, 6, 1), Some("/disks/B/b.dsk".into()));
    assert_eq!(hdd(&back, 7, 0), Some("/hd/one/x.hdv".into()));
    assert_eq!(hdd(&back, 7, 1), Some("/hd/two/y.hdv".into()));
    // AppleWin's registry (Disk.cpp:227-258) keeps the whole pathname of each drive
    let ini = "[Configuration\\Slot 6]\nCard type=1\nLast Disk Image 1=/disks/A1/a.dsk\n\
               Last Disk Image 2=/disks/B/b.dsk\n[Configuration\\Slot 7]\nCard type=5\n\
               Last Harddisk Image 1=/hd/one/x.hdv\nLast Harddisk Image 2=/hd/two/y.hdv\n";
    let imp = import_ini(ini);
    assert_eq!(floppy(&imp, 6, 0), Some("/disks/A1/a.dsk".into()));
    assert_eq!(floppy(&imp, 6, 1), Some("/disks/B/b.dsk".into()));
    assert_eq!(hdd(&imp, 7, 1), Some("/hd/two/y.hdv".into()));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn gh98_gh511_inserting_in_one_drive_never_touches_the_other() {
    // Disk.cpp:227-258: one drive's change rewrites only its own value, and 'Starting
    // Directory' follows slot 6 drive 1 only (otherwise "drive 1, then drive 2 in another
    // folder" gives inconsistent results).
    let mut s = Settings::default();
    s.remember_floppy(0, Some("/A1/a.dsk".into()));
    s.remember_floppy(1, Some("/B/b.dsk".into()));
    assert_eq!(s.storage.disk_dir, Some("/A1".into()));
    assert_eq!(floppy(&s, 6, 0), Some("/A1/a.dsk".into()));
    s.remember_floppy(0, None); // eject drive 1
    assert_eq!(floppy(&s, 6, 0), None);
    assert_eq!(floppy(&s, 6, 1), Some("/B/b.dsk".into()));
    s.remember_floppy(0, Some("/A1/a.dsk".into()));
    s.swap_floppies(); // F5
    assert_eq!(floppy(&s, 6, 0), Some("/B/b.dsk".into()));
    assert_eq!(floppy(&s, 6, 1), Some("/A1/a.dsk".into()));
    // no Disk II card: nothing to remember, nothing panics
    let mut none = Settings::default();
    *none.slot_mut(6) = SlotCardSpec::Empty;
    none.remember_floppy(0, Some("/x.dsk".into()));
    none.swap_floppies();
    assert_eq!(none.slot(6).card_type(), CardType::Empty);
}

// ---- #108 #276: -d1 with a relative path / without a value ------------------------------------

#[test]
fn gh108_gh276_relative_d1_is_resolved_and_a_missing_value_is_an_error_not_a_crash() {
    let (c, s) = apply(&["-d1", "disks/diskname.dsk", "-d2", "./other.dsk"]);
    assert!(c.unsupported.is_empty());
    // CmdLine.cpp:140-151 + GetFullPathName: the full pathname is what is kept and saved
    assert_eq!(floppy(&s, 6, 0), Some(abs("disks/diskname.dsk")));
    assert_eq!(floppy(&s, 6, 1), Some(abs("./other.dsk")));
    // 'Starting Directory' is the folder of drive 1's image (Disk.cpp:252-256), hdv one is untouched
    assert_eq!(
        s.storage.disk_dir,
        abs("disks/diskname.dsk").parent().map(Path::to_path_buf)
    );
    assert_eq!(s.storage.hdv_dir, None);
    // -d1 with nothing after it (the nullptr that quit AppleWin 1.25.0.3)
    assert_eq!(
        CmdLine::parse(&["-d1"]),
        Err(CmdLineError::MissingValue("-d1".into()))
    );
    // other options before -d1 are still honoured
    let (_, s) = apply(&["-f", "-no-printscreen-dlg", "-d1", "a.dsk"]);
    assert!(s.video.fullscreen && floppy(&s, 6, 0).is_some());
}

// ---- #790: -s7-empty-on-exit and the starting directories -------------------------------------

#[test]
fn gh790_s7_empty_on_exit_leaves_slot7_empty_in_the_saved_configuration() {
    let d = tmp("gh790");
    let (c, s) = apply(&["-h1", "game.hdv", "-s7-empty-on-exit"]);
    assert!(c.unsupported.is_empty());
    assert_eq!(hdd(&s, 7, 0), Some(abs("game.hdv")));
    assert!(s.storage.slot7_empty_on_exit);
    // AppleWin.cpp:1042: GetCardMgr().Remove(SLOT7) at exit; the registry then says "empty"
    let f = d.join("settings.toml");
    s.save(&f).unwrap();
    let back = Settings::load(&f).unwrap();
    assert_eq!(back.slot(7).card_type(), CardType::Empty);
    assert!(!back.storage.slot7_empty_on_exit, "the switch is per run");
    // the other settings of the run are saved untouched
    let (_, s) = apply(&["-h1", "game.hdv"]);
    s.save(&f).unwrap();
    assert_eq!(
        Settings::load(&f).unwrap().slot(7).card_type(),
        CardType::GenericHdd
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn gh790_d1_and_h1_set_the_starting_directories_but_d2_and_h2_do_not() {
    // Disk.cpp:249 and Harddisk.cpp:370: only slot 6 drive 1 / slot 7 hard disk 1
    let (_, s) = apply(&["-d2", "/two/b.dsk", "-h2", "/hdtwo/b.hdv"]);
    assert_eq!(s.storage.disk_dir, None);
    assert_eq!(s.storage.hdv_dir, None);
    let (_, s) = apply(&[
        "-d1",
        "/one/a.dsk",
        "-d2",
        "/two/b.dsk",
        "-h1",
        "/hdone/a.hdv",
    ]);
    assert_eq!(s.storage.disk_dir, Some("/one".into()));
    assert_eq!(s.storage.hdv_dir, Some("/hdone".into()));
}

// ---- #398 #469 #689 #876 #1187: command-line switches ---------------------------------------

#[test]
fn gh398_h1_h2_start_with_images_in_the_hard_disk_slot() {
    let (c, s) = apply(&["-h1", "a.hdv", "-h2", "b.hdv"]);
    assert!(c.unsupported.is_empty());
    assert_eq!(s.slot(7).card_type(), CardType::GenericHdd);
    assert_eq!(hdd(&s, 7, 0), Some(abs("a.hdv")));
    assert_eq!(hdd(&s, 7, 1), Some(abs("b.hdv")));
    // CmdLine.cpp has no "-run": AppleWin lists it as unsupported, so do we
    assert_eq!(CmdLine::parse(&["-run"]).unwrap().unsupported, ["-run"]);
}

#[test]
fn gh469_no_printscreen_key_stops_capturing_the_key() {
    assert!(Settings::default().video.capture_printscreen_key);
    let (c, s) = apply(&["-no-printscreen-key"]);
    assert!(c.unsupported.is_empty());
    assert!(!s.video.capture_printscreen_key);
}

#[test]
fn gh689_model_and_clock_multiplier_on_the_command_line() {
    // CmdLine.cpp:656-673 accepts exactly these names; -clock-multiplier is the CPU speed
    for (name, m) in [
        ("apple2", Model::Apple2),
        ("apple2p", Model::Apple2Plus),
        ("apple2jp", Model::Apple2JPlus),
        ("apple2e", Model::Apple2e),
        ("apple2ee", Model::Apple2eEnhanced),
    ] {
        let (c, s) = apply(&["-model", name]);
        assert!(c.warnings.is_empty(), "{name}");
        assert_eq!(s.machine.model, m, "{name}");
    }
    let (c, s) = apply(&["-model", "pravets82"]);
    assert_eq!(c.warnings, ["-model: unsupported type: pravets82"]);
    assert_eq!(s.machine.model, Settings::default().machine.model);
    let (_, s) = apply(&["-clock-multiplier", "2.5"]);
    assert_eq!(s.machine.clock_multiplier, Some(2.5));
}

#[test]
fn gh876_fs_height_implies_full_screen_unless_no_full_screen_is_given_in_either_order() {
    // CmdLine.cpp:405-443: "-fs-height" sets full-screen only when nothing else said so
    let (_, s) = apply(&["-fs-height=768"]);
    assert!(s.video.fullscreen && s.video.fullscreen_height == Some(768));
    let (_, s) = apply(&["-fs-height=768", "-no-full-screen"]);
    assert!(!s.video.fullscreen && s.video.fullscreen_height == Some(768));
    let (_, s) = apply(&["-no-full-screen", "-fs-height=768"]);
    assert!(!s.video.fullscreen && s.video.fullscreen_height == Some(768));
    let (_, s) = apply(&["-fs-height=best", "-no-full-screen"]);
    assert!(!s.video.fullscreen && s.video.fullscreen_best_resolution);
}

#[test]
fn gh1187_capslock_off_at_startup() {
    assert!(Settings::default().keyboard.caps_lock);
    let (c, s) = apply(&["-capslock=off"]);
    assert!(c.unsupported.is_empty());
    assert!(!s.keyboard.caps_lock);
}

// ---- #709 #446: configuration without the registry (-conf / ini) ------------------------------

#[test]
fn gh709_gh446_conf_ini_replaces_the_registry() {
    let c = CmdLine::parse(&["-conf", "portable/applewin.ini", "-d1", "a.dsk"]).unwrap();
    assert_eq!(c.conf_file, Some("portable/applewin.ini".into()));
    // the same values a .reg file would carry, written as the ini of Registry.cpp:34-55
    let ini = "[Configuration]\nApple2 Type=1\nCPU Type=1\nEnhance Disk Speed=0\n\
               Confirm Reboot=0\nSave State On Exit=1\n[Configuration\\Slot 6]\nCard type=1\n\
               Last Disk Image 1=/games/a.dsk\n[Preferences]\nStarting Directory=/games/\n";
    let s = import_ini(ini);
    assert_eq!(s.machine.model, Model::Apple2Plus);
    assert!(
        !s.storage.enhance_disk_speed && !s.misc.confirm_reboot && s.storage.save_state_on_exit
    );
    assert_eq!(floppy(&s, 6, 0), Some("/games/a.dsk".into()));
    assert_eq!(s.storage.disk_dir, Some("/games/".into()));
    // a script can write that file with a few lines and no registry access
    assert_eq!(
        import_ini("").machine.model,
        Settings::default().machine.model
    );
}

// ---- #980: "Enhanced disk access speed" is remembered ----------------------------------------

#[test]
fn gh980_enhance_disk_speed_is_remembered() {
    // Utilities.cpp:327-330: default on, anything but 0 means on
    assert!(Settings::default().storage.enhance_disk_speed);
    assert!(
        !import_ini("[Configuration]\nEnhance Disk Speed=0\n")
            .storage
            .enhance_disk_speed
    );
    assert!(
        import_ini("[Configuration]\nEnhance Disk Speed=1\n")
            .storage
            .enhance_disk_speed
    );
    let d = tmp("gh980");
    let mut s = Settings::default();
    s.storage.enhance_disk_speed = false;
    let f = d.join("settings.toml");
    s.save(&f).unwrap();
    assert!(!Settings::load(&f).unwrap().storage.enhance_disk_speed);
    let _ = std::fs::remove_dir_all(d);
}

// ---- #183: F2 reboot confirmation setting -----------------------------------------------------

#[test]
fn gh183_confirm_reboot_defaults_on_and_is_remembered() {
    // FrameBase.h:5 kConfirmReboot_Default = true; PageConfig.cpp:319-323 saves it
    assert!(Settings::default().misc.confirm_reboot);
    assert!(
        !import_ini("[Configuration]\nConfirm Reboot=0\n")
            .misc
            .confirm_reboot
    );
}

// ---- #155 #975 #977 #1132 #1015: per-slot configuration ---------------------------------------

#[test]
fn gh155_gh975_gh977_every_slot_has_its_own_card_configuration() {
    // CmdLine.cpp:196-327: -sN <card> for slots 1-7, each with its own sub-options
    let (_, s) = apply(&[
        "-s5", "diskii", "-s5d2", "x.dsk", "-s7", "hdc-sp", "-s7h1", "h.hdv",
    ]);
    assert_eq!(s.slot(5).card_type(), CardType::Disk2);
    assert_eq!(floppy(&s, 5, 1), Some(abs("x.dsk")));
    assert_eq!(floppy(&s, 6, 1), None, "slot 6 is a separate card");
    assert_eq!(hdd(&s, 7, 0), Some(abs("h.hdv")));
    // TOML keeps a table per slot, so slot 5 is stored apart from slot 6 (GH#977's hierarchy)
    let t = s.to_toml().unwrap();
    assert_eq!(Settings::from_toml(&t).unwrap().slot(5), s.slot(5));
    // the images of a second Disk II go into its own section of the ini, not 'Preferences'
    let imp = import_ini("[Configuration\\Slot 5]\nCard type=1\nLast Disk Image 1=/s5.dsk\n");
    assert_eq!(floppy(&imp, 5, 0), Some("/s5.dsk".into()));
    // the command line only changes the in-memory copy: a run that never saves leaves the
    // stored configuration (and thus slot 5) alone (GH#975)
    let stored = Settings::default();
    let mut run = stored.clone();
    CmdLine::parse(&["-s5", "diskii"]).unwrap().apply(&mut run);
    assert_eq!(run.slot(5).card_type(), CardType::Disk2);
    assert_eq!(stored.slot(5).card_type(), CardType::Empty);
}

#[test]
fn gh1132_ssc_can_be_put_back_with_s2_ssc() {
    let mut s = Settings::default();
    CmdLine::parse(&["-s2", "empty"]).unwrap().apply(&mut s);
    assert_eq!(s.slot(2).card_type(), CardType::Empty);
    CmdLine::parse(&["-s2", "ssc"]).unwrap().apply(&mut s);
    assert_eq!(s.slot(2).card_type(), CardType::Ssc);
}

#[test]
fn gh1015_legacy_registry_without_slot_sections_still_loads_every_card() {
    // Utilities.cpp:235-257: new install or AppleWin <= 1.30.3 (no "Card type" values)
    let s =
        import_ini("[Configuration]\nHarddisk Enable=1\nSlot 4=3\nSlot 5=1\nUthernet Active=1\n");
    assert_eq!(s.slot(7).card_type(), CardType::GenericHdd);
    assert_eq!(s.slot(4).card_type(), CardType::MockingboardC);
    assert_eq!(s.slot(5).card_type(), CardType::Disk2);
    assert_eq!(s.slot(3).card_type(), CardType::Uthernet);
    // and a card in a slot the dialog would reject does not stop the import
    let odd =
        import_ini("[Configuration\\Slot 0]\nCard type=5\n[Configuration\\Slot 6]\nCard type=99\n");
    assert_eq!(odd.slot(0).card_type(), CardType::GenericHdd);
    assert_eq!(
        odd.slot(6).card_type(),
        CardType::Disk2,
        "unknown type: default of the slot"
    );
}

// ---- #732: -s0 empty takes the Language Card out of a ][ / ][+ ---------------------------------

fn rom_ii() -> Vec<u8> {
    let mut r = vec![0xEAu8; 0x3000];
    r[0x2FFC] = 0x00;
    r[0x2FFD] = 0x03;
    r
}

/// Writes $5A to $D000 through the Language Card sequence and reads it back.
fn lc_ram_answers(s: &Settings) -> bool {
    let mut cfg = s.to_machine_config();
    cfg.custom_rom = Some(rom_ii());
    let mut m = Machine::new(cfg);
    let b = m.board_mut();
    b.read(0xC081);
    b.read(0xC081); // two odd reads: write-enable RAM
    b.write(0xD000, 0x5A);
    b.read(0xC080); // read RAM
    b.read(0xD000) == 0x5A
}

#[test]
fn gh732_s0_empty_removes_the_language_card_from_the_apple_ii_plus() {
    let (c, with_lc) = apply(&["-model", "apple2p"]);
    assert!(c.unsupported.is_empty());
    assert!(
        lc_ram_answers(&with_lc),
        "][+ has a Language Card by default"
    );
    let (_, without) = apply(&["-model", "apple2p", "-s0", "empty"]);
    assert_eq!(without.slot(0).card_type(), CardType::Empty);
    assert!(
        !lc_ram_answers(&without),
        "no card: $D000 stays ROM (CmdLine.cpp:177-180)"
    );
}

// ---- #771 #45 #629 #773 #806: custom ROMs / more models ---------------------------------------

#[test]
fn gh771_gh45_gh773_custom_rom_options_reach_the_settings_and_j_plus_has_its_own_rom() {
    let (c, s) = apply(&[
        "-model",
        "apple2jp",
        "-rom",
        "IIJPLUS.ROM",
        "-videorom",
        "JVIDEO.ROM",
    ]);
    assert!(c.unsupported.is_empty() && c.warnings.is_empty());
    assert_eq!(s.machine.model, Model::Apple2JPlus);
    assert_eq!(s.machine.custom_rom, Some("IIJPLUS.ROM".into()));
    assert_eq!(s.machine.video_rom, Some("JVIDEO.ROM".into()));
    assert_eq!(Model::Apple2JPlus.system_rom().unwrap().len(), 12 * 1024);
    // Base 64A (GH#806): the 48K bank-switched ROM of Memory.cpp:2063
    assert_eq!(Model::Base64A.system_rom().unwrap().len(), 48 * 1024);
}

// ---- #318 #415: Pravets models ---------------------------------------------------------------

#[test]
fn gh318_gh415_pravets_82_and_8m_are_apple_ii_clones_and_only_8a_is_a_iie_clone() {
    // Common.h:197-209
    for m in [Model::Pravets82, Model::Pravets8M] {
        assert!(
            m.is_clone() && m.is_apple2() && !m.is_apple2e_family(),
            "{m:?}"
        );
    }
    assert!(Model::Pravets8A.is_clone() && Model::Pravets8A.is_apple2e_family());
    // GH#415: Pravets 82 has a value of its own (0x102), distinct from the generic "clone" 0x100
    assert_eq!(Model::Pravets82.value(), 0x102);
    assert_eq!(Model::try_from(0x102), Ok(Model::Pravets82));
    assert_ne!(Model::Pravets82.value(), 0x100);
    // Utilities.cpp:106-111: old wrongly tagged values are remapped on load
    assert_eq!(
        import_ini("[Configuration]\nApple2 Type=258\n")
            .machine
            .model,
        Model::Pravets82
    );
    assert_eq!(
        import_ini("[Configuration]\nApple2 Type=272\n")
            .machine
            .model,
        Model::Pravets82
    );
    assert_eq!(
        import_ini("[Configuration]\nApple2 Type=256\n")
            .machine
            .model,
        Model::Pravets82
    );
    assert_eq!(
        import_ini("[Configuration]\nApple2 Type=273\n")
            .machine
            .model,
        Model::Pravets8M
    );
    // and it is saved and reloaded as itself
    let mut s = Settings::default();
    s.machine.model = Model::Pravets82;
    assert_eq!(
        Settings::from_toml(&s.to_toml().unwrap())
            .unwrap()
            .machine
            .model,
        Model::Pravets82
    );
}
