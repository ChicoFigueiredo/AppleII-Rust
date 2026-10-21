//! Auditoria das issues upstream de categoria `config`/`outra` que tocam a interface (P23).
//! Oraculo: AppleWin a8f0d1d (WinFrame.cpp, Configuration/*.cpp). A interface e desenhada com
//! egui sem janela: cada teste roda quadros, le o texto desenhado e clica nos botoes.
//! Resultados em `docs/conformance/results/p23-config-outras.csv`.

use apple2_config::{CardType, Settings, SlotCardSpec};
use apple2_rust_shell::emu::EmuStatus;
use apple2_rust_shell::settings::model::{needs_restart, restart_reasons, slot_choices};
use apple2_rust_shell::settings::{ApplyPlan, SettingsDialog};
use apple2_rust_shell::ui::{UiEvent, UiState};
use egui::{Context, Event, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};

/// Runs one frame with `events`; returns every text drawn, with its screen rectangle, in paint
/// order (windows drawn later come later).
fn frame(ctx: &Context, events: Vec<Event>, ui: impl FnMut(&Context)) -> Vec<(String, Rect)> {
    frame_sized(ctx, events, 900.0, ui)
}

/// [`frame`] on a screen of the given height (a tall one shows the whole page, no scrolling).
fn frame_sized(
    ctx: &Context,
    events: Vec<Event>,
    height: f32,
    mut ui: impl FnMut(&Context),
) -> Vec<(String, Rect)> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, height))),
        events,
        ..Default::default()
    };
    let out = ctx.run(input, |c| ui(c));
    let mut texts = Vec::new();
    fn walk(s: &Shape, texts: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => texts.push((
                t.galley.text().to_string(),
                Rect::from_min_size(t.pos, t.galley.size()),
            )),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
    for cs in &out.shapes {
        walk(&cs.shape, &mut texts);
    }
    texts
}

/// Lets the windows lay themselves out (egui sizes a new window on its first frames).
fn warm_up(ctx: &Context, ui: &mut impl FnMut(&Context)) -> Vec<(String, Rect)> {
    let mut t = Vec::new();
    for _ in 0..4 {
        t = frame(ctx, vec![], &mut *ui);
    }
    t
}

/// Presses and releases the primary button on the last text equal to `label`.
fn click_last(ctx: &Context, texts: &[(String, Rect)], label: &str, ui: &mut impl FnMut(&Context)) {
    let r = texts
        .iter()
        .rev()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| {
            panic!(
                "no {label:?} on screen: {:?}",
                texts.iter().map(|t| &t.0).collect::<Vec<_>>()
            )
        })
        .1;
    let pos = r.center();
    let btn = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, vec![Event::PointerMoved(pos)], &mut *ui);
    frame(ctx, vec![btn(true)], &mut *ui);
    frame(ctx, vec![btn(false)], &mut *ui);
    frame(ctx, vec![], &mut *ui);
}

fn has(texts: &[(String, Rect)], needle: &str) -> bool {
    texts.iter().any(|(t, _)| t.contains(needle))
}

// ---- #183: F2 asks before it reboots, unless "Confirm reboot" is off ---------------------------

