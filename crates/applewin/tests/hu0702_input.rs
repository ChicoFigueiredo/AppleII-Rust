//! HU-0702 / HU-0802: the input path without a window (AUD-P-028, 017, 018), the hot swap
//! contract (AUD-P-048), read-only mount (042), drop target (041), sector monitor (043) and the
//! volume (045). Oracle: AppleWin a8f0d1d.

use apple2_config::settings::Joystick0Kind;
use apple2_config::{CardType, Settings, SlotCardSpec};
use applewin_shell::actions::Action;
use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};
use applewin_shell::input::{releases_keys, InputRouter, KeyCtx, Routed};
use applewin_shell::keymap::{HostKey, KeyInput, Mods};

fn emulator(f: impl FnOnce(&mut Settings)) -> Emulator {
    let mut s = Settings::default();
    s.machine.model = apple2_core::Model::Apple2eEnhanced;
    s.slots.0[6] = SlotCardSpec::Empty;
    f(&mut s);
    Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap()
}

/// Runs `LDA addr` on the CPU and returns the byte it loaded (a real, side-effecting access).
fn read_io(e: &mut Emulator, addr: u16) -> u8 {
    let code = [0xAD, addr as u8, (addr >> 8) as u8, 0x8D, 0x00, 0x02];
    for (i, b) in code.iter().enumerate() {
        e.machine.poke_main(0x2E0 + i as u16, *b);
    }
    e.machine.cpu_mut().pc = 0x2E0;
    e.machine.step();
    e.machine.step();
    e.machine.peek_main(0x200)
}

fn feed(e: &mut Emulator, routed: Vec<Routed>) {
    for r in routed {
        if let Routed::Cmd(c) = r {
            e.apply(c);
        }
    }
}

fn key(c: char) -> KeyInput {
    KeyInput::new(HostKey::Char(c))
}

fn numpad(c: char) -> KeyInput {
    KeyInput::numpad(HostKey::Char(c))
}

fn ctx() -> KeyCtx {
    KeyCtx::default()
}

fn any_key_down(e: &mut Emulator) -> bool {
    read_io(e, 0xC010) & 0x80 != 0
}

// ------------------------------------------------------------------ AUD-P-028

fn numpad_router() -> InputRouter {
    let mut s = Settings::default();
    s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
    InputRouter::new(&s)
}

/// The report of the finding: hold a key, press F7, let go, leave the debugger. The key press
/// reaches the Apple; F7 lets go of everything (`releases_keys`), so the late release is harmless
/// and "any key down" is false.
#[test]
fn aud_p_028_key_held_while_entering_the_debugger_does_not_stay_down() {
    let mut e = emulator(|_| {});
    let mut r = numpad_router();
    feed(&mut e, r.key(key('a'), true, false, ctx(), 0));
    assert!(any_key_down(&mut e), "a key is down");

    // F7: the app's ToggleDebugger action
    assert!(releases_keys(Action::ToggleDebugger));
    for c in r.release_all(1) {
        e.apply(c);
    }
    e.apply(Command::ToggleDebugger);
    let dbg = KeyCtx {
        debugger_active: true,
        ..ctx()
    };
    // the release arrives while the debugger owns the keyboard
    feed(&mut e, r.key(key('a'), false, false, dbg, 2));
    e.apply(Command::ToggleDebugger); // F7 again: back to the emulation
    assert!(!any_key_down(&mut e), "AKD stuck after the debugger");
}

/// Second defence: even without the F7 clean-up, a release that arrives while the debugger (or
/// an egui widget) has the keyboard still reaches `KeyboardState`.
#[test]
fn aud_p_028_release_reaches_the_keyboard_when_the_debugger_or_egui_owns_it() {
    for (debugger_active, ui_wants_keys) in [(true, false), (false, true)] {
        let mut e = emulator(|_| {});
        let mut r = numpad_router();
        feed(&mut e, r.key(key('a'), true, false, ctx(), 0));
        assert!(any_key_down(&mut e));
        let owned = KeyCtx {
            debugger_active,
            ui_wants_keys,
            ..ctx()
        };
        feed(&mut e, r.key(key('a'), false, false, owned, 1));
        assert!(!any_key_down(&mut e), "{debugger_active} {ui_wants_keys}");
        // and a press while owned does not type
        feed(&mut e, r.key(key('b'), true, false, owned, 2));
        assert!(!any_key_down(&mut e));
    }
}

