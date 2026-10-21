//! AppleWin's Settings property sheet (F8) in egui.
//!
//! Pages follow `Applewin.rc`: Configuration, Input, Slots and Advanced, plus a Sound page for
//! the sound switches that AppleWin only has on its command line. Card "..." buttons open the
//! per-card option dialogs (`options`). The dialog edits a draft [`Settings`]; OK/Apply decide
//! between a live apply and a restart (with AppleWin's "The emulator needs to restart" prompt)
//! using [`model::restart_reasons`]. Logic lives in [`model`] and is tested without egui.

pub mod model;
mod options;
mod pages;

use crate::ui::UiEvent;
use apple2_config::Settings;
use egui::Context;
use model::{Problem, SpeedForm};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Configuration,
    Input,
    Sound,
    Slots,
    Advanced,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Configuration,
        Page::Input,
        Page::Sound,
        Page::Slots,
        Page::Advanced,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Configuration => "Configuration",
            Page::Input => "Input",
            Page::Sound => "Sound",
            Page::Slots => "Slots",
            Page::Advanced => "Advanced",
        }
    }
}

/// What pressing OK/Apply leads to.
#[derive(Debug, PartialEq)]
pub enum ApplyPlan {
    /// Blocking errors; nothing is applied.
    Invalid(Vec<Problem>),
    /// Nothing that needs a restart changed.
    Live(Box<Settings>),
    /// Hardware changed: ask first (AppleWin's `HardwareConfigChanged` + `IsOkToRestart`).
    Restart {
        reasons: Vec<String>,
        settings: Box<Settings>,
    },
}

/// Modal questions the dialog asks.
#[derive(Debug)]
enum Confirm {
    Restart {
        reasons: Vec<String>,
        settings: Box<Settings>,
        close: bool,
    },
    ResetAll,
    ResetSlots,
}

/// A card options sub-dialog; edits the draft in place and restores `backup` on Cancel.
struct Sub {
    /// 0-7, or `AUX`.
    slot: usize,
    backup: apple2_config::SlotCardSpec,
    ssc: Option<model::SscForm>,
}

pub const AUX: usize = 8;

/// Host lists for the SSC serial port and Uthernet interface drop-downs.
#[derive(Clone, Debug, Default)]
pub struct HostLists {
    pub serial_ports: Vec<String>,
    /// (name, description)
    pub interfaces: Vec<(String, String)>,
}

impl HostLists {
    pub fn scan() -> Self {
        HostLists {
            serial_ports: scan_serial_ports(),
            interfaces: scan_interfaces(),
        }
    }
}

/// Serial ports for the SSC drop-down: the `serialport` crate's list (when built in) plus the
/// per-OS heuristic scan, see `apple2_hostio::serial::list_serial_ports` (AUD-P-031).
fn scan_serial_ports() -> Vec<String> {
    apple2_hostio::serial::list_serial_ports()
}

/// Interfaces for the Uthernet drop-down with their description: libpcap/Npcap's with the `pcap`
/// feature, else the OS's own list, see `apple2_hostio::interfaces::list_interfaces` (AUD-P-032).
fn scan_interfaces() -> Vec<(String, String)> {
    apple2_hostio::interfaces::list_interfaces()
}

pub struct SettingsDialog {
    pub open: bool,
    /// What is applied now; the baseline for restart detection.
    original: Settings,
    pub draft: Settings,
    page: Page,
    speed: SpeedForm,
    sub: Option<Sub>,
    confirm: Option<Confirm>,
    errors: Vec<String>,
    hosts: Option<HostLists>,
}

impl SettingsDialog {
    pub fn new(settings: &Settings) -> Self {
        SettingsDialog {
            open: false,
            original: settings.clone(),
            draft: settings.clone(),
            page: Page::Configuration,
            speed: SpeedForm::from_settings(settings),
            sub: None,
            confirm: None,
            errors: Vec::new(),
            hosts: None,
        }
    }

    /// F8: opens with the applied settings, or closes (Cancel).
    pub fn toggle(&mut self, settings: &Settings) {
        if self.open {
            self.open = false;
        } else {
            self.open_with(settings);
        }
    }

    pub fn open_with(&mut self, settings: &Settings) {
        self.original = settings.clone();
        self.draft = settings.clone();
        self.speed = SpeedForm::from_settings(settings);
        self.sub = None;
        self.confirm = None;
        self.errors.clear();
        self.open = true;
    }

