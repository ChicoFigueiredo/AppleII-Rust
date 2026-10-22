//! Central de Impressoras: the second native window (own surface, own egui context) that shows
//! what the Impressora virtual printed. Spec: docs/prompts/AppleII_Rust_Prompt_Impressoras_Virtuais_LX310.md,
//! ADR 0008.
//!
//! Layout of this module:
//! - pure, GPU-free logic (routing, lifecycle request, preview state, repaint limiter, activity
//!   indicator, page rasteriser): unit-tested without a window;
//! - [`PrinterWindow`]: owns the optional native window and draws [`PrinterViewModel`] snapshots.
//!
//! Keyboard isolation: every `WindowEvent` is routed by window id ([`route`]); only
//! [`Route::Main`] events can reach the Apple II. Anything typed in this window stays in egui.
//!
//! `WindowAttributes::with_active(false)` (auto-open without stealing focus) is documented by winit
//! 0.30 as unsupported on X11, Wayland, Android, iOS and Orbital; it works on Windows, macOS and
//! web. Where it is unsupported [`NO_ACTIVE_FLAG`] is true and the app hands focus back to the main
//! window right after an automatic opening (best effort: the compositor may still refuse).

use apple2_printer::document::{Page, PageDocument};
use apple2_printer::paper::{InkColor, PaperKind, Ribbon};
use apple2_printer::units::PER_INCH;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions, Vec2};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::gfx::{Gfx, GpuShared};
use crate::printer_panels::{self as panels, CenterUi, Tab};

pub const TITLE: &str = "Central de Impressoras — apple2-rust";
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;
/// 100% zoom shows one inch as this many points.
pub const POINTS_PER_INCH_AT_100: f32 = 96.0;
/// Redraws of the Central are capped at about 60 per second, whatever the byte rate.
pub const MIN_REPAINT_INTERVAL: Duration = Duration::from_micros(16_667);
/// The activity indicator stays lit this long after the last byte.
pub const ACTIVITY_HOLD: Duration = Duration::from_millis(300);
/// Largest raster side in pixels (also below the usual wgpu texture limit of 8192).
const MAX_RASTER_SIDE: f32 = 8192.0;

/// Platforms where winit ignores `with_active(false)` (see the module docs).
pub const NO_ACTIVE_FLAG: bool = cfg!(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "android",
    target_os = "ios"
));

// ---------------------------------------------------------------------------------------------
// Event routing
// ---------------------------------------------------------------------------------------------

/// Which window an event belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Main,
    Printer,
    /// A window we do not know (already destroyed): ignore.
    Unknown,
}

impl Route {
    /// Only events of the main window may feed the emulated keyboard, joystick and mouse.
    pub fn reaches_emulator(self) -> bool {
        self == Route::Main
    }
}

/// Routes a window id. Generic so that tests do not need real `WindowId`s.
pub fn route<T: PartialEq>(id: T, main: Option<T>, printer: Option<T>) -> Route {
    if main.as_ref() == Some(&id) {
        Route::Main
    } else if printer.as_ref() == Some(&id) {
        Route::Printer
    } else {
        Route::Unknown
    }
}

// ---------------------------------------------------------------------------------------------
// View model (what the integration layer feeds the window; see `printer_model`)
// ---------------------------------------------------------------------------------------------

pub use crate::printer_model::{
    CenterEvent, JobEntry, JobState, Look, PrintSetup, PrinterViewModel, Shown, Status,
};

// ---------------------------------------------------------------------------------------------
// Preview state (page navigation + zoom)
// ---------------------------------------------------------------------------------------------

pub const ZOOM_STEPS: [f32; 9] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewState {
    /// 0-based index of the page shown.
    pub page: usize,
    pub zoom: f32,
    pub fit_width: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState {
            page: 0,
            zoom: 1.0,
            fit_width: true,
        }
    }
}