/// The keyboard joystick comes back to the centre the same way.
#[test]
fn aud_p_028_keyboard_joystick_is_centred_after_the_debugger() {
    let mut e = emulator(|_| {});
    let mut r = numpad_router();
    feed(&mut e, r.key(numpad('4'), true, false, ctx(), 0)); // left
    assert_eq!(e.machine.board().game.paddles[0], 0);
    for c in r.release_all(1) {
        e.apply(c);
    }
    assert_eq!(e.machine.board().game.paddles[0], 127, "F7 centres it");

    // without the clean-up, the release in the debugger centres it too (centering mode)
    let mut s = Settings::default();
    s.joystick.joystick0 = Joystick0Kind::KeyboardNumpad;
    s.joystick.centering = true;
    let mut r = InputRouter::new(&s);
    feed(&mut e, r.key(numpad('6'), true, false, ctx(), 0));
    assert_eq!(e.machine.board().game.paddles[0], 255);
    let dbg = KeyCtx {
        debugger_active: true,
        ..ctx()
    };
    feed(&mut e, r.key(numpad('6'), false, false, dbg, 1));
    assert_eq!(e.machine.board().game.paddles[0], 127);
}

/// Focus lost and the full-screen toggle also let go (`release_inputs` / `releases_keys`).
#[test]
fn aud_p_028_fullscreen_and_focus_loss_release_everything() {
    assert!(releases_keys(Action::ToggleFullscreen));
    assert!(!releases_keys(Action::Reboot));
    let mut e = emulator(|_| {});
    let mut r = numpad_router();
    feed(&mut e, r.key(key('a'), true, false, ctx(), 0));
    feed(&mut e, r.key(numpad('8'), true, false, ctx(), 0));
    feed(
        &mut e,
        r.key(KeyInput::new(HostKey::AltLeft), true, false, ctx(), 0),
    );
    assert!(any_key_down(&mut e));
    assert!(e.machine.board().game.button(0, false), "Open Apple down");
    for c in r.release_all(1) {
        e.apply(c);
    }
    assert!(!any_key_down(&mut e));
    assert_eq!(e.machine.board().game.paddles[1], 127);
    assert!(!e.machine.board().game.button(0, false));
}

// ------------------------------------------------------------------ AUD-P-017 / 018

/// Reads 16 bits of the SNES MAX in slot `slot` with a real 6502 program (latch, then
/// read + clock); returns the 16 bytes read.
fn read_snes(e: &mut Emulator, slot: u8) -> Vec<u8> {
    let io = 0xC080 + u16::from(slot) * 16;
    let (lo, hi) = (io as u8, (io >> 8) as u8);
    #[rustfmt::skip]
    let code = [
        0xA9, 0x00,                 // LDA #0
        0x8D, lo, hi,               // STA io      latch
        0xA0, 0x00,                 // LDY #0
        0xAD, lo, hi,               // loop: LDA io
        0x99, 0x00, 0x02,           // STA $0200,Y
        0x8D, lo + 1, hi,           // STA io+1    clock
        0xC8,                       // INY
        0xC0, 0x10,                 // CPY #16
        0xD0, 0xF2,                 // BNE loop
        0x4C, 0x16, 0x03,           // JMP *
    ];
    for (i, b) in code.iter().enumerate() {
        e.machine.poke_main(0x300 + i as u16, *b);
    }
    e.machine.cpu_mut().pc = 0x300;
    e.machine.run_cycles(3000);
    (0..16).map(|i| e.machine.peek_main(0x200 + i)).collect()
}

fn pad_router() -> InputRouter {
    InputRouter::new(&Settings::default())
}

fn send(e: &mut Emulator, cmds: Vec<Command>) {
    for c in cmds {
        e.apply(c);
    }
}