    /// Opens straight on a page / card dialog: `"slots"`, `"slots:2"` (options of slot 2, `aux` for the
    /// aux slot). Used by `APPLEWIN_OPEN_DIALOG` to take screenshots of every dialog.
    pub fn open_at(&mut self, settings: &Settings, spec: &str) {
        self.open_with(settings);
        let (page, slot) = spec.split_once(':').unwrap_or((spec, ""));
        if let Some(p) = Page::ALL
            .iter()
            .find(|p| p.title().eq_ignore_ascii_case(page))
        {
            self.page = *p;
        }
        match slot {
            "" => {}
            "aux" => self.open_sub(AUX),
            n => {
                if let Some(n) = n.parse::<usize>().ok().filter(|n| *n < 8) {
                    self.open_sub(n);
                }
            }
        }
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
    }

    /// The settings the form currently describes, repaired (`normalize`) and with the speed form applied.
    pub fn current(&self) -> Settings {
        let mut s = self.draft.clone();
        self.speed.apply(&mut s);
        model::normalize(&mut s);
        s
    }

    /// Decides what OK/Apply does with the current form.
    pub fn plan(&self) -> ApplyPlan {
        let s = self.current();
        let problems = model::validate(&s);
        if !problems.is_empty() {
            return ApplyPlan::Invalid(problems);
        }
        let reasons = model::restart_reasons(&self.original, &s);
        if reasons.is_empty() {
            ApplyPlan::Live(Box::new(s))
        } else {
            ApplyPlan::Restart {
                reasons,
                settings: Box::new(s),
            }
        }
    }

    /// The applied settings are now `s` (so the next Apply compares against it).
    pub fn committed(&mut self, s: &Settings) {
        self.original = s.clone();
        self.draft = s.clone();
        self.speed = SpeedForm::from_settings(s);
    }

    /// Runs OK/Apply: emits the events, or opens the restart prompt / error list.
    fn do_apply(&mut self, close: bool, ev: &mut Vec<UiEvent>) {
        match self.plan() {
            ApplyPlan::Invalid(p) => self.errors = p.into_iter().map(|p| p.message).collect(),
            ApplyPlan::Live(s) => {
                self.errors.clear();
                ev.push(UiEvent::ApplyLive(s.clone()));
                self.committed(&s);
                self.open &= !close;
            }
            ApplyPlan::Restart { reasons, settings } => {
                self.errors.clear();
                self.confirm = Some(Confirm::Restart {
                    reasons,
                    settings,
                    close,
                });
            }
        }
    }