impl PreviewState {
    pub fn clamp_page(&mut self, count: usize) {
        self.page = self.page.min(count.saturating_sub(1));
    }
    pub fn first(&mut self) {
        self.page = 0;
    }
    pub fn prev(&mut self) {
        self.page = self.page.saturating_sub(1);
    }
    pub fn next(&mut self, count: usize) {
        self.page = (self.page + 1).min(count.saturating_sub(1));
    }
    pub fn last(&mut self, count: usize) {
        self.page = count.saturating_sub(1);
    }
    /// Manual zoom turns "fit to width" off.
    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
        self.fit_width = false;
    }
    pub fn zoom_in(&mut self) {
        let z = ZOOM_STEPS
            .iter()
            .copied()
            .find(|s| *s > self.zoom + 1e-4)
            .unwrap_or(MAX_ZOOM);
        self.set_zoom(z);
    }
    pub fn zoom_out(&mut self) {
        let z = ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|s| *s < self.zoom - 1e-4)
            .unwrap_or(MIN_ZOOM);
        self.set_zoom(z);
    }
    /// Zoom actually used: the fixed one, or the one that makes the page as wide as `avail_width`
    /// points (clamped to the 25-400% range).
    pub fn effective_zoom(&self, avail_width: f32, page_width_in: f32) -> f32 {
        if self.fit_width && page_width_in > 0.0 && avail_width > 0.0 {
            (avail_width / (page_width_in * POINTS_PER_INCH_AT_100)).clamp(MIN_ZOOM, MAX_ZOOM)
        } else {
            self.zoom
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Repaint limiter and activity indicator
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redraw {
    Idle,
    Now,
    At(Instant),
}

/// Coalesces redraw requests to at most one per [`MIN_REPAINT_INTERVAL`]. Requests are cheap
/// flags, so thousands of bytes (or view model updates) between two frames cost one frame.
#[derive(Debug)]
pub struct RepaintLimiter {
    interval: Duration,
    last: Option<Instant>,
    pending: bool,
}

impl Default for RepaintLimiter {
    fn default() -> Self {
        RepaintLimiter::new(MIN_REPAINT_INTERVAL)
    }
}

impl RepaintLimiter {
    pub fn new(interval: Duration) -> Self {
        RepaintLimiter {
            interval,
            last: None,
            pending: false,
        }
    }
    pub fn request(&mut self) {
        self.pending = true;
    }
    pub fn poll(&self, now: Instant) -> Redraw {
        if !self.pending {
            return Redraw::Idle;
        }
        match self.last {
            Some(t) if now.duration_since(t) < self.interval => Redraw::At(t + self.interval),
            _ => Redraw::Now,
        }
    }
    /// Call when a frame was drawn.
    pub fn painted(&mut self, now: Instant) {
        self.last = Some(now);
        self.pending = false;
    }
}

/// "Bytes are arriving" light. `set_activity(true)` is O(1) and may be called per byte; the light
/// holds for [`ACTIVITY_HOLD`] so the toolbar never repaints per byte.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActivityIndicator {
    last: Option<Instant>,
}

impl ActivityIndicator {
    pub fn set_activity(&mut self, on: bool, now: Instant) {
        self.last = on.then_some(now);
    }
    pub fn is_active(&self, now: Instant) -> bool {
        self.last
            .is_some_and(|t| now.saturating_duration_since(t) < ACTIVITY_HOLD)
    }
    pub fn hold(&self) -> Duration {
        ACTIVITY_HOLD
    }
}

// ---------------------------------------------------------------------------------------------
// Page rasteriser
// ---------------------------------------------------------------------------------------------

/// Pixels per inch for a page drawn at `zoom` on a display with `pixels_per_point`, bounded so
/// that neither side exceeds [`MAX_RASTER_SIDE`]. Whole numbers: the texture cache key.
pub fn raster_ppi(page: &Page, zoom: f32, pixels_per_point: f32) -> u32 {
    let wanted = POINTS_PER_INCH_AT_100 * zoom * pixels_per_point;
    let longest_in = page.width.max(page.height).max(1) as f32 / PER_INCH as f32;
    wanted.min(MAX_RASTER_SIDE / longest_in).round().max(8.0) as u32
}

/// Draws one page with the paper of a plain form (`zebra`: green bars) or a white sheet.
pub fn rasterize_page(page: &Page, ppi: u32, zebra: bool) -> ColorImage {
    let look = Look {
        paper: if zebra {
            PaperKind::GreenBar
        } else {
            PaperKind::A4
        },
        ..Look::default()
    };
    rasterize_page_look(page, ppi, &look)
}

fn ink_rgb(ink: InkColor) -> [u32; 3] {
    match ink {
        InkColor::Black => [20, 20, 32],
        InkColor::Blue => [20, 50, 170],
        InkColor::Purple => [110, 30, 140],
    }
}

/// Deterministic 0..=255 noise from a dot position (worn ribbon).
fn noise(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    h & 0xFF
}

/// Draws one page: paper (white, green/blue-bar zebra with tractor holes for continuous forms)
/// plus the dots in the ribbon's colour. Repeated dots darken (double strike), like the ribbon; a
/// worn ribbon gives each dot a deterministic, slightly different strength.
pub fn rasterize_page_look(page: &Page, ppi: u32, look: &Look) -> ColorImage {
    let ppi = ppi as f32;
    let to_px = |u: u32| u as f32 / PER_INCH as f32 * ppi;
    let w = (to_px(page.width).ceil() as usize).max(1);
    let h = (to_px(page.height).ceil() as usize).max(1);
    let mut ink = vec![0u8; w * h];
    // LX-310 dot diameter is roughly 0.28 mm; never thinner than one pixel.
    // Same diameter as the PDF's dots (`pdf::DOT_DIAMETER`), so preview and file agree.
    let r = (apple2_printer::pdf::DOT_DIAMETER as f32 / 2.0 / PER_INCH as f32 * ppi).max(0.75);
    let r2 = r * r;
    let worn = look.ribbon == Ribbon::Worn;
    for d in &page.dots {
        let (cx, cy) = (to_px(d.x), to_px(d.y));
        let x0 = (cx - r).floor().max(0.0) as usize;
        let x1 = ((cx + r).ceil() as usize).min(w - 1);
        let y0 = (cy - r).floor().max(0.0) as usize;
        let y1 = ((cy + r).ceil() as usize).min(h - 1);
        let (px, py) = (cx.floor() as usize, cy.floor() as usize);
        let strength = if worn {
            (80 + noise(d.x, d.y) * 70 / 255) as u8
        } else {
            130
        };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let inside = dx * dx + dy * dy <= r2 || (x == px && y == py);
                if inside && x < w && y < h {
                    let p = &mut ink[y * w + x];
                    *p = p.saturating_add(strength);
                }
            }
        }
    }
    let continuous = matches!(
        look.paper,
        PaperKind::GreenBar | PaperKind::BlueBar | PaperKind::PlainContinuous
    );
    let bar: Option<[u32; 3]> = match look.paper {
        PaperKind::GreenBar => Some([226, 240, 226]),
        PaperKind::BlueBar => Some([224, 233, 246]),
        _ => None,
    };
    let band_h = (ppi / 2.0).max(1.0); // bars: half an inch (3 lines)
    let ink_c = ink_rgb(look.ink);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let paper = match bar {
            Some(b) if ((y as f32 / band_h) as usize) % 2 == 1 => b,
            _ => [255u32, 255, 255],
        };
        for x in 0..w {
            let a = u32::from(ink[y * w + x]);
            let mix = |p: u32, i: u32| ((p * (255 - a) + i * a) / 255) as u8;
            pixels.push(Color32::from_rgb(
                mix(paper[0], ink_c[0]),
                mix(paper[1], ink_c[1]),
                mix(paper[2], ink_c[2]),
            ));
        }
    }
    if continuous {
        // Tractor strips: a hole every half inch, 0.25" from each edge, and a faint perforation.
        let hole_r = (ppi * 0.055).max(1.0);
        let grey = Color32::from_gray(150);
        let perf = Color32::from_gray(205);
        let strip = ppi * 0.5;
        let mut put = |x: usize, y: usize, c: Color32| {
            if x < w && y < h {
                pixels[y * w + x] = c;
            }
        };
        for &cx in &[ppi * 0.25, w as f32 - ppi * 0.25] {
            let mut cy = ppi * 0.25;
            while cy < h as f32 {
                let (xa, xb) = (
                    (cx - hole_r).floor().max(0.0) as usize,
                    (cx + hole_r).ceil() as usize,
                );
                let (ya, yb) = (
                    (cy - hole_r).floor().max(0.0) as usize,
                    (cy + hole_r).ceil() as usize,
                );
                for y in ya..=yb {
                    for x in xa..=xb {
                        let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                        if dx * dx + dy * dy <= hole_r * hole_r {
                            put(x, y, grey);
                        }
                    }
                }
                cy += strip;
            }
        }
        for &px in &[strip as usize, w.saturating_sub(strip as usize)] {
            for y in (0..h).step_by(4) {
                put(px, y, perf);
                put(px, y + 1, perf);
            }
        }
        // Perforation between sheets (top and bottom edge).
        for x in (0..w).step_by(4) {
            put(x, 0, perf);
            put(x + 1, 0, perf);
            put(x, h - 1, perf);
            put(x + 1, h - 1, perf);
        }
    }
    ColorImage {
        size: [w, h],
        pixels,
    }
}