/// A simulated gamepad (gilrs button / axis events on `PadState`) reaches the SNES MAX card and
/// the program reads the direction and buttons back, active low, controller 2 unplugged.
#[test]
fn aud_p_017_gamepad_reaches_the_snes_max_and_the_program_reads_it() {
    use apple2_cards::snesmax::buttons as b;
    let mut e = emulator(|s| s.slots.0[4] = SlotCardSpec::new(CardType::SnesMax));
    let mut r = pad_router();
    r.joy.set_pads_connected(1);
    r.joy.pads[0].button(gilrs::Button::DPadUp, true);
    r.joy.pads[0].button(gilrs::Button::South, true); // B
    r.joy.pads[0].button(gilrs::Button::Start, true);
    let cmds = r.update(0);
    assert!(cmds.iter().any(|c| matches!(c, Command::HostPads(_))));
    send(&mut e, cmds);
    let bytes = read_snes(&mut e, 4);
    let pressed = |bit: u32| bytes[bit.trailing_zeros() as usize] & 0x80 == 0;
    assert!(pressed(b::UP) && pressed(b::B) && pressed(b::START));
    assert!(!pressed(b::DOWN) && !pressed(b::A) && !pressed(b::SELECT));
    assert!(
        bytes.iter().all(|v| v & 0x40 != 0),
        "controller 2 is not plugged: all ones"
    );

    // an analog stick to the left is LEFT (axis < 103 of 255); a second pad is controller 2
    r.joy.pads[0].button(gilrs::Button::DPadUp, false);
    r.joy.pads[0].axis(gilrs::Axis::LeftStickX, -1.0);
    r.joy.set_pads_connected(2);
    r.joy.pads[1].button(gilrs::Button::East, true); // A
    send(&mut e, r.update(1));
    let bytes = read_snes(&mut e, 4);
    assert_eq!(bytes[b::LEFT.trailing_zeros() as usize] & 0x80, 0);
    assert_ne!(
        bytes[b::UP.trailing_zeros() as usize] & 0x80,
        0,
        "UP released"
    );
    assert_eq!(
        bytes[b::A.trailing_zeros() as usize] & 0x40,
        0,
        "controller 2: A"
    );
    assert_ne!(bytes[b::B.trailing_zeros() as usize] & 0x40, 0);

    // unplugging both pads: nothing pressed any more, nothing plugged
    r.joy.set_pads_connected(0);
    send(&mut e, r.update(2));
    assert!(read_snes(&mut e, 4).iter().all(|v| v & 0xC0 == 0xC0));
}

/// The host controls survive a rebuild of the machine (new cards): the app asks the router to
/// send everything again and the emulator keeps the last pads.
#[test]
fn aud_p_017_host_pads_survive_a_settings_rebuild() {
    use apple2_cards::snesmax::buttons as b;
    let mut e = emulator(|s| s.slots.0[4] = SlotCardSpec::new(CardType::SnesMax));
    let mut r = pad_router();
    r.joy.set_pads_connected(1);
    r.joy.pads[0].button(gilrs::Button::DPadDown, true);
    send(&mut e, r.update(0));
    let mut s = e.settings.clone();
    s.sound.master_volume = 10;
    e.apply(Command::ApplySettings(Box::new(s)));
    let bytes = read_snes(&mut e, 4);
    assert_eq!(bytes[b::DOWN.trailing_zeros() as usize] & 0x80, 0);
}

/// 4Play: ports 1-2 are the host pads, ports 3-4 the E/S/D/F + Z/X and I/J/K/L + N/M keys, which
/// also type on the Apple (FourPlay.cpp polls the keys without consuming them).
#[test]
fn aud_p_018_four_play_gets_pads_and_keys() {
    let mut e = emulator(|s| s.slots.0[4] = SlotCardSpec::new(CardType::FourPlay));
    let mut r = pad_router();
    r.joy.set_pads_connected(2);
    r.joy.pads[0].button(gilrs::Button::DPadRight, true);
    r.joy.pads[0].button(gilrs::Button::South, true); // trigger 1
    r.joy.pads[1].axis(gilrs::Axis::LeftStickY, 1.0); // gilrs +Y is up
    send(&mut e, r.update(0));
    let io = 0xC0C0;
    assert_eq!(read_io(&mut e, io) & 0x0F, 0x08, "port 1: right");
    assert_ne!(read_io(&mut e, io) & 0x80, 0, "port 1: trigger 1");
    assert_eq!(read_io(&mut e, io + 1) & 0x0F, 0x01, "port 2: up");
    assert_eq!(
        read_io(&mut e, io + 2),
        0x20,
        "port 3 idle (bit 5 always high)"
    );

    // E = port 3 up, X = port 3 trigger 2, K = port 4 down
    feed(&mut e, r.key(key('e'), true, false, ctx(), 1));
    feed(&mut e, r.key(key('x'), true, false, ctx(), 1));
    feed(&mut e, r.key(key('K'), true, false, ctx(), 1));
    assert_eq!(read_io(&mut e, io + 2) & 0x4F, 0x41);
    assert_eq!(read_io(&mut e, io + 3) & 0x0F, 0x02);
    assert!(any_key_down(&mut e), "the keys still type on the Apple");
    feed(&mut e, r.key(key('e'), false, false, ctx(), 2));
    assert_eq!(read_io(&mut e, io + 2) & 0x0F, 0x00);
    // the F7 clean-up lets go of them too
    send(&mut e, r.release_all(3));
    assert_eq!(read_io(&mut e, io + 2), 0x20);
    assert_eq!(read_io(&mut e, io + 3), 0x20);
}

