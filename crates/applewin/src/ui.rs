//! egui layer: the AppleWin-like toolbar, the Apple screen, the status bar and the settings
//! dialog skeleton. Pure drawing: it reports what happened as [`UiEvent`]s.

use crate::actions::Action;
use crate::backend::DriveStatus;
use crate::disk_ui::DiskUi;
use crate::emu::EmuStatus;
use crate::settings::SettingsDialog;
use crate::icons::{Icon, IconCache};
use crate::toolbar::{self, Button};
use crate::video::Frame;
use apple2_config::Settings;
use egui::{Color32, Context, Rect, TextureHandle, TextureOptions, Vec2};

/// Toolbar icon / LED sizes in points (rasterised at the current pixels-per-point).
const ICON_POINTS: f32 = 40.0;
const LED_POINTS: f32 = 12.0;

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
    /// Mouse capture state for the overlay hint (set by the shell).
    pub mouse: crate::overlay::MouseState,
    /// Rasterised SVG toolbar icons, per pixel size.
    icons: IconCache,
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
            mouse: Default::default(),
            icons: IconCache::default(),
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
            egui::SidePanel::left("toolbar").exact_width(104.0).resizable(false).show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for b in Button::ALL {
                    self.button(ui, b, status, &mut ev);
                }
            });
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(icon) = Icon::caps_for_model(&status.model, status.caps_lock) {
                        let ctx = ui.ctx().clone();
                        if let Some(t) = self.icons.get(&ctx, icon, 16.0) {
                            ui.add(egui::Image::new((t.id(), Vec2::splat(16.0)))).on_hover_text("Caps Lock");
                        }
                    }
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
        if let Some(rect) = self.screen_rect {
            let toolbar_visible = !fullscreen && self.show_toolbar;
            crate::overlay::paint(ctx, rect, &crate::overlay::Overlay::new(status, toolbar_visible, self.mouse));
        }
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
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            let tex = self.icons.get(&ctx, b.icon(&status.model), ICON_POINTS).map(|t| t.id());
            let resp = match tex {
                Some(id) => ui.add(egui::ImageButton::new(egui::Image::new((id, Vec2::splat(ICON_POINTS))))),
                None => ui.add_sized(Vec2::splat(ICON_POINTS), egui::Button::new(&label)),
            };
            let resp = resp.on_hover_text(format!("{label} (F{})", b.fkey()));
            if resp.clicked() {
                ev.push(UiEvent::Action(b.action()));
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(egui::RichText::new(b.label()).small());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.label(egui::RichText::new(format!("F{}", b.fkey())).small().weak());
                    if let Some(d) = drive {
                        if resp.secondary_clicked() {
                            self.drive_menu = Some(d);
                        }
                        let drv: &DriveStatus = &status.drives[d];
                        let led = self.icons.get(&ctx, toolbar::led_icon(drv), LED_POINTS).map(|t| t.id());
                        if let Some(id) = led {
                            ui.add(egui::Image::new((id, Vec2::splat(LED_POINTS))));
                        }
                    }
                });
            });
        });
    }

    /// F8: opens the Settings dialog on the applied settings, or closes it.
    pub fn toggle_settings(&mut self, settings: &Settings) {
        self.applied = settings.clone();
        self.settings.toggle(settings);
    }
}