/// Texture of the visible page only: one entry, replaced when the page, the document revision or
/// the pixel density changes. Pages that are not shown are never rasterised.
#[derive(Default)]
pub struct PreviewCache {
    key: Option<(usize, u32, Look)>,
    /// The document the texture was drawn from (compared by identity: a new publication of the
    /// spool is a new `Arc`).
    doc: Option<Arc<PageDocument>>,
    texture: Option<TextureHandle>,
    /// How many times a page was rasterised (tests and diagnostics).
    pub rasterisations: u64,
}

impl PreviewCache {
    pub fn get(
        &mut self,
        ctx: &Context,
        page_index: usize,
        doc: &Arc<PageDocument>,
        ppi: u32,
        look: Look,
    ) -> &TextureHandle {
        let key = (page_index, ppi, look);
        let same_doc = self.doc.as_ref().is_some_and(|d| Arc::ptr_eq(d, doc));
        if self.key != Some(key) || !same_doc || self.texture.is_none() {
            let Some(page) = doc.pages.get(page_index) else {
                // Nothing to draw: keep (or make) a 1x1 texture.
                let img = ColorImage::new([1, 1], Color32::WHITE);
                match &mut self.texture {
                    Some(t) => t.set(img, TextureOptions::LINEAR),
                    None => {
                        self.texture =
                            Some(ctx.load_texture("printer-page", img, TextureOptions::LINEAR))
                    }
                }
                self.key = None;
                self.doc = None;
                return self.texture.as_ref().expect("texture just created");
            };
            let img = rasterize_page_look(page, ppi, &look);
            self.rasterisations += 1;
            match &mut self.texture {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => {
                    self.texture =
                        Some(ctx.load_texture("printer-page", img, TextureOptions::LINEAR))
                }
            }
            self.key = Some(key);
            self.doc = Some(doc.clone());
        }
        self.texture.as_ref().expect("texture just created")
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// Draws the whole Central into `ctx`. Pure egui: no window access.
pub fn draw_center(
    ctx: &Context,
    preview: &mut PreviewState,
    cache: &mut PreviewCache,
    ui_state: &mut CenterUi,
    vm: &PrinterViewModel,
    events: &mut Vec<CenterEvent>,
) {
    let count = vm.document().pages.len();
    preview.clamp_page(count);
    egui::TopBottomPanel::top("printer-status").show(ctx, |ui| {
        panels::status_bar(ui, vm);
        if let Some(s) = panels::setup_bar(ui, vm) {
            events.push(CenterEvent::SetNextSetup(s));
        }
        panels::action_bar(ui, vm, ui_state, events);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ui_state.tab, Tab::Preview, "Visualização");
            ui.selectable_value(&mut ui_state.tab, Tab::Archive, "Arquivo de impressões");
        });
    });
    egui::TopBottomPanel::bottom("printer-messages").show(ctx, |ui| {
        panels::message_bar(ui, vm);
    });
    if ui_state.tab == Tab::Archive {
        egui::CentralPanel::default().show(ctx, |ui| {
            if panels::archive_tab(ui, vm, ui_state, events) {
                ui_state.tab = Tab::Preview;
            }
        });
        panels::dialogs(ctx, ui_state, events);
        return;
    }
    egui::SidePanel::left("printer-jobs")
        .default_width(220.0)
        .resizable(true)
        .show(ctx, |ui| panels::job_list(ui, vm, events));
    egui::CentralPanel::default().show(ctx, |ui| {
        draw_preview(ctx, ui, preview, cache, vm, count);
    });
    panels::dialogs(ctx, ui_state, events);
}

