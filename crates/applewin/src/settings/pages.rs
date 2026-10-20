//! The five pages of the Settings dialog. Labels and order follow `Applewin.rc`.

use super::model::{self, ModelChoice};
use super::{Confirm, Page, SettingsDialog, Sub, AUX};
use crate::ui::UiEvent;
use apple2_config::settings::{CloneKeyboard, DongleType, Joystick0Kind, Joystick1Kind, VideoType};
use apple2_config::CardType;
use egui::Ui;
use std::path::PathBuf;

/// "Video" drop-down (AppleWin `g_aVideoChoices`).
pub const VIDEO_TYPES: [(VideoType, &str); 9] = [
    (VideoType::MonoCustom, "Monochrome (Custom)"),
    (VideoType::ColorIdealized, "Color (Composite Idealized)"),
    (VideoType::ColorVideocardRgb, "Color (RGB Card/Monitor)"),
    (VideoType::ColorMonitorNtsc, "Color (Composite Monitor)"),
    (VideoType::ColorTv, "Color TV"),
    (VideoType::MonoTv, "B&W TV"),
    (VideoType::MonoAmber, "Monochrome (Amber)"),
    (VideoType::MonoGreen, "Monochrome (Green)"),
    (VideoType::MonoWhite, "Monochrome (White)"),
];

/// "Game I/O Connector" drop-down (PageAdvanced.cpp:48).
pub const DONGLES: [(DongleType, &str); 7] = [
    (DongleType::Empty, "Empty"),
    (DongleType::SdsSpeedStar, "SDS DataKey - SpeedStar"),
    (DongleType::Codewriter, "Cortechs Corp - CodeWriter"),
    (DongleType::Robocom500, "Robocom Ltd - Robo 500"),
    (DongleType::Robocom1000, "Robocom Ltd - Robo 1000"),
    (DongleType::Robocom1500, "Robocom Ltd - Robo 1500, CAD-2P"),
    (DongleType::HaydenCompiler, "Hayden - Applesoft Compiler"),
];

/// Framed group spanning the full width.
pub fn group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        add(ui)
    })
    .inner
}