#[test]
fn gh183_reboot_asks_first_and_yes_confirms_no_cancels() {
    use std::cell::RefCell;
    let status = EmuStatus::default();
    let events: RefCell<Vec<UiEvent>> = RefCell::new(Vec::new());
    // "Confirm reboot" off: reboot at once, nothing on screen (WinFrame.cpp:1999)
    let ctx = Context::default();
    let mut ui = UiState::new(&Settings::default());
    assert!(ui.request_reboot(false));
    let mut draw = |c: &Context| events.borrow_mut().extend(ui.draw(c, &status, false));
    let t = warm_up(&ctx, &mut draw);
    assert!(!has(&t, "Are you sure you want to reboot?"));
    // default (on): the question appears and nothing reboots yet (WinFrame.cpp:2001-2012)
    let ctx = Context::default();
    let mut ui = UiState::new(&Settings::default());
    assert!(!ui.request_reboot(Settings::default().misc.confirm_reboot));
    let mut draw = |c: &Context| events.borrow_mut().extend(ui.draw(c, &status, false));
    let t = warm_up(&ctx, &mut draw);
    assert!(has(&t, "Are you sure you want to reboot?"), "{t:?}");
    assert!(events.borrow().is_empty());
    click_last(&ctx, &t, "Yes", &mut draw);
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, UiEvent::RebootConfirmed)),
        "{:?}",
        events.borrow()
    );
    // "No" closes the question without a reboot
    events.borrow_mut().clear();
    let ctx = Context::default();
    let mut ui = UiState::new(&Settings::default());
    ui.request_reboot(true);
    let mut draw = |c: &Context| events.borrow_mut().extend(ui.draw(c, &status, false));
    let t = warm_up(&ctx, &mut draw);
    click_last(&ctx, &t, "No", &mut draw);
    let t = frame(&ctx, vec![], &mut draw);
    assert!(!has(&t, "Are you sure you want to reboot?"));
    assert!(events.borrow().is_empty());
}

// ---- #546: changing the disk access speed needs no restart --------------------------------------

#[test]
fn gh546_enhance_disk_speed_applies_live_but_a_model_change_restarts() {
    // Config.cpp:267-289: CConfigNeedingRestart::operator== ignores m_enhanceDiskAccessSpeed
    let mut d = SettingsDialog::new(&Settings::default());
    let base = Settings::default();
    d.open_with(&base);
    d.draft.storage.enhance_disk_speed = !base.storage.enhance_disk_speed;
    match d.plan() {
        ApplyPlan::Live(s) => assert_eq!(
            s.storage.enhance_disk_speed,
            !base.storage.enhance_disk_speed
        ),
        other => panic!("{other:?}"),
    }
    let mut changed = base.clone();
    changed.storage.enhance_disk_speed = !base.storage.enhance_disk_speed;
    assert!(restart_reasons(&base, &changed).is_empty() && !needs_restart(&base, &changed));
    d.draft.machine.model = apple2_core::Model::Apple2Plus;
    assert!(matches!(d.plan(), ApplyPlan::Restart { .. }));
}

// ---- #1490: "Show status for slots 5 & 6" is global, not part of the Disk II card dialog -------

#[test]
fn gh1490_disk_status_checkbox_is_not_in_the_disk_ii_card_dialog() {
    let s = Settings::default();
    assert_eq!(s.slot(6).card_type(), CardType::Disk2);
    let ctx = Context::default();
    let mut dlg = SettingsDialog::new(&s);
    dlg.open_at(&s, "slots:6");
    let mut ev = Vec::new();
    let mut scale = false;
    let mut draw = |c: &Context| dlg.show(c, &mut scale, &mut ev);
    let t = warm_up(&ctx, &mut draw);
    assert!(
        has(&t, "Floppy Disk Drives"),
        "the card dialog is open: {t:?}"
    );
    assert!(!has(&t, "Show status for slots 5 & 6"), "{t:?}");
    // the global setting lives in the Configuration page
    let mut dlg = SettingsDialog::new(&s);
    dlg.open_at(&s, "Configuration");
    let mut draw = |c: &Context| dlg.show(c, &mut scale, &mut ev);
    let t = warm_up(&Context::default(), &mut draw);
    assert!(has(&t, "Show Disk II status in the toolbar"), "{t:?}");
}

// ---- #1502: cancelling the Super Serial Card dialog undoes its edits ---------------------------