fn draw_preview(
    ctx: &Context,
    ui: &mut egui::Ui,
    preview: &mut PreviewState,
    cache: &mut PreviewCache,
    vm: &PrinterViewModel,
    count: usize,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(preview.page > 0, egui::Button::new("⏮"))
            .on_hover_text("Primeira página (Home)")
            .clicked()
        {
            preview.first();
        }
        if ui
            .add_enabled(preview.page > 0, egui::Button::new("◀"))
            .on_hover_text("Página anterior (Page Up)")
            .clicked()
        {
            preview.prev();
        }
        ui.label(if count == 0 {
            "—".to_string()
        } else {
            format!("Página {} de {}", preview.page + 1, count)
        });
        if ui
            .add_enabled(preview.page + 1 < count, egui::Button::new("▶"))
            .on_hover_text("Próxima página (Page Down)")
            .clicked()
        {
            preview.next(count);
        }
        if ui
            .add_enabled(preview.page + 1 < count, egui::Button::new("⏭"))
            .on_hover_text("Última página (End)")
            .clicked()
        {
            preview.last(count);
        }
        ui.separator();
        if ui.button("−").on_hover_text("Reduzir (Ctrl+-)").clicked() {
            preview.zoom_out();
        }
        let mut pct = (preview.zoom * 100.0).round();
        if ui
            .add(
                egui::DragValue::new(&mut pct)
                    .range(MIN_ZOOM * 100.0..=MAX_ZOOM * 100.0)
                    .suffix("%"),
            )
            .changed()
        {
            preview.set_zoom(pct / 100.0);
        }
        if ui.button("+").on_hover_text("Ampliar (Ctrl++)").clicked() {
            preview.zoom_in();
        }
        if ui
            .selectable_label(preview.fit_width, "Ajustar à largura")
            .clicked()
        {
            preview.fit_width = !preview.fit_width;
        }
        if vm.shown.loading {
            ui.separator();
            ui.weak("lendo do Arquivo…");
        }
    });
    // Page navigation / zoom keys act on this window only (and not while typing in a field).
    if !ctx.wants_keyboard_input() {
        let (go_first, go_last, go_prev, go_next, zin, zout) = ui.input(|i| {
            let c = i.modifiers.command;
            (
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                c && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
                c && i.key_pressed(egui::Key::Minus),
            )
        });
        if go_first {
            preview.first();
        }
        if go_last {
            preview.last(count);
        }
        if go_prev {
            preview.prev();
        }
        if go_next {
            preview.next(count);
        }
        if zin {
            preview.zoom_in();
        }
        if zout {
            preview.zoom_out();
        }
    }
    let diags = &vm.document().diagnostics;
    if !diags.is_empty() {
        egui::CollapsingHeader::new(format!("Diagnósticos ({})", diags.len()))
            .id_salt("printer-diagnostics")
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for d in diags.iter().take(200) {
                            ui.label(format!("@{}: {} {:02X?}", d.offset, d.message, d.bytes));
                        }
                    });
            });
    }
    ui.separator();
    let Some(page) = vm.document().pages.get(preview.page) else {
        ui.centered_and_justified(|ui| ui.weak("Nenhuma página para mostrar."));
        return;
    };
    let page_w_in = page.width as f32 / PER_INCH as f32;
    let avail_w = (ui.available_width() - ui.spacing().scroll.bar_width - 12.0).max(32.0);
    let zoom = preview.effective_zoom(avail_w, page_w_in);
    let ppp = ctx.pixels_per_point();
    let ppi = raster_ppi(page, zoom, ppp);
    // The paper comes from the job (its frozen options), not from the page shape.
    let look = Look::from(vm.shown.options);
    let tex = cache.get(ctx, preview.page, &vm.shown.document, ppi, look);
    let size = Vec2::new(
        page_w_in * POINTS_PER_INCH_AT_100 * zoom,
        page.height as f32 / PER_INCH as f32 * POINTS_PER_INCH_AT_100 * zoom,
    );
    let id = tex.id();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_filled(
                rect.translate(Vec2::splat(3.0)),
                0.0,
                Color32::from_black_alpha(60),
            );
            ui.painter().image(
                id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        });
}