    /// Draws the dialog (and its sub-dialogs); pushes the resulting events.
    pub fn show(&mut self, ctx: &Context, integer_scale: &mut bool, ev: &mut Vec<UiEvent>) {
        if !self.open {
            return;
        }
        let mut open = true;
        let max_h = (ctx.available_rect().height() - 60.0).max(240.0);
        egui::Window::new("Configuration")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .max_height(max_h)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .enabled(self.sub.is_none() && self.confirm.is_none())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for p in Page::ALL {
                        ui.selectable_value(&mut self.page, p, p.title());
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height((max_h - 240.0).max(160.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        self.draw_page(ui, integer_scale, ev);
                    });
                ui.separator();
                for e in &self.errors {
                    ui.colored_label(egui::Color32::from_rgb(230, 80, 60), e);
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        self.do_apply(true, ev);
                    }
                    if ui.button("Cancel").clicked() {
                        self.open = false;
                    }
                    if ui.button("Apply").clicked() {
                        self.do_apply(false, ev);
                    }
                });
            });
        if !open {
            self.open = false;
        }
        self.draw_sub(ctx);
        self.draw_confirm(ctx, ev);
    }

    fn draw_confirm(&mut self, ctx: &Context, ev: &mut Vec<UiEvent>) {
        let Some(c) = &self.confirm else { return };
        enum Answer {
            Pending,
            Yes,
            No,
        }
        let mut answer = Answer::Pending;
        let (title, text) = match c {
            Confirm::Restart { reasons, .. } => {
                let mut t = String::from("The emulator needs to restart as the hardware configuration has changed:\n\n");
                for r in reasons {
                    t.push_str(&format!(". {r}\n"));
                }
                t.push_str(
                    "\nRestarting the emulator will reset the state of the emulated machine, causing you to lose any unsaved work.\n\nAre you sure you want to do this?",
                );
                ("Configuration", t)
            }
            Confirm::ResetAll => (
                "Reset Configuration",
                "Resetting configuration to the default values whilst the machine is being emulated could result in loss of unsaved work, or corruption of floppy or hard disk images.\n\nAre you sure you want to do this?".to_string(),
            ),
            Confirm::ResetSlots => ("Slots", "Reset the cards of all slots to the defaults for this model?".to_string()),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        answer = Answer::Yes;
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Answer::No;
                    }
                });
            });
        match answer {
            Answer::Pending => {}
            Answer::No => self.confirm = None,
            Answer::Yes => match self.confirm.take() {
                Some(Confirm::Restart {
                    settings, close, ..
                }) => {
                    ev.push(UiEvent::Apply(settings.clone()));
                    self.committed(&settings);
                    self.open &= !close;
                }
                Some(Confirm::ResetAll) => {
                    let keep_pos = self.draft.video.window_pos;
                    self.draft = Settings::default();
                    self.draft.video.window_pos = keep_pos;
                    self.speed = SpeedForm::from_settings(&self.draft);
                }
                Some(Confirm::ResetSlots) => model::reset_slots_to_default(&mut self.draft),
                None => {}
            },
        }
    }

    fn hosts(&mut self) -> &HostLists {
        self.hosts.get_or_insert_with(HostLists::scan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_config::{CardType, SlotCardSpec};
    use apple2_core::Model;

    fn dlg() -> SettingsDialog {
        let mut d = SettingsDialog::new(&Settings::default());
        d.open_with(&Settings::default());
        d
    }

    #[test]
    fn untouched_form_is_a_live_apply_of_the_same_settings() {
        let d = dlg();
        match d.plan() {
            ApplyPlan::Live(s) => assert_eq!(*s, Settings::default()),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn video_and_input_edits_apply_live() {
        let mut d = dlg();
        d.draft.video.vertical_blend = true;
        d.draft.joystick.pdl_x_trim = -5;
        d.draft.sound.master_volume = 12;
        d.speed = SpeedForm {
            authentic: false,
            custom: 25,
        };
        match d.plan() {
            ApplyPlan::Live(s) => {
                assert!(s.video.vertical_blend);
                assert_eq!(s.joystick.pdl_x_trim, -5);
                assert_eq!(s.machine.speed, 25);
            }
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn card_change_asks_for_restart_with_reason() {
        let mut d = dlg();
        d.draft.slots.0[5] = SlotCardSpec::new(CardType::Phasor);
        match d.plan() {
            ApplyPlan::Restart { reasons, settings } => {
                assert!(reasons
                    .iter()
                    .any(|r| r.contains("Slot 5") && r.contains("Phasor")));
                assert_eq!(settings.slot(5).card_type(), CardType::Phasor);
            }
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn invalid_form_is_blocked() {
        let mut d = dlg();
        d.draft.slots.0[3] = SlotCardSpec::new(CardType::Ssc); // second SSC
        assert!(matches!(d.plan(), ApplyPlan::Invalid(_)));
    }

    #[test]
    fn model_change_via_form_resets_slot0_and_needs_restart() {
        let mut d = dlg();
        model::set_model(&mut d.draft, Model::Apple2Plus);
        match d.plan() {
            ApplyPlan::Restart { reasons, settings } => {
                assert!(reasons[0].contains("computer"));
                assert_eq!(settings.slot(0).card_type(), CardType::LanguageCard);
            }
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn committed_becomes_the_new_baseline() {
        let mut d = dlg();
        d.draft.slots.0[5] = SlotCardSpec::new(CardType::Phasor);
        let ApplyPlan::Restart { settings, .. } = d.plan() else {
            panic!()
        };
        d.committed(&settings);
        assert!(matches!(d.plan(), ApplyPlan::Live(_)));
    }

    #[test]
    fn ok_with_live_changes_closes_and_emits() {
        let mut d = dlg();
        d.draft.video.half_scanlines = false;
        let mut ev = Vec::new();
        d.do_apply(true, &mut ev);
        assert!(!d.open);
        assert!(matches!(ev.as_slice(), [UiEvent::ApplyLive(s)] if !s.video.half_scanlines));
    }

    #[test]
    fn apply_with_restart_waits_for_confirmation() {
        let mut d = dlg();
        d.draft.machine.no_slot_clock = false;
        let mut ev = Vec::new();
        d.do_apply(true, &mut ev);
        assert!(d.open && ev.is_empty() && d.confirm.is_some());
    }

    #[test]
    fn toggle_reopens_from_the_applied_settings() {
        let mut d = dlg();
        d.draft.video.vertical_blend = true;
        d.toggle(&Settings::default());
        assert!(!d.open);
        d.toggle(&Settings::default());
        assert!(d.open && !d.draft.video.vertical_blend);
    }
}
