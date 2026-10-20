//! egui layer: the AppleWin-like toolbar, the Apple screen, the status bar and the settings
//! dialog skeleton. Pure drawing: it reports what happened as [`UiEvent`]s.

use crate::actions::Action;
use crate::backend::DriveStatus;
use crate::disk_ui::DiskUi;
use crate::emu::EmuStatus;
use crate::settings::SettingsDialog;
use crate::toolbar::{self, Button, Led};
use crate::video::Frame;
use apple2_config::Settings;
use egui::{Color32, Context, Rect, TextureHandle, TextureOptions, Vec2};

#[derive(Debug)]
pub enum UiEvent {
    Action(Action),
    /// Rebuild the machine (restart) with these settings.
    Apply(Box<Settings>),
    /// Settings that need no restart: video, input, speed, file names...
    ApplyLive(Box<Settings>),
    Volume(u32),
    EjectDisk(usize),
    InsertDisk { drive: usize, path: std::path::PathBuf },
    SetWriteProtect { drive: usize, on: bool },
    /// "Paste From Clipboard" button of the Input page.
    Paste,
}

pub struct UiState {
    texture: Option<TextureHandle>,
    pub settings: SettingsDialog,
    pub disk: DiskUi,
    /// The settings in force (for the disk menu's hard disk section).
    applied: Settings,
    pub show_toolbar: bool,
    pub integer_scale: bool,
    /// Where the Apple screen was drawn last frame (for mouse-as-joystick mapping).
    pub screen_rect: Option<Rect>,
    pub message: String,
    pub drive_menu: Option<usize>,
}

impl UiState {
    pub fn new(settings: &Settings) -> Self {
        let mut ui = Self::blank(settings);
        // developer hook for screenshots: APPLEWIN_OPEN_DIALOG=slots:2 or drive:0
        if let Ok(spec) = std::env::var("APPLEWIN_OPEN_DIALOG") {
            match spec.strip_prefix("drive:").and_then(|d| d.parse::<usize>().ok()) {
                Some(d) => ui.drive_menu = Some(d.min(1)),
                None => ui.settings.open_at(settings, &spec),
            }
        }
        ui
    }

    fn blank(settings: &Settings) -> Self {
        UiState {
            texture: None,
            settings: SettingsDialog::new(settings),
            disk: DiskUi::default(),
            applied: settings.clone(),
            show_toolbar: true,
            integer_scale: false,
            screen_rect: None,
            message: String::new(),
            drive_menu: None,
        }
    }

    pub fn upload(&mut self, ctx: &Context, frame: &Frame) {
        if frame.width == 0 {
            return;
        }
        let img = egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
        match &mut self.texture {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.texture = Some(ctx.load_texture("apple-screen", img, TextureOptions::NEAREST)),
        }
    }

    pub fn draw(&mut self, ctx: &Context, status: &EmuStatus, fullscreen: bool) -> Vec<UiEvent> {
        let mut ev = Vec::new();
        if !fullscreen && self.show_toolbar {
            egui::SidePanel::left("toolbar").exact_width(92.0).resizable(false).show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for b in Button::ALL {
                    self.button(ui, b, status, &mut ev);
                }
            });
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(toolbar::status_line(status));
                    for (i, d) in status.drives.iter().enumerate() {
                        ui.separator();
                        ui.label(toolbar::drive_text(i, d));
                    }
                    if !self.message.is_empty() {
                        ui.separator();
                        ui.label(&self.message);
                    }
                });
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(ctx, |ui| {
            let avail = ui.available_rect_before_wrap();
            if let Some(tex) = &self.texture {
                let size = tex.size_vec2();
                let mut scale = (avail.width() / size.x).min(avail.height() / size.y);
                if self.integer_scale && scale >= 1.0 {
                    scale = scale.floor();
                }
                let rect = Rect::from_center_size(avail.center(), size * scale);
                ui.painter().image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                self.screen_rect = Some(rect);
            }
        });
        self.disk.drive_menu(ctx, &mut self.drive_menu, &status.drives, &self.applied, &mut ev);
        self.settings.show(ctx, &mut self.integer_scale, &mut ev);
        for e in &ev {
            if let UiEvent::Apply(s) | UiEvent::ApplyLive(s) = e {
                self.applied = (**s).clone();
            }
        }
        ev
    }

    fn button(&mut self, ui: &mut egui::Ui, b: Button, status: &EmuStatus, ev: &mut Vec<UiEvent>) {
        let drive = match b {
            Button::Drive1 => Some(0),
            Button::Drive2 => Some(1),
            _ => None,
        };
        let label = match b {
            Button::Run if status.paused => "Run (paused)".to_string(),
            _ => b.label().to_string(),
        };
        ui.horizontal(|ui| {
            let resp = ui.add_sized(Vec2::new(68.0, 26.0), egui::Button::new(format!("{label}\nF{}", b.fkey())).small());
            if resp.clicked() {
                ev.push(UiEvent::Action(b.action()));
            }
            if let Some(d) = drive {
                if resp.secondary_clicked() {
                    self.drive_menu = Some(d);
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                let drv: &DriveStatus = &status.drives[d];
                let color = match toolbar::led(drv) {
                    Led::Off => Color32::from_gray(60),
                    Led::Green => Color32::from_rgb(40, 220, 60),
                    Led::Red => Color32::from_rgb(230, 40, 40),
                };
                ui.painter().circle_filled(rect.center(), 4.5, color);
            }
        });
    }

    /// F8: opens the Settings dialog on the applied settings, or closes it.
    pub fn toggle_settings(&mut self, settings: &Settings) {
        self.applied = settings.clone();
        self.settings.toggle(settings);
    }
}