// ---------------------------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------------------------

/// Position and size remembered between openings in the same session (physical pixels). Wayland
/// does not let applications place windows, so only the size is restored there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub pos: Option<(i32, i32)>,
    pub size: (u32, u32),
}

struct Native {
    // Drop order: GPU state, egui, then the window it draws on.
    gfx: Gfx,
    egui_state: egui_winit::State,
    egui_ctx: Context,
    window: Arc<Window>,
    cache: PreviewCache,
}

#[derive(Default)]
pub struct PrinterWindow {
    /// An opening that was asked for and not yet realised: `true` = automatic (no focus steal).
    pending_open: Option<bool>,
    native: Option<Native>,
    placement: Option<Placement>,
    /// Page, zoom and fit mode survive closing and reopening.
    pub preview: PreviewState,
    limiter: RepaintLimiter,
    wake_at: Option<Instant>,
    events: Vec<CenterEvent>,
    /// Tab, open forms and confirmations (kept while the window is closed).
    ui_state: CenterUi,
    /// The last opening was automatic and could not avoid taking focus (see [`NO_ACTIVE_FLAG`]).
    pub opened_auto: bool,
    /// Development: save the Central's first settled frame as a PNG, then quit the app.
    pub dump: Option<std::path::PathBuf>,
    dump_requested: bool,
}

impl PrinterWindow {
    /// Asks for the window. `auto`: opened by the first printed byte, so it must not take focus
    /// from the main window. Manual (the toolbar button) focuses it. The window is created by
    /// [`PrinterWindow::realize`] on the next turn of the event loop.
    pub fn open(&mut self, auto: bool) {
        if self.native.is_some() {
            if !auto {
                self.pending_open = Some(false); // realize() will only focus it
            }
            return;
        }
        self.pending_open = Some(auto);
    }

    /// Hides (destroys) the window, remembering where it was. Emulation is not touched.
    pub fn close(&mut self) {
        if let Some(n) = self.native.take() {
            self.placement = Some(placement_of(&n.window));
        }
        self.pending_open = None;
        self.wake_at = None;
        self.limiter = RepaintLimiter::default();
    }

    pub fn is_open(&self) -> bool {
        self.native.is_some() || self.pending_open.is_some()
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.native.as_ref().map(|n| n.window.id())
    }

    pub fn placement(&self) -> Option<Placement> {
        self.placement
    }

    /// Records the geometry to use at the next opening (also used by tests).
    pub fn remember(&mut self, p: Placement) {
        self.placement = Some(p);
    }

    /// Something changed in the view model: draw again (rate-limited).
    pub fn invalidate(&mut self) {
        if self.native.is_some() {
            self.limiter.request();
        }
    }

    pub fn take_events(&mut self) -> Vec<CenterEvent> {
        std::mem::take(&mut self.events)
    }

    /// Creates the native window for a pending opening (or focuses the existing one). Returns
    /// whether the caller should give focus back to the main window.
    pub fn realize(
        &mut self,
        el: &ActiveEventLoop,
        shared: &Arc<GpuShared>,
        icon: Option<winit::window::Icon>,
    ) -> Result<bool, String> {
        let Some(auto) = self.pending_open.take() else {
            return Ok(false);
        };
        if let Some(n) = &self.native {
            if !auto {
                n.window.focus_window();
            }
            return Ok(false);
        }
        let mut attrs = Window::default_attributes()
            .with_title(TITLE)
            .with_window_icon(icon)
            .with_active(!auto);
        attrs = match self.placement {
            Some(p) => {
                let a = attrs.with_inner_size(PhysicalSize::new(p.size.0, p.size.1));
                match p.pos {
                    Some((x, y)) => a.with_position(PhysicalPosition::new(x, y)),
                    None => a,
                }
            }
            None => attrs.with_inner_size(LogicalSize::new(960.0, 720.0)),
        };
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        let gfx = Gfx::for_window(shared, window.clone())?;
        let egui_ctx = Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::from_hash_of("printer-center"),
            &window,
            None,
            None,
            None,
        );
        window.set_ime_allowed(false);
        self.native = Some(Native {
            gfx,
            egui_state,
            egui_ctx,
            window,
            cache: PreviewCache::default(),
        });
        self.opened_auto = auto;
        self.limiter.request();
        Ok(auto && NO_ACTIVE_FLAG)
    }

    /// Handles an event of this window. Never forwards anything to the emulator.
    /// Returns true when the window closed itself.
    pub fn window_event(&mut self, event: WindowEvent, vm: &PrinterViewModel) -> bool {
        let Some(n) = self.native.as_mut() else {
            return false;
        };
        let r = n.egui_state.on_window_event(&n.window, &event);
        if r.repaint {
            self.limiter.request();
        }
        match event {
            WindowEvent::CloseRequested => {
                self.close();
                return true;
            }
            WindowEvent::Resized(s) => {
                n.gfx.resize(s.width, s.height);
                self.limiter.request();
            }
            WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false) => {
                self.limiter.request();
            }
            WindowEvent::RedrawRequested => self.paint(vm),
            _ => {}
        }
        false
    }

    fn paint(&mut self, vm: &PrinterViewModel) {
        let Some(n) = self.native.as_mut() else {
            return;
        };
        let input = n.egui_state.take_egui_input(&n.window);
        let vid = input.viewport_id;
        let preview = &mut self.preview;
        let cache = &mut n.cache;
        let events = &mut self.events;
        let ui_state = &mut self.ui_state;
        let output = n.egui_ctx.run(input, |ctx| {
            draw_center(ctx, preview, cache, ui_state, vm, events)
        });
        n.egui_state
            .handle_platform_output(&n.window, output.platform_output.clone());
        let delay = output.viewport_output.get(&vid).map(|v| v.repaint_delay);
        if self.dump.is_some()
            && n.egui_ctx.cumulative_pass_nr_for(vid) > 4
            && !vm.document().pages.is_empty()
        {
            if let Some(path) = self.dump.take() {
                n.gfx.request_capture(path);
                self.dump_requested = true;
            }
        }
        n.gfx.render(&n.egui_ctx, output);
        // Right after the surface's present(): the frame with these dots is on its way to the screen.
        if let Some((job, bytes)) = vm.shown.probe {
            apple2_printer::latency::presented(job, bytes);
        }
        if self.dump_requested && n.gfx.captured {
            self.dump_requested = false;
            eprintln!("printer window dump saved");
            crate::app::request_exit();
        }
        let now = Instant::now();
        self.limiter.painted(now);
        match delay {
            Some(d) if d.is_zero() => self.limiter.request(),
            _ if self.dump.is_some() || self.dump_requested => self.limiter.request(),
            Some(d) if d < Duration::from_secs(3600) => self.wake_at = Some(now + d),
            _ => {}
        }
    }

    /// What the event loop should do next for this window: call after every batch of events.
    /// Requests the (rate-limited) redraw itself and returns the instant to wake up at, if any.
    pub fn drive(&mut self, now: Instant) -> Option<Instant> {
        let n = self.native.as_ref()?;
        if self.wake_at.is_some_and(|t| now >= t) {
            self.wake_at = None;
            self.limiter.request();
        }
        match self.limiter.poll(now) {
            Redraw::Now => {
                n.window.request_redraw();
                self.wake_at
            }
            Redraw::At(t) => Some(self.wake_at.map_or(t, |w| w.min(t))),
            Redraw::Idle => self.wake_at,
        }
    }

    /// The main window is closing: drop everything in order.
    pub fn shutdown(&mut self) {
        self.close();
    }
}