pub fn label_of<T: PartialEq + Copy>(items: &[(T, &'static str)], v: T) -> &'static str {
    items.iter().find(|(t, _)| *t == v).map_or("?", |(_, n)| *n)
}

/// Drop-down over `items`; true when the user picked something else.
pub fn combo<T: PartialEq + Copy>(ui: &mut Ui, id: &str, items: &[(T, &'static str)], value: &mut T) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(label_of(items, *value)).show_ui(ui, |ui| {
        for (t, name) in items {
            if ui.selectable_label(*value == *t, *name).clicked() && *value != *t {
                *value = *t;
                changed = true;
            }
        }
    });
    changed
}

pub fn browse_file(title: &str, filter: Option<(&str, &[&str])>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title);
    if let Some((name, exts)) = filter {
        d = d.add_filter(name, exts);
    }
    d.pick_file()
}

pub fn browse_save(title: &str, file_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).set_file_name(file_name).save_file()
}

/// Path display + Browse... + clear. Returns true on change.
pub fn path_row(ui: &mut Ui, title: &str, path: &mut Option<PathBuf>, filter: Option<(&str, &[&str])>) -> bool {
    let mut changed = false;
    let mut text = path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    if ui.add(egui::TextEdit::singleline(&mut text).desired_width(260.0).hint_text("(none)")).changed() {
        *path = (!text.is_empty()).then(|| PathBuf::from(&text));
        changed = true;
    }
    if ui.button("Browse...").clicked() {
        if let Some(p) = browse_file(title, filter) {
            *path = Some(p);
            changed = true;
        }
    }
    if path.is_some() && ui.button("Clear").clicked() {
        *path = None;
        changed = true;
    }
    changed
}

impl SettingsDialog {
    pub(super) fn draw_page(&mut self, ui: &mut Ui, integer_scale: &mut bool, ev: &mut Vec<UiEvent>) {
        match self.page {
            Page::Configuration => self.page_configuration(ui, integer_scale, ev),
            Page::Input => self.page_input(ui, ev),
            Page::Sound => self.page_sound(ui, ev),
            Page::Slots => self.page_slots(ui),
            Page::Advanced => self.page_advanced(ui, ev),
        }
    }

    fn volume_slider(&mut self, ui: &mut Ui, ev: &mut Vec<UiEvent>) {
        let mut pos = model::volume_to_slider(self.draft.sound.master_volume);
        if ui.add(egui::Slider::new(&mut pos, 0..=model::VOLUME_MAX).show_value(false)).changed() {
            self.draft.sound.master_volume = model::slider_to_volume(pos);
            ev.push(UiEvent::Volume(self.draft.sound.master_volume));
        }
    }

    // ---- Configuration -------------------------------------------------------------------

    fn page_configuration(&mut self, ui: &mut Ui, integer_scale: &mut bool, ev: &mut Vec<UiEvent>) {
        ui.horizontal(|ui| {
            ui.label("Model:");
            let mut choice = ModelChoice::of(self.draft.machine.model);
            let items: Vec<(ModelChoice, &'static str)> = ModelChoice::ALL.iter().map(|c| (*c, c.label())).collect();
            if combo(ui, "model", &items, &mut choice) {
                let m = model::model_for_choice(choice, self.draft.machine.model);
                model::set_model(&mut self.draft, m);
            }
            ui.checkbox(&mut self.draft.misc.confirm_reboot, "Confirm reboot");
        });
        ui.horizontal(|ui| {
            ui.label("Master Volume");
            self.volume_slider(ui, ev);
        });
        ui.add_space(4.0);

        group(ui, |ui| {
            ui.strong("Video");
            ui.horizontal(|ui| {
                ui.label("Mode:");
                combo(ui, "videotype", &VIDEO_TYPES, &mut self.draft.video.mode);
            });
            ui.horizontal(|ui| {
                let mono = self.draft.video.mode == VideoType::MonoCustom;
                ui.add_enabled_ui(mono, |ui| {
                    ui.label("Monochrome Color...");
                    let mut c = model::rgb_to_array(self.draft.video.monochrome_rgb);
                    if ui.color_edit_button_srgb(&mut c).changed() {
                        self.draft.video.monochrome_rgb = model::array_to_rgb(c);
                    }
                });
            });
            ui.checkbox(&mut self.draft.video.half_scanlines, "50% Scan lines");
            ui.checkbox(&mut self.draft.video.vertical_blend, "Vertical blend");
            let mut hz50 = self.draft.machine.refresh_rate == apple2_config::settings::RefreshRate::Hz50;
            if ui.checkbox(&mut hz50, "50Hz video").changed() {
                self.draft.machine.refresh_rate =
                    if hz50 { apple2_config::settings::RefreshRate::Hz50 } else { apple2_config::settings::RefreshRate::Hz60 };
            }
            ui.checkbox(&mut self.draft.video.fullscreen_show_subunit_status, "Full-Screen: Show drive/keyboard status");
            ui.checkbox(&mut self.draft.video.show_disk_leds, "Show Disk II status in the toolbar");
        });

        group(ui, |ui| {
            ui.strong("Window and full screen");
            ui.horizontal(|ui| {
                ui.label("Window scale");
                ui.add(egui::Slider::new(&mut self.draft.video.window_scale, 1..=4));
            });
            ui.checkbox(integer_scale, "Integer scaling");
            ui.checkbox(&mut self.draft.video.fullscreen, "Start in full-screen mode");
            ui.checkbox(&mut self.draft.video.fullscreen_best_resolution, "Full-screen: best resolution");
            ui.checkbox(&mut self.draft.video.multi_monitor, "Full-screen across all monitors");
            ui.checkbox(&mut self.draft.video.alt_enter_toggles_fullscreen, "Alt+Enter toggles full-screen");
        });

        group(ui, |ui| {
            ui.strong("Emulation Speed Control");
            ui.checkbox(&mut self.draft.storage.enhance_disk_speed, "Enhanced floppy disk access speed (all drives)");
            ui.checkbox(&mut self.draft.keyboard.scroll_lock_toggle, "Scroll Lock acts as toggle for full-speed CPU");
            ui.radio_value(&mut self.speed.authentic, true, "Use Authentic Machine Speed");
            ui.radio_value(&mut self.speed.authentic, false, "Select Custom Speed (in MHz)");
            ui.add_enabled_ui(!self.speed.authentic, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.speed.custom, model::SPEED_MIN..=model::SPEED_MAX)
                        .custom_formatter(|v, _| model::speed_label(v as u32))
                        .custom_parser(|s| s.trim().trim_end_matches("MHz").trim().parse::<f64>().ok().map(|v| v * 10.0)),
                );
            });
            ui.horizontal(|ui| {
                ui.weak("0.5");
                ui.weak("   1.0");
                ui.weak("   2.0");
                ui.weak("   ...   Fastest");
            });
        });

        if ui.button("Reset all to Default").clicked() {
            self.confirm = Some(Confirm::ResetAll);
        }
    }

    // ---- Input ---------------------------------------------------------------------------

    fn page_input(&mut self, ui: &mut Ui, ev: &mut Vec<UiEvent>) {
        let mouse_card = model::has_mouse_card(&self.draft);
        group(ui, |ui| {
            ui.strong("Joystick Control");
            let j0: Vec<(Joystick0Kind, &'static str)> =
                model::joystick0_choices(self.draft.joystick.joystick1, mouse_card).into_iter().map(|k| (k, model::joystick0_label(k))).collect();
            let j1: Vec<(Joystick1Kind, &'static str)> =
                model::joystick1_choices(self.draft.joystick.joystick0, mouse_card).into_iter().map(|k| (k, model::joystick1_label(k))).collect();
            ui.horizontal(|ui| {
                ui.label("Joystick 1:");
                combo(ui, "joy0", &j0, &mut self.draft.joystick.joystick0);
            });
            ui.horizontal(|ui| {
                ui.label("Joystick 2:");
                combo(ui, "joy1", &j1, &mut self.draft.joystick.joystick1);
            });
            ui.horizontal(|ui| {
                ui.label("X-trim:");
                ui.add(egui::DragValue::new(&mut self.draft.joystick.pdl_x_trim).range(-128..=127));
                ui.label("Y-trim:");
                ui.add(egui::DragValue::new(&mut self.draft.joystick.pdl_y_trim).range(-128..=127));
            });
            let cursors = model::cursor_control_enabled(&self.draft);
            let centering = model::centering_enabled(&self.draft);
            ui.add_enabled(cursors, egui::Checkbox::new(&mut self.draft.joystick.cursor_control, "Allow cursor keys to be read from keyboard"));
            ui.checkbox(&mut self.draft.joystick.swap_buttons, "Swap 0/1");
            let mut af = model::autofire_all(self.draft.joystick.autofire);
            if ui.checkbox(&mut af, "Auto-fire (all 3 buttons)").changed() {
                model::set_autofire_all(&mut self.draft, af);
            }
            ui.add_enabled(centering, egui::Checkbox::new(&mut self.draft.joystick.centering, "Keyboard auto-centering"));
            ui.checkbox(&mut self.draft.joystick.shift_key_mod, "SHIFT key mod");
            ui.checkbox(&mut self.draft.joystick.hook_alt_keys, "Alt keys are joystick buttons 0 and 1");
        });
        group(ui, |ui| {
            ui.strong("Keyboard");
            ui.checkbox(&mut self.draft.keyboard.caps_lock, "Caps Lock on at start");
            ui.checkbox(&mut self.draft.keyboard.altgr_sends_char, "AltGr sends characters");
            let clone = self.draft.machine.model.is_clone();
            ui.add_enabled_ui(clone, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Pravets / TK3000 keyboard:");
                    let items = [(CloneKeyboard::Auto, "Automatic"), (CloneKeyboard::Latin, "Latin"), (CloneKeyboard::Cyrillic, "Cyrillic")];
                    combo(ui, "clonekbd", &items, &mut self.draft.keyboard.clone_layout);
                });
            });
        });
        ui.horizontal(|ui| {
            if ui.button("Paste From Clipboard").clicked() {
                ev.push(UiEvent::Paste);
            }
            ui.label("(Shift+Insert during emulation)");
        });
    }

    // ---- Sound ---------------------------------------------------------------------------

    fn page_sound(&mut self, ui: &mut Ui, ev: &mut Vec<UiEvent>) {
        ui.horizontal(|ui| {
            ui.label("Speaker / master volume");
            self.volume_slider(ui, ev);
        });
        ui.checkbox(&mut self.draft.sound.disable_sound, "Disable all sound");
        ui.checkbox(&mut self.draft.sound.disable_mockingboard, "Disable Mockingboard, Phasor, MegaAudio and SD Music output");
        ui.checkbox(&mut self.draft.sound.enable_speech, "Enable speech (SSI263 / SC-01) output");
        ui.add_space(6.0);
        ui.weak("Sound cards are chosen on the Slots page (Mockingboard C, Phasor, SAM). Their chips are set with the \"...\" button.");
    }

    // ---- Slots ---------------------------------------------------------------------------

    fn page_slots(&mut self, ui: &mut Ui) {
        let m = self.draft.machine.model;
        group(ui, |ui| {
            ui.strong("Slots");
            egui::Grid::new("slots").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
                for slot in 0..8 {
                    let types = model::current_types(&self.draft);
                    ui.label(format!("Slot {slot}:"));
                    match model::slot_choices(slot, m, &types) {
                        None => {
                            ui.add_enabled_ui(false, |ui| {
                                egui::ComboBox::from_id_salt(("slot", slot)).selected_text(model::card_name(types[0])).show_ui(ui, |_| {});
                            });
                            ui.add_enabled(false, egui::Button::new("..."));
                        }
                        Some(choices) => {
                            let mut cur = types[slot];
                            let items: Vec<(CardType, &'static str)> = choices.iter().map(|c| (*c, model::card_name(*c))).collect();
                            if combo(ui, &format!("slot{slot}"), &items, &mut cur) {
                                model::set_slot_card(&mut self.draft, slot, cur);
                            }
                            let has = model::has_options(self.draft.slots.0[slot].card_type());
                            if ui.add_enabled(has, egui::Button::new("...")).clicked() {
                                self.open_sub(slot);
                            }
                        }
                    }
                    ui.end_row();
                }
                ui.label("Slot Aux:");
                match model::aux_choices(m) {
                    None => {
                        ui.add_enabled_ui(false, |ui| {
                            egui::ComboBox::from_id_salt("slotaux").selected_text("Empty").show_ui(ui, |_| {});
                        });
                        ui.add_enabled(false, egui::Button::new("..."));
                    }
                    Some(choices) => {
                        let mut cur = self.draft.aux.card.card_type();
                        let items: Vec<(CardType, &'static str)> = choices.iter().map(|c| (*c, model::card_name(*c))).collect();
                        if combo(ui, "slotaux", &items, &mut cur) {
                            model::set_aux_card(&mut self.draft, cur);
                        }
                        let has = model::has_options(self.draft.aux.card.card_type());
                        if ui.add_enabled(has, egui::Button::new("...")).clicked() {
                            self.open_sub(AUX);
                        }
                    }
                }
                ui.end_row();
            });
            if ui.button("Reset to Default").clicked() {
                self.confirm = Some(Confirm::ResetSlots);
            }
        });
        ui.weak("Changing cards, the model, the CPU or the video refresh rate restarts the emulator when applied.");
    }

    pub(super) fn open_sub(&mut self, slot: usize) {
        let spec = if slot == AUX { self.draft.aux.card.clone() } else { self.draft.slots.0[slot].clone() };
        self.hosts();
        self.sub = Some(Sub { slot, ssc: model::SscForm::from_spec(&spec), backup: spec });
    }

    // ---- Advanced ------------------------------------------------------------------------

    fn page_advanced(&mut self, ui: &mut Ui, ev: &mut Vec<UiEvent>) {
        group(ui, |ui| {
            ui.strong("Save State");
            ui.horizontal(|ui| {
                ui.label("Filename:");
                path_row(ui, "Save state file", &mut self.draft.storage.savestate_filename, Some(("Save states", &["aws.yaml", "yaml", "json"])));
            });
            ui.checkbox(&mut self.draft.storage.save_state_on_exit, "Save state on exit");
            ui.horizontal(|ui| {
                if ui.button("Save State").clicked() {
                    self.state_action(crate::actions::Action::SaveState, ev);
                }
                ui.label("(F11 during emulation)");
            });
            ui.horizontal(|ui| {
                if ui.button("Load State").clicked() {
                    self.state_action(crate::actions::Action::LoadState, ev);
                }
                ui.label("(F12 during emulation)");
            });
        });
        ui.horizontal(|ui| {
            ui.label("CiderPress path:");
            path_row(ui, "CiderPress", &mut self.draft.storage.ciderpress_path, None);
        });
        ui.separator();
        let model_now = self.draft.machine.model;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(model_now.is_clone(), |ui| {
                ui.label("Clone:");
                let mut cur = model_now;
                let items: Vec<(apple2_core::Model, &'static str)> = model::CLONES.to_vec();
                if combo(ui, "clone", &items, &mut cur) {
                    model::set_model(&mut self.draft, cur);
                }
            });
        });
        let ii = matches!(model_now, apple2_core::Model::Apple2 | apple2_core::Model::Apple2Plus);
        ui.add_enabled(ii, egui::Checkbox::new(&mut self.draft.machine.freeze_f8_rom, "The Freeze's non-autostart F8 rom (Apple ][ or ][+ only)"));
        ui.horizontal(|ui| {
            ui.label("Game I/O Connector:");
            combo(ui, "gameio", &DONGLES, &mut self.draft.joystick.dongle);
        });
        ui.checkbox(&mut self.draft.machine.no_slot_clock, "No-Slot clock");
        ui.horizontal(|ui| {
            // AppleWin's benchmark runs the emulator flat out for a few seconds; headless mode of this
            // program does that, the in-window variant is not implemented.
            ui.add_enabled(false, egui::Button::new("Benchmark Emulator"));
        });
        ui.separator();
        ui.collapsing("Custom ROMs", |ui| {
            let rom = Some(("ROM images", &["rom", "bin"][..]));
            ui.horizontal(|ui| {
                ui.label("System ROM (16K or 12K):");
                path_row(ui, "System ROM", &mut self.draft.machine.custom_rom, rom);
            });
            ui.horizontal(|ui| {
                ui.label("F8 ROM (2K):");
                path_row(ui, "F8 ROM", &mut self.draft.machine.custom_f8_rom, rom);
            });
            ui.horizontal(|ui| {
                ui.label("Video ROM:");
                path_row(ui, "Video ROM", &mut self.draft.machine.video_rom, rom);
            });
            ui.horizontal(|ui| {
                ui.label("Boot sector:");
                path_row(ui, "Boot sector", &mut self.draft.machine.boot_sector, None);
            });
            ui.horizontal(|ui| {
                ui.label("Memory power-on pattern:");
                ui.add(egui::DragValue::new(&mut self.draft.machine.memory_clear_type).range(0..=7));
            });
            ui.weak("ROM and pattern changes restart the emulator when applied.");
        });
        ui.collapsing("New disk images", |ui| {
            ui.label("Files copied onto new ProDOS disks:");
            ui.checkbox(&mut self.draft.storage.new_disk_copy_bitsy_boot, "BITSY.BOOT");
            ui.checkbox(&mut self.draft.storage.new_disk_copy_bitsy_bye, "QUIT.SYSTEM (Bitsy Bye)");
            ui.checkbox(&mut self.draft.storage.new_disk_copy_basic, "BASIC.SYSTEM");
            ui.checkbox(&mut self.draft.storage.new_disk_copy_prodos, "PRODOS");
        });
    }

    /// Save/Load State buttons: AppleWin drops pending hardware changes and runs the action, using the
    /// file name typed here. Only the file name is applied.
    fn state_action(&mut self, action: crate::actions::Action, ev: &mut Vec<UiEvent>) {
        let mut s = self.original.clone();
        s.storage.savestate_filename = self.draft.storage.savestate_filename.clone();
        ev.push(UiEvent::ApplyLive(Box::new(s.clone())));
        self.original = s;
        ev.push(UiEvent::Action(action));
        self.open = false;
    }
}