// ------------------------------------------------------------------ AUD-P-048

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("applewin-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Hot swap contract: a hard disk image is plugged into the running machine; the RAM is kept
/// (no restart, no reboot). Cards and the model need a restart, asked first (settings tests).
#[test]
fn aud_p_048_hdd_image_is_hot_plugged_without_losing_ram() {
    use apple2_storage::hdd::HardDiskCard;
    let dir = temp_dir("hdd-hot");
    let img = dir.join("a.hdv");
    std::fs::write(&img, vec![0u8; 512 * 280]).unwrap();
    let mut e = emulator(|s| s.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd));
    e.machine.poke_main(0x1234, 0x77);
    assert!(e
        .machine
        .card::<HardDiskCard>(7)
        .unwrap()
        .image(0)
        .is_none());
    e.apply(Command::SetHdd {
        slot: 7,
        unit: 0,
        image: Some(img.clone()),
    });
    assert!(
        e.machine
            .card::<HardDiskCard>(7)
            .unwrap()
            .image(0)
            .is_some(),
        "{}",
        e.status().message
    );
    assert_eq!(e.machine.peek_main(0x1234), 0x77, "RAM untouched");
    e.apply(Command::SetHdd {
        slot: 7,
        unit: 0,
        image: None,
    });
    assert!(e
        .machine
        .card::<HardDiskCard>(7)
        .unwrap()
        .image(0)
        .is_none());
    assert_eq!(e.machine.peek_main(0x1234), 0x77);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The settings dialog treats an image change as a live apply (no restart prompt), a card
/// change as a restart that waits for confirmation.
#[test]
fn aud_p_048_dialog_plans_images_live_and_cards_with_a_prompt() {
    use applewin_shell::settings::{ApplyPlan, SettingsDialog};
    let mut base = Settings::default();
    base.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd);
    let mut d = SettingsDialog::new(&base);
    d.open_with(&base);
    if let SlotCardSpec::GenericHdd { images, .. } = &mut d.draft.slots.0[7] {
        images[1] = Some("x.hdv".into());
    }
    assert!(matches!(d.plan(), ApplyPlan::Live(_)));
    d.draft.slots.0[7] = SlotCardSpec::Empty;
    match d.plan() {
        ApplyPlan::Restart { reasons, .. } => assert!(!reasons.is_empty()),
        other => panic!("{other:?}"),
    }
}

// ------------------------------------------------------------------ AUD-P-041 / 042 / 043

fn floppy(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, vec![0u8; 143_360]).unwrap();
    p
}

fn with_disk2(s: &mut Settings) {
    s.slots.0[6] = SlotCardSpec::new(CardType::Disk2);
}

/// AUD-P-042: "Open as Read Only" mounts write-protected; a normal mount follows the file.
#[test]
fn aud_p_042_open_as_read_only_write_protects() {
    let dir = temp_dir("ro");
    let p = floppy(&dir, "a.dsk");
    let mut e = emulator(with_disk2);
    e.apply(Command::InsertDisk {
        drive: 0,
        path: p.clone(),
        read_only: false,
    });
    assert!(!e.status().drives[0].write_protected, "writable file");
    e.apply(Command::InsertDisk {
        drive: 0,
        path: p,
        read_only: true,
    });
    let st = e.status();
    assert!(st.drives[0].write_protected, "{}", st.message);
    let _ = std::fs::remove_dir_all(&dir);
}

