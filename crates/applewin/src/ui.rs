//! egui layer: the AppleWin-like toolbar, the Apple screen, the status bar and the settings
//! dialog skeleton. Pure drawing: it reports what happened as [`UiEvent`]s.

use crate::actions::Action;
use crate::backend::DriveStatus;
use crate::emu::{EmuStatus, VOLUME_MAX};
use crate::toolbar::{self, Button, Led};
use crate::video::Frame;
use apple2_config::settings::RefreshRate;
use apple2_config::Settings;
use apple2_core::Model;
use egui::{Color32, Context, Rect, TextureHandle, TextureOptions, Vec2};

#[derive(Debug)]
pub enum UiEvent {
    Action(Action),
    /// Rebuild the machine with these settings.
    Apply(Box<Settings>),
    Volume(u32),
    EjectDisk(usize),
}

pub struct UiState {
    texture: Option<TextureHandle>,
    pub show_settings: bool,
    pub draft: Settings,
    pub show_toolbar: bool,
    pub integer_scale: bool,
    /// Where the Apple screen was drawn last frame (for mouse-as-joystick mapping).
    pub screen_rect: Option<Rect>,
    pub message: String,
    pub drive_menu: Option<usize>,
}

const MODELS: [(Model, &str); 9] = [
    (Model::Apple2, "Apple ]["),
    (Model::Apple2Plus, "Apple ][+"),
    (Model::Apple2JPlus, "Apple ][ J-Plus"),
    (Model::Apple2e, "Apple //e"),
    (Model::Apple2eEnhanced, "Apple //e Enhanced"),
    (Model::Pravets82, "Pravets 82"),
    (Model::Pravets8M, "Pravets 8M"),
    (Model::Base64A, "Base64A"),
    (Model::Tk3000e, "TK3000 //e"),
];

impl UiState {
    pub fn new(settings: &Settings) -> Self {
        UiState {
            texture: None,
            show_settings: false,
            draft: settings.clone(),
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
        if let Some(d) = self.drive_menu {
            let mut open = true;
            egui::Window::new(format!("Drive {}", d + 1)).open(&mut open).collapsible(false).show(ctx, |ui| {
                if ui.button("Insert disk image...").clicked() {
                    ev.push(UiEvent::Action(Action::DriveSelect(d as u8)));
                    self.drive_menu = None;
                }
                if ui.button("Eject").clicked() {
                    ev.push(UiEvent::EjectDisk(d));
                    self.drive_menu = None;
                }
            });
            if !open {
                self.drive_menu = None;
            }
        }
        if self.show_settings {
            self.settings_window(ctx, &mut ev);
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

    fn settings_window(&mut self, ctx: &Context, ev: &mut Vec<UiEvent>) {
        let mut open = true;
        egui::Window::new("Configuration").open(&mut open).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                ui.label("Machine");
                let current = MODELS.iter().find(|(m, _)| *m == self.draft.machine.model).map_or("Other", |(_, n)| *n);
                egui::ComboBox::from_id_salt("model").selected_text(current).show_ui(ui, |ui| {
                    for (m, n) in MODELS {
                        ui.selectable_value(&mut self.draft.machine.model, m, n);
                    }
                });
                ui.end_row();

                ui.label("Video standard");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.draft.machine.refresh_rate, RefreshRate::Hz60, "NTSC 60 Hz");
                    ui.selectable_value(&mut self.draft.machine.refresh_rate, RefreshRate::Hz50, "PAL 50 Hz");
                });
                ui.end_row();

                ui.label("Speed");
                let mut speed = self.draft.machine.speed.min(40);
                ui.add(egui::Slider::new(&mut speed, 1..=40).custom_formatter(|v, _| if v >= 40.0 { "max".into() } else { format!("{:.1}x", v / 10.0) }));
                self.draft.machine.speed = speed;
                self.draft.machine.clock_multiplier = None;
                ui.end_row();

                ui.label("Volume");
                let mut vol = VOLUME_MAX - self.draft.sound.master_volume.min(VOLUME_MAX);
                if ui.add(egui::Slider::new(&mut vol, 0..=VOLUME_MAX)).changed() {
                    self.draft.sound.master_volume = VOLUME_MAX - vol;
                    ev.push(UiEvent::Volume(self.draft.sound.master_volume));
                }
                ui.end_row();

                ui.label("Window scale");
                ui.add(egui::Slider::new(&mut self.draft.video.window_scale, 1..=4));
                ui.end_row();

                ui.label("Integer scaling");
                ui.checkbox(&mut self.integer_scale, "");
                ui.end_row();

                ui.label("Disable sound");
                ui.checkbox(&mut self.draft.sound.disable_sound, "");
                ui.end_row();
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply (reboots)").clicked() {
                    ev.push(UiEvent::Apply(Box::new(self.draft.clone())));
                }
                if ui.button("Close").clicked() {
                    self.show_settings = false;
                }
            });
            ui.label("Cards, disks, joysticks and the remaining options arrive with apple2-config's full dialog.");
        });
        if !open {
            self.show_settings = false;
        }
    }
}