fn placement_of(w: &Window) -> Placement {
    let s = w.inner_size();
    Placement {
        pos: w.outer_position().ok().map(|p| (p.x, p.y)),
        size: (s.width.max(1), s.height.max(1)),
    }
}

// ---------------------------------------------------------------------------------------------
// Development data
// ---------------------------------------------------------------------------------------------

/// Environment flag: `APPLE2_RUST_PRINTER_DEMO=1` shows the Printer button, fills the Central with
/// a sample document and opens it at start (`=auto` opens it as an automatic opening would).
pub const DEMO_ENV: &str = "APPLE2_RUST_PRINTER_DEMO";

/// A sample document of `pages` 9.5" x 11" pages of pseudo text, plus a few jobs.
pub fn demo_view_model(pages: usize) -> PrinterViewModel {
    use apple2_printer::document::Dot;
    let (w, h) = (PER_INCH * 19 / 2, PER_INCH * 11);
    let mut doc = PageDocument::default();
    for p in 0..pages.max(1) {
        let mut page = Page {
            width: w,
            height: h,
            ..Default::default()
        };
        // 60 lines of 80 columns at 10 cpi / 6 lpi; glyph = 5x7 pseudo bitmap from a hash
        for line in 0..60u32 {
            let y0 = PER_INCH / 2 + line * 360;
            if y0 + 8 * 10 >= h {
                break;
            }
            let fill = 40 + (p as u32 * 7 + line * 13) % 40;
            for col in 0..fill {
                let seed =
                    (p as u32).wrapping_mul(2654435761) ^ line.wrapping_mul(40503) ^ (col * 977);
                if seed.is_multiple_of(7) {
                    continue; // a space
                }
                let x0 = PER_INCH * 3 / 4 + col * 216;
                for gx in 0..5u32 {
                    for gy in 0..7u32 {
                        let bit = seed.rotate_left(gx * 5 + gy).wrapping_mul(0x9E37_79B1) >> 28;
                        if bit & 1 == 1 || gx == 0 && gy < 6 && seed.is_multiple_of(3) {
                            page.dots.push(Dot {
                                x: x0 + gx * 27,
                                y: y0 + gy * 30,
                                ink: 0,
                            });
                        }
                    }
                }
            }
        }
        doc.pages.push(std::sync::Arc::new(page));
    }
    let entry = |n: u32, state: JobState| JobEntry {
        id: format!("demo-{n}"),
        title: format!("Trabalho {n}"),
        note: String::new(),
        machine: "Apple //e (enhanced)".into(),
        slot: 1,
        transport: "Automático".into(),
        started: "01/01/2026 10:00".into(),
        pages: pages as u32,
        state,
        active: state == JobState::Receiving,
        damaged: None,
        versions: Vec::new(),
        exports: Vec::new(),
    };
    doc.last_in_progress = true;
    PrinterViewModel {
        status: Status::Receiving,
        detail: "dados de demonstração".into(),
        machine: "Apple //e (enhanced)".into(),
        slot: Some(1),
        profile: "Epson LX-310".into(),
        setup: PrintSetup::default(),
        jobs: vec![
            entry(3, JobState::Failed),
            entry(2, JobState::Receiving),
            entry(1, JobState::Finished),
        ],
        active_job: Some(1),
        selected_job: Some(1),
        shown: Shown {
            job: Some("demo-2".into()),
            version: 0,
            label: "Trabalho 2".into(),
            document: Arc::new(doc),
            options: Default::default(),
            loading: false,
            live: true,
            probe: None,
        },
        messages: Vec::new(),
        revision: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_printer::document::Dot;

    fn at(ms: u64, base: Instant) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn events_route_by_window_id_and_only_main_reaches_the_emulator() {
        assert_eq!(route(1, Some(1), Some(2)), Route::Main);
        assert_eq!(route(2, Some(1), Some(2)), Route::Printer);
        assert_eq!(route(3, Some(1), Some(2)), Route::Unknown);
        assert_eq!(route(2, Some(1), None), Route::Unknown);
        assert!(Route::Main.reaches_emulator());
        assert!(!Route::Printer.reaches_emulator());
        assert!(!Route::Unknown.reaches_emulator());
    }

    #[test]
    fn open_close_reopen_keeps_placement_and_preview() {
        let mut w = PrinterWindow::default();
        assert!(!w.is_open());
        w.open(true);
        assert!(w.is_open());
        w.preview.next(5);
        w.preview.set_zoom(2.0);
        w.remember(Placement {
            pos: Some((10, 20)),
            size: (800, 600),
        });
        w.close();
        assert!(!w.is_open());
        w.open(false);
        assert!(w.is_open());
        assert_eq!(w.preview.page, 1);
        assert_eq!(w.preview.zoom, 2.0);
        assert_eq!(w.placement().unwrap().size, (800, 600));
        // closing never panics without a native window, and invalidating a closed one is a no-op
        w.close();
        w.invalidate();
        assert_eq!(w.drive(Instant::now()), None);
    }

    #[test]
    fn view_model_survives_the_window_closing() {
        let mut vm = demo_view_model(3);
        let mut w = PrinterWindow::default();
        w.open(false);
        w.close();
        vm.bump();
        assert_eq!(vm.document().pages.len(), 3);
        assert_eq!(vm.revision, 2);
    }

    #[test]
    fn navigation_and_zoom_are_clamped() {
        let mut p = PreviewState::default();
        p.prev();
        assert_eq!(p.page, 0);
        p.last(100);
        assert_eq!(p.page, 99);
        p.next(100);
        assert_eq!(p.page, 99);
        p.first();
        assert_eq!(p.page, 0);
        p.set_zoom(10.0);
        assert_eq!(p.zoom, MAX_ZOOM);
        p.set_zoom(0.01);
        assert_eq!(p.zoom, MIN_ZOOM);
        assert!(!p.fit_width);
        p.set_zoom(1.0);
        p.zoom_in();
        assert_eq!(p.zoom, 1.5);
        p.zoom_out();
        p.zoom_out();
        assert_eq!(p.zoom, 0.75);
        // the page count may shrink under the cursor
        p.page = 9;
        p.clamp_page(3);
        assert_eq!(p.page, 2);
        p.clamp_page(0);
        assert_eq!(p.page, 0);
    }

    #[test]
    fn fit_width_scales_the_page_to_the_available_width() {
        let p = PreviewState::default();
        let z = p.effective_zoom(912.0, 9.5); // 9.5" * 96 = 912 points at 100%
        assert!((z - 1.0).abs() < 1e-6);
        assert_eq!(p.effective_zoom(100_000.0, 9.5), MAX_ZOOM);
        assert_eq!(p.effective_zoom(10.0, 9.5), MIN_ZOOM);
        let mut m = p;
        m.set_zoom(2.0);
        assert_eq!(m.effective_zoom(50.0, 9.5), 2.0);
    }

    #[test]
    fn repaint_is_limited_to_about_60_per_second_and_coalesced() {
        let t0 = Instant::now();
        let mut l = RepaintLimiter::default();
        assert_eq!(l.poll(t0), Redraw::Idle);
        // a burst of 10 000 "bytes" before the first frame is one frame
        for _ in 0..10_000 {
            l.request();
        }
        assert_eq!(l.poll(t0), Redraw::Now);
        l.painted(t0);
        assert_eq!(l.poll(at(1, t0)), Redraw::Idle);
        l.request();
        match l.poll(at(5, t0)) {
            Redraw::At(t) => assert!(t.duration_since(t0) >= MIN_REPAINT_INTERVAL),
            other => panic!("{other:?}"),
        }
        assert_eq!(l.poll(at(17, t0)), Redraw::Now);
        // frames drawn over 1 s with a request every millisecond: at most ~60
        let mut l = RepaintLimiter::default();
        let mut frames = 0;
        for ms in 0..1000 {
            l.request();
            if l.poll(at(ms, t0)) == Redraw::Now {
                l.painted(at(ms, t0));
                frames += 1;
            }
        }
        assert!((50..=62).contains(&frames), "{frames}");
    }

    #[test]
    fn activity_holds_then_goes_out() {
        let t0 = Instant::now();
        let mut a = ActivityIndicator::default();
        assert!(!a.is_active(t0));
        a.set_activity(true, t0);
        assert!(a.is_active(at(100, t0)));
        assert!(!a.is_active(at(400, t0)));
        a.set_activity(true, at(400, t0));
        a.set_activity(false, at(410, t0));
        assert!(!a.is_active(at(411, t0)));
    }

    #[test]
    fn rasteriser_draws_dots_and_overstrike_darkens() {
        let page = Page {
            width: PER_INCH,
            height: PER_INCH,
            dots: vec![
                Dot {
                    x: 1080,
                    y: 1080,
                    ink: 0,
                },
                Dot {
                    x: 1080,
                    y: 1080,
                    ink: 0,
                },
                Dot {
                    x: 360,
                    y: 360,
                    ink: 0,
                },
            ],
            ..Default::default()
        };
        let img = rasterize_page(&page, 96, false);
        assert_eq!(img.size, [96, 96]);
        let px = |x: usize, y: usize| img.pixels[y * 96 + x];
        assert_eq!(px(0, 0), Color32::WHITE);
        let single = px(15, 15); // 360/2160*96 = 16
        let double = px(48, 48);
        assert!(single.r() < 255);
        assert!(double.r() < single.r());
    }

    #[test]
    fn zebra_alternates_bands_and_ppi_is_bounded() {
        let page = Page {
            width: PER_INCH * 19 / 2,
            height: PER_INCH * 11,
            ..Default::default()
        };
        let img = rasterize_page(&page, 48, true);
        let a = img.pixels[5 * img.size[0]];
        let b = img.pixels[(24 + 5) * img.size[0]];
        assert_ne!(a, b);
        assert!(raster_ppi(&page, MAX_ZOOM, 2.0) as f32 * 11.0 <= MAX_RASTER_SIDE + 11.0);
        assert_eq!(raster_ppi(&page, 1.0, 1.0), 96);
    }

    #[test]
    fn no_per_page_memory_beyond_the_visible_one() {
        // The cache holds a single texture slot, whatever the document size.
        let vm = demo_view_model(100);
        assert_eq!(vm.document().pages.len(), 100);
        let ctx = Context::default();
        let mut cache = PreviewCache::default();
        let mut ui_state = CenterUi::default();
        let run = |cache: &mut PreviewCache,
                   ui_state: &mut CenterUi,
                   page: usize,
                   vm: &PrinterViewModel| {
            let mut pv = PreviewState {
                page,
                ..Default::default()
            };
            let mut ev = Vec::new();
            let _ = ctx.run(egui::RawInput::default(), |c| {
                draw_center(c, &mut pv, cache, ui_state, vm, &mut ev)
            });
        };
        run(&mut cache, &mut ui_state, 0, &vm);
        run(&mut cache, &mut ui_state, 0, &vm);
        assert_eq!(cache.rasterisations, 1, "same page+document is cached");
        run(&mut cache, &mut ui_state, 1, &vm);
        assert_eq!(cache.rasterisations, 2);
        // a message does not change the pages: still cached
        let mut other = vm.clone();
        other.bump();
        other.messages.push(crate::printer_model::Message {
            text: "x".into(),
            error: false,
        });
        run(&mut cache, &mut ui_state, 1, &other);
        assert_eq!(cache.rasterisations, 2);
        // a new document (the spool published again) rasterises again
        let mut newer = vm.clone();
        newer.shown.document = Arc::new((*vm.shown.document).clone());
        run(&mut cache, &mut ui_state, 1, &newer);
        assert_eq!(cache.rasterisations, 3, "a new document rasterises again");
    }

    #[test]
    fn the_preview_look_follows_the_jobs_paper_and_ink() {
        let page = Page {
            width: PER_INCH * 19 / 2,
            height: PER_INCH * 11,
            dots: vec![apple2_printer::document::Dot {
                x: 1080,
                y: 1080,
                ink: 0,
            }],
            ..Default::default()
        };
        let look = |paper, ink| Look {
            paper,
            ink,
            ribbon: Ribbon::New,
        };
        let green = rasterize_page_look(&page, 48, &look(PaperKind::GreenBar, InkColor::Black));
        let blue = rasterize_page_look(&page, 48, &look(PaperKind::BlueBar, InkColor::Black));
        let a4 = rasterize_page_look(&page, 48, &look(PaperKind::A4, InkColor::Black));
        let w = green.size[0];
        let bar_px = |img: &ColorImage| img.pixels[(24 + 5) * w + 100];
        assert_ne!(bar_px(&green), bar_px(&blue), "green and blue bars differ");
        assert_ne!(bar_px(&green), Color32::WHITE);
        // a sheet (even if taller than wide) has no bars
        let a4_w = a4.size[0];
        assert_eq!(a4.pixels[(24 + 5) * a4_w + 100], Color32::WHITE);
        // ink colour
        let dot = |ink| {
            let img = rasterize_page_look(&page, 96, &look(PaperKind::A4, ink));
            img.pixels[img.size[0] * 48 + 48]
        };
        assert!(dot(InkColor::Blue).b() > dot(InkColor::Blue).r());
        assert!(dot(InkColor::Purple).r() > dot(InkColor::Black).r());
    }
}