#[test]
fn gh1502_cancel_in_the_ssc_dialog_restores_the_previous_options() {
    let s = Settings::default();
    assert_eq!(s.slot(2).card_type(), CardType::Ssc);
    let before = s.slot(2).clone();
    let ctx = Context::default();
    let mut dlg = SettingsDialog::new(&s);
    dlg.open_at(&s, "slots:2");
    let mut scale = false;
    let mut ev = Vec::new();
    let t = {
        let mut draw = |c: &Context| dlg.show(c, &mut scale, &mut ev);
        warm_up(&ctx, &mut draw)
    };
    assert!(has(&t, "Cancel"), "{t:?}");
    // the user picks TCP (an edit of the draft, as the widgets do every frame) ...
    dlg.draft.slots.0[2] = SlotCardSpec::Ssc {
        serial_port: String::new(),
        tcp_port: Some(1977),
        support_dcd: true,
    };
    assert_ne!(dlg.draft.slots.0[2], before);
    let mut draw = |c: &Context| dlg.show(c, &mut scale, &mut ev);
    let t = frame(&ctx, vec![], &mut draw);
    // ... and cancels the sub-dialog: the card is back as it was (PageSlots.cpp, GH#1502)
    click_last(&ctx, &t, "Cancel", &mut draw);
    assert_eq!(dlg.draft.slots.0[2], before);
}

// ---- #853: "Improve the Configuration GUI" (AppleWin 1.32.0.0) ---------------------------------

/// A lista exata do History.txt da 1.32.0.0 para a #853, uma linha por item:
/// página "Slots" com qualquer carta em cada slot; "Reset to Default" só dos slots; "Reset all to
/// Default" na página principal; caixa "No-Slot clock" na página Advanced; páginas "Sound" e
/// "Disk" removidas (a janela do a8f0d1d tem PageConfig, PageInput, PageSlots, PageAdvanced).
/// O port acrescenta uma aba Sound com os interruptores de som que o AppleWin só tem na linha de
/// comando; é uma extensão do hospedeiro documentada em docs/manual/12-diferencas-...md.
#[test]
fn gh853_configuration_gui_has_the_1_32_0_0_changes() {
    use apple2_rust_shell::settings::model::{reset_slots_to_default, set_slot_card};
    use apple2_rust_shell::settings::Page;
    let s = Settings::default();
    let mut scale = false;
    let mut ev = Vec::new();
    let mut page_text = |spec: &str| {
        let mut dlg = SettingsDialog::new(&s);
        dlg.open_at(&s, spec);
        let mut draw = |c: &Context| dlg.show(c, &mut scale, &mut ev);
        let ctx = Context::default();
        let mut t = Vec::new();
        for _ in 0..12 {
            t = frame_sized(&ctx, vec![], 3000.0, &mut draw);
        }
        t
    };
    // páginas: as quatro do AppleWin, nenhuma "Disk"
    let titles: Vec<&str> = Page::ALL.iter().map(|p| p.title()).collect();
    for want in ["Configuration", "Input", "Slots", "Advanced"] {
        assert!(titles.contains(&want), "{titles:?}");
    }
    assert!(!titles.contains(&"Disk"), "{titles:?}");
    // "Reset all to Default" na página principal
    let t = page_text("Configuration");
    assert!(has(&t, "Reset all to Default"), "{t:?}");
    // página Slots: os 8 slots e o "Reset to Default" próprio
    let t = page_text("Slots");
    for slot in 0..8 {
        assert!(has(&t, &format!("Slot {slot}:")), "slot {slot}: {t:?}");
    }
    assert!(has(&t, "Reset to Default"), "{t:?}");
    // "No-Slot clock" na página Advanced
    let t = page_text("Advanced");
    assert!(has(&t, "No-Slot clock"), "{t:?}");
    // qualquer carta em qualquer slot que o CardManager aceita, e o Reset to Default dos slots
    // volta às cartas de uma instalação nova sem tocar no resto da configuração
    let mut edited = Settings::default();
    edited.machine.no_slot_clock = true;
    let cur = apple2_rust_shell::settings::model::current_types(&edited);
    for slot in 0..8 {
        if let Some(choices) = slot_choices(slot, edited.machine.model, &cur) {
            for ty in choices {
                let mut x = edited.clone();
                set_slot_card(&mut x, slot, ty);
                assert_eq!(
                    apple2_rust_shell::settings::model::current_types(&x)[slot],
                    ty,
                    "slot {slot}"
                );
            }
        }
    }
    set_slot_card(&mut edited, 4, CardType::Empty);
    set_slot_card(&mut edited, 6, CardType::Empty);
    reset_slots_to_default(&mut edited);
    assert_eq!(
        apple2_rust_shell::settings::model::current_types(&edited),
        apple2_rust_shell::settings::model::current_types(&Settings::default())
    );
    assert!(edited.machine.no_slot_clock, "só os slots voltam ao padrão");
}