/// AUD-P-041: a dropped disk goes to drive 1 and reboots unless it fell on the Drive 1 button;
/// on the Drive 2 button it goes to drive 2 (and reboots); a rejected disk does not reboot.
#[test]
fn aud_p_041_drop_target_and_reboot_like_applewin() {
    use applewin_shell::toolbar::{drop_target, DropTarget};
    let r1 = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 40.0));
    let r2 = egui::Rect::from_min_size(egui::pos2(0.0, 50.0), egui::vec2(40.0, 40.0));
    let rects = [Some(r1), Some(r2)];
    assert_eq!(
        drop_target(Some((10.0, 10.0)), rects),
        DropTarget {
            drive: 0,
            reboot: false
        }
    );
    assert_eq!(
        drop_target(Some((10.0, 60.0)), rects),
        DropTarget {
            drive: 1,
            reboot: true
        }
    );
    assert_eq!(
        drop_target(Some((300.0, 300.0)), rects),
        DropTarget {
            drive: 0,
            reboot: true
        }
    );
    assert_eq!(
        drop_target(None, [None, None]),
        DropTarget {
            drive: 0,
            reboot: true
        }
    );

    let dir = temp_dir("drop");
    let p = floppy(&dir, "d.dsk");
    let mut e = emulator(with_disk2);
    e.apply(Command::DropDisk {
        drive: 1,
        path: p.clone(),
        reboot: true,
    });
    let st = e.status();
    assert!(st.drives[1].image.is_some() && st.drives[0].image.is_none());
    assert_eq!(st.message, "Machine rebooted");
    // dropped on the Drive 1 button: no reboot
    e.apply(Command::DropDisk {
        drive: 0,
        path: p,
        reboot: false,
    });
    assert!(e.status().message.starts_with("Drive 1:"));
    // not a disk: AppleWin reports it and does not press Run
    let bad = dir.join("x.dsk");
    std::fs::write(&bad, b"nope").unwrap();
    let before = e.status().message_seq;
    e.apply(Command::DropDisk {
        drive: 0,
        path: bad,
        reboot: true,
    });
    let st = e.status();
    assert!(
        st.message_seq > before && st.message != "Machine rebooted",
        "{}",
        st.message
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AUD-P-043: the "S??" monitor shows the sector DOS 3.3 / ProDOS name for the controller, and
/// "S??" when they do not (AppleWin `GetTrackSector`); the value is kept per drive.
#[test]
fn aud_p_043_sector_monitor_follows_the_guest_os() {
    let mut e = emulator(with_disk2);
    e.run_frame();
    assert_eq!(e.status().sectors, [None, None], "S??");
    // DOS 3.3: slot*16 at $B7E9, track at $B7EC, sector at $B7ED
    e.machine.poke_main(0xB7E9, 0x60);
    e.machine.poke_main(0xB7EC, 5);
    e.machine.poke_main(0xB7ED, 11);
    e.run_frame();
    assert_eq!(e.status().sectors, [Some(11), None]);
    // another slot's RWTS, or an impossible sector: back to "??"
    e.machine.poke_main(0xB7ED, 16);
    e.run_frame();
    assert_eq!(e.status().sectors[0], None);
    // the text AppleWin-style
    let d = applewin_shell::backend::DriveStatus {
        present: true,
        image: Some("a.dsk".into()),
        track: 5,
        ..Default::default()
    };
    assert_eq!(
        applewin_shell::toolbar::drive_text(0, &d, Some(11)),
        "Drive 1: T 5 S11 a.dsk"
    );
}

// ------------------------------------------------------------------ AUD-P-045

/// The oracle (a8f0d1d) has ONE volume: "Master volume" feeds the speaker and the Mockingboard
/// alike (PageConfig.cpp:326-332, Utilities.cpp:283-287); "Speaker Volume" is the deprecated
/// older name of the same value. Nothing to add: the single slider is the conforming behaviour.
#[test]
fn aud_p_045_single_master_volume_like_the_oracle() {
    let mut e = emulator(|_| {});
    assert!((e.master_gain() - 1.0).abs() < 1e-6, "0 = loudest");
    e.apply(Command::SetVolume(20));
    let g = e.master_gain();
    assert!((g - apple2_audio::volume_gain(20, applewin_shell::emu::VOLUME_MAX)).abs() < 1e-9);
    assert!(g < 1.0);
    // the legacy key is read as the same setting
    let s = apple2_config::import::import_ini("[Configuration]\nSpeaker Volume=15\n");
    assert_eq!(s.sound.master_volume, 15);
    let s =
        apple2_config::import::import_ini("[Configuration]\nMaster volume=12\nSpeaker Volume=15\n");
    assert_eq!(s.sound.master_volume, 12);
}

// ------------------------------------------------------------------ AUD-P-047

/// The Advanced page button feeds `import_applewin_file` (the same importer as `-conf`).
#[test]
fn aud_p_047_import_button_reads_reg_and_ini() {
    use applewin_shell::settings::model::import_applewin_file;
    let dir = temp_dir("import");
    let ini = dir.join("AppleWin.ini");
    std::fs::write(&ini, "[Configuration]\nMaster volume=7\n").unwrap();
    let keep = Settings::default();
    assert_eq!(
        import_applewin_file(&ini, &keep)
            .unwrap()
            .sound
            .master_volume,
        7
    );
    let reg = dir.join("AppleWin.reg");
    std::fs::write(
        &reg,
        "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\AppleWin\\CurrentVersion\\Configuration]\r\n\"Master volume\"=dword:00000009\r\n",
    )
    .unwrap();
    assert_eq!(
        import_applewin_file(&reg, &keep)
            .unwrap()
            .sound
            .master_volume,
        9
    );
    assert!(import_applewin_file(&dir.join("nope.ini"), &keep).is_err());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = Mods::default();
}