// ---- #975 #155: slot 5 can hold a Disk II from the configuration UI ----------------------------

#[test]
fn gh975_slot5_offers_a_disk_ii_in_the_configuration_ui() {
    let s = Settings::default();
    let cur = apple2_rust_shell::settings::model::current_types(&s);
    let choices = slot_choices(5, s.machine.model, &cur).unwrap();
    assert!(
        choices.contains(&CardType::Disk2) && choices.contains(&CardType::Empty),
        "{choices:?}"
    );
}

// ---- #98 #511 #960: the disks inserted while running survive a restart / the next run -------

#[test]
fn gh98_gh511_gh960_inserted_floppies_are_remembered_and_survive_a_restart() {
    use apple2_rust_shell::emu::Command;
    use apple2_rust_shell::hotswap::remember_media;
    let mut s = Settings::default();
    let floppy = |s: &Settings, d: usize| match s.slot(6) {
        SlotCardSpec::Disk2 { drives, .. } => drives[d].image.clone(),
        _ => unreachable!(),
    };
    let insert = |drive, p: &str| Command::InsertDisk {
        drive,
        path: p.into(),
        read_only: false,
    };
    // two disks from two folders (GH#511): both stay, whatever order they went in
    assert!(remember_media(&mut s, &insert(0, "/A1/a.dsk")));
    assert!(remember_media(&mut s, &insert(1, "/B/b.dsk")));
    assert_eq!(
        (floppy(&s, 0), floppy(&s, 1)),
        (Some("/A1/a.dsk".into()), Some("/B/b.dsk".into()))
    );
    // the configuration dialog opens on these settings: a hardware change then restarts the
    // machine from a draft that still holds the images (GH#98)
    let mut d = SettingsDialog::new(&s);
    d.open_with(&s);
    d.draft.machine.model = apple2_core::Model::Apple2Plus;
    match d.plan() {
        ApplyPlan::Restart { settings, .. } => {
            let kept = match settings.slot(6) {
                SlotCardSpec::Disk2 { drives, .. } => {
                    (drives[0].image.clone(), drives[1].image.clone())
                }
                _ => unreachable!(),
            };
            assert_eq!(kept, (Some("/A1/a.dsk".into()), Some("/B/b.dsk".into())));
        }
        other => panic!("{other:?}"),
    }
    // drop / eject / swap follow too; a command that touches no floppy changes nothing
    assert!(remember_media(&mut s, &Command::SwapDrives));
    assert_eq!(floppy(&s, 0), Some("/B/b.dsk".into()));
    assert!(remember_media(&mut s, &Command::EjectDisk { drive: 0 }));
    assert_eq!(
        (floppy(&s, 0), floppy(&s, 1)),
        (None, Some("/A1/a.dsk".into()))
    );
    let drop = Command::DropDisk {
        drive: 0,
        path: "/C/c.dsk".into(),
        reboot: true,
    };
    assert!(remember_media(&mut s, &drop));
    assert_eq!(floppy(&s, 0), Some("/C/c.dsk".into()));
    assert!(!remember_media(&mut s, &Command::PowerCycle));
}
