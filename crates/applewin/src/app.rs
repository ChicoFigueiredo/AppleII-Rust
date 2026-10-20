//! The winit application: window, input routing, actions, audio and gamepad glue.

use crate::actions::{action_for, Action};
use crate::audio::AudioOut;
use crate::backend::BackendFactory;
use crate::emu::{self, default_state_path, Command, EmuHandle, EmuStatus};
use crate::gfx::Gfx;
use crate::host;
use crate::joy::{mouse_to_paddle, JoyConfig, JoyFinal, JoySystem, KeyUse};
use crate::keymap::{HostKey, KeyEvent as AppleKey, KeyboardState, Mods};
use crate::overlay::MouseState;
use crate::pacing::Speed;
use crate::screenshot::{next_screenshot_path, save_png};
use crate::toolbar;
use crate::ui::{UiEvent, UiState};
use crate::video::Frame;
use apple2_config::Settings;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

#[derive(Debug)]
pub enum UserEvent {
    /// The emulation thread published a frame.
    Frame,
}

pub struct Options {
    pub settings: Settings,
    pub factory: BackendFactory,
    pub no_audio: bool,
    pub load_state: Option<PathBuf>,
    /// Disk images / states given on the command line (opened by file association in AppleWin).
    pub open: Vec<PathBuf>,
    /// `--dump-window FILE --window-frames N`: save the whole window after N emulated frames, then exit.
    pub dump_window: Option<(PathBuf, u64)>,
    /// Text pasted into the machine at start (testing).
    pub type_text: Option<String>,
}

pub fn run(opts: Options) -> Result<(), String> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App::new(opts, proxy)?;
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

struct App {
    settings: Settings,
    emu: EmuHandle,
    _audio: Option<crate::audio::AudioOut>,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    ui: UiState,
    help: crate::help::HelpViewer,
    frame: Frame,
    frame_seq: u64,
    status: EmuStatus,
    last_title: String,
    last_message_seq: u64,
    mods: ModifiersState,
    kbd: KeyboardState,
    joy: JoySystem,
    /// What the machine was last told about the joysticks.
    last_joy: Option<JoyFinal>,
    started: std::time::Instant,
    gilrs: Option<gilrs::Gilrs>,
    pad_ids: Vec<gilrs::GamepadId>,
    clipboard: Option<arboard::Clipboard>,
    cursor: Option<(f32, f32)>,
    /// AppleMouse card capture (relative motion, cursor locked).
    mouse_captured: bool,
    /// Mouse-as-joystick capture (absolute position, cursor confined and hidden).
    joy_cursor: bool,
    last_copied_seq: u64,
    scale_toggle: bool,
    dump_window: Option<(PathBuf, u64)>,
    exit_after_capture: bool,
}

impl App {
    fn new(opts: Options, proxy: EventLoopProxy<UserEvent>) -> Result<App, String> {
        let Options { settings, factory, no_audio, load_state, open, dump_window, type_text } = opts;
        let settings_caps = settings.keyboard.caps_lock;
        let settings_for_joy = settings.clone();
        let audio = if no_audio || settings.sound.disable_sound {
            None
        } else {
            match AudioOut::open() {
                Ok(a) => Some(a),
                Err(e) => {
                    eprintln!("audio disabled: {e}");
                    None
                }
            }
        };
        let (ring, rate) = match &audio {
            Some(a) => (Some(a.ring.clone()), a.sample_rate),
            None => (None, 48_000),
        };
        let emu = emu::spawn(settings.clone(), factory, ring, rate, Box::new(move || drop(proxy.send_event(UserEvent::Frame))))?;
        if let Some(p) = load_state {
            emu.send(Command::LoadState(p));
        }
        for p in open {
            emu.send(open_command(&p));
        }
        if let Some(t) = type_text {
            emu.send(Command::Paste(t));
        }
        Ok(App {
            dump_window,
            exit_after_capture: false,
            ui: UiState::new(&settings),
            help: crate::help::HelpViewer::new(),
            settings,
            emu,
            _audio: audio,
            window: None,
            gfx: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            frame: Frame::new(0, 0),
            frame_seq: 0,
            status: EmuStatus::default(),
            last_title: String::new(),
            last_message_seq: 0,
            mods: ModifiersState::empty(),
            kbd: KeyboardState::with_caps(settings_caps),
            joy: JoySystem::new(JoyConfig::from_settings(&settings_for_joy)),
            last_joy: None,
            started: std::time::Instant::now(),
            gilrs: gilrs::Gilrs::new().map_err(|e| eprintln!("gamepads unavailable: {e}")).ok(),
            pad_ids: Vec::new(),
            clipboard: None,
            cursor: None,
            mouse_captured: false,
            joy_cursor: false,
            last_copied_seq: 0,
            scale_toggle: false,
        })
    }

    fn mods(&self) -> Mods {
        host::mods(self.mods)
    }

    fn window(&self) -> Option<&Arc<Window>> {
        self.window.as_ref()
    }

    // ---- actions ----

    fn do_action(&mut self, a: Action) {
        match a {
            Action::Help => self.help.toggle(),
            Action::Reboot => self.emu.send(Command::PowerCycle),
            Action::SoftReset => self.emu.send(Command::Reset),
            Action::DriveSelect(d) => self.pick_disk(usize::from(d)),
            Action::DriveMenu(d) => self.ui.drive_menu = Some(usize::from(d)),
            Action::SwapDrives => self.emu.send(Command::SwapDrives),
            Action::ToggleFullscreen => {
                if let Some(w) = self.window() {
                    let fs = if w.fullscreen().is_some() { None } else { Some(Fullscreen::Borderless(None)) };
                    w.set_fullscreen(fs);
                }
            }
            Action::ToggleScale => {
                self.scale_toggle = !self.scale_toggle;
                let s = if self.scale_toggle { 1 } else { 2 };
                if let Some(w) = self.window() {
                    let _ = w.request_inner_size(window_size(s, self.ui.show_toolbar));
                }
            }
            Action::ToggleDebugger => self.emu.send(Command::ToggleDebugger),
            Action::Settings => {
                self.ui.draft = self.settings.clone();
                self.ui.show_settings = !self.ui.show_settings;
            }
            Action::NextVideoMode => self.emu.send(Command::VideoMode(1)),
            Action::PrevVideoMode => self.emu.send(Command::VideoMode(-1)),
            Action::ToggleScanlines => self.emu.send(Command::ToggleScanlines),
            Action::RockerSwitch => self.emu.send(Command::RockerSwitch),
            Action::SaveState => self.emu.send(Command::SaveState(default_state_path(&self.settings))),
            Action::LoadState => self.emu.send(Command::LoadState(default_state_path(&self.settings))),
            Action::TogglePause => self.emu.send(Command::TogglePause),
            Action::ToggleFullSpeed => self.emu.send(Command::ToggleFullSpeed),
            Action::Step => self.emu.send(Command::Step),
            Action::Screenshot => self.screenshot(false),
            Action::ScreenshotSmall => self.screenshot(true),
            Action::CopyScreenText => self.emu.send(Command::CopyScreenText),
            Action::SpeedToggleMax => self.toggle_max_speed(),
            Action::Speed1Mhz => self.emu.send(Command::SetSpeed(Speed::NORMAL)),
            Action::SpeedMax => self.emu.send(Command::SetSpeed(Speed::Max)),
        }
    }

    fn pick_disk(&mut self, drive: usize) {
        // Pause while the modal dialog is open so sound does not stutter and time does not pass.
        let was_paused = self.status.paused;
        self.emu.send(Command::SetPaused(true));
        let mut dialog = rfd::FileDialog::new().set_title(format!("Select disk image for drive {}", drive + 1)).add_filter(
            "Disk images",
            &["dsk", "do", "po", "nib", "woz", "2mg", "2img", "hdv", "gz", "zip", "iie"],
        );
        if let Some(dir) = &self.settings.storage.disk_dir {
            dialog = dialog.set_directory(dir);
        }
        let picked = dialog.pick_file();
        self.emu.send(Command::SetPaused(was_paused));
        if let Some(path) = picked {
            self.emu.send(Command::InsertDisk { drive, path });
        }
    }

    fn screenshot(&mut self, small: bool) {
        let mut f = Frame::new(0, 0);
        self.emu.shared.copy_frame(&mut f);
        if small {
            f = halve(&f);
        }
        let dir = std::env::current_dir().unwrap_or_default();
        let path = next_screenshot_path(&dir);
        self.ui.message = match save_png(&path, &f) {
            Ok(()) => format!("Screenshot: {}", path.display()),
            Err(e) => format!("Screenshot failed: {e}"),
        };
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    fn paste(&mut self) {
        if let Some(text) = self.clipboard().and_then(|c| c.get_text().ok()) {
            self.emu.send(Command::Paste(text.replace("\r\n", "\r").replace('\n', "\r")));
        }
    }

    /// Ctrl+0: custom speed <-> full speed.
    fn toggle_max_speed(&mut self) {
        let custom = match emu::speed_from_settings(&self.settings) {
            Speed::Max => Speed::NORMAL,
            s => s,
        };
        self.emu.send(Command::SetSpeed(if self.status.full_speed { custom } else { Speed::Max }));
    }

    // ---- input ----

    /// Sends the machine whatever changed in the joystick state (also advances autofire).
    fn update_joy(&mut self) {
        let now = self.started.elapsed().as_micros() as u64;
        let f = self.joy.output(now);
        let prev = self.last_joy;
        if prev == Some(f) {
            return;
        }
        for i in 0..2 {
            if prev.is_none_or(|p| p.attached[i] != f.attached[i]) {
                self.emu.send(Command::JoyAttached { index: i, attached: f.attached[i] });
            }
            if prev.is_none_or(|p| p.out[i] != f.out[i]) {
                self.emu.send(Command::Joy { index: i, out: f.out[i] });
            }
        }
        self.last_joy = Some(f);
    }

    fn poll_gamepads(&mut self) {
        use gilrs::{Axis, Button, EventType};
        if let Some(g) = self.gilrs.as_mut() {
            while let Some(ev) = g.next_event() {
                if let EventType::Disconnected = ev.event {
                    if let Some(slot) = self.pad_ids.iter().position(|i| *i == ev.id) {
                        self.pad_ids.remove(slot);
                        if slot == 0 {
                            self.joy.pads[0] = self.joy.pads[1];
                        }
                        self.joy.pads[1] = Default::default();
                        self.joy.set_pads_connected(self.pad_ids.len());
                    }
                    continue;
                }
                if !self.pad_ids.contains(&ev.id) && self.pad_ids.len() < 2 {
                    self.pad_ids.push(ev.id);
                    self.joy.set_pads_connected(self.pad_ids.len());
                }
                let Some(slot) = self.pad_ids.iter().position(|i| *i == ev.id) else { continue };
                let p = &mut self.joy.pads[slot];
                match ev.event {
                    EventType::AxisChanged(Axis::LeftStickX, v, _) => p.lx = v,
                    EventType::AxisChanged(Axis::LeftStickY, v, _) => p.ly = -v,
                    EventType::AxisChanged(Axis::RightStickX, v, _) => p.rx = v,
                    EventType::AxisChanged(Axis::RightStickY, v, _) => p.ry = -v,
                    EventType::ButtonPressed(b, _) | EventType::ButtonReleased(b, _) => {
                        let down = matches!(ev.event, EventType::ButtonPressed(..));
                        match b {
                            Button::South | Button::West => p.buttons[0] = down,
                            Button::East | Button::North => p.buttons[1] = down,
                            Button::DPadLeft => p.lx = if down { -1.0 } else { 0.0 },
                            Button::DPadRight => p.lx = if down { 1.0 } else { 0.0 },
                            Button::DPadUp => p.ly = if down { -1.0 } else { 0.0 },
                            Button::DPadDown => p.ly = if down { 1.0 } else { 0.0 },
                            _ => {}
                        }
                    }
                    _ => continue,
                }
            }
        }
        self.update_joy();
    }

    /// Scroll Lock: full speed while held, or toggled when "ScrollLock Toggle" is set.
    fn scroll_lock(&mut self, pressed: bool, repeat: bool) {
        if repeat {
            return;
        }
        if pressed || !self.settings.keyboard.scroll_lock_toggle {
            self.emu.send(Command::ToggleFullSpeed);
        }
    }

    fn keyboard(&mut self, ev: &winit::event::KeyEvent) {
        let pressed = ev.state == ElementState::Pressed;
        let input = host::key_input(ev, self.mods);
        let mods = self.mods();
        if pressed && !ev.repeat {
            if let Some(a) = action_for(input.key, mods) {
                self.do_action(a);
                return;
            }
        } else if action_for(input.key, mods).is_some() {
            return; // repeats and releases of bound keys
        }
        if self.status.debugger_active {
            if pressed {
                if let Some(k) = crate::keymap::debug_key(input.key, mods) {
                    self.emu.send(Command::Debug(k));
                }
            }
            return;
        }
        if input.key == HostKey::ScrollLock {
            self.scroll_lock(pressed, ev.repeat);
        }
        // the game port takes its keys first (AppleWin JoyProcessKey)
        match self.joy.process_key(input, pressed, ev.repeat) {
            KeyUse::Joystick => {
                self.update_joy();
                return;
            }
            KeyUse::JoystickAndApple => self.update_joy(),
            KeyUse::Ignored => {}
        }
        for e in self.kbd.handle(input, pressed, ev.repeat, mods) {
            match e {
                AppleKey::Paste => self.paste(),
                other => self.emu.send(Command::Key(other)),
            }
        }
    }

    fn release_inputs(&mut self) {
        for e in self.kbd.release_all() {
            self.emu.send(Command::Key(e));
        }
        self.joy.release_all();
        self.update_joy();
        self.set_mouse_capture(false);
        self.set_joy_cursor(false);
    }

    fn set_mouse_capture(&mut self, on: bool) {
        if self.mouse_captured == on {
            return;
        }
        self.mouse_captured = on;
        if let Some(w) = &self.window {
            if on {
                let _ = w.set_cursor_grab(CursorGrabMode::Locked).or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined));
            } else {
                let _ = w.set_cursor_grab(CursorGrabMode::None);
            }
            w.set_cursor_visible(!on);
        }
    }

    /// Mouse as joystick: the pointer is confined to the window and hidden while it is "in".
    fn set_joy_cursor(&mut self, on: bool) {
        if self.joy_cursor == on {
            return;
        }
        self.joy_cursor = on;
        if let Some(w) = &self.window {
            let _ = w.set_cursor_grab(if on { CursorGrabMode::Confined } else { CursorGrabMode::None });
            w.set_cursor_visible(!on);
        }
        if !on {
            self.joy.set_mouse_button(0, false);
            self.joy.set_mouse_button(1, false);
            self.update_joy();
        }
    }

    /// Pointer position inside the Apple screen as paddle values.
    fn mouse_joy_position(&mut self) {
        if let (Some((cx, cy)), Some(r)) = (self.cursor, self.ui.screen_rect) {
            self.joy.set_mouse_position(mouse_to_paddle(cx - r.min.x, r.width()), mouse_to_paddle(cy - r.min.y, r.height()));
            self.update_joy();
        }
    }

    fn cursor_over_screen(&self) -> bool {
        matches!((self.cursor, self.ui.screen_rect), (Some((x, y)), Some(r)) if r.contains(egui::pos2(x, y)))
    }

    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        let idx = match button {
            MouseButton::Left => 0u8,
            MouseButton::Right => 1,
            _ => return,
        };
        let over_screen = self.cursor_over_screen();
        let release = self.mods.control_key() || self.mods.shift_key();
        if self.mouse_captured {
            if pressed && release {
                self.set_mouse_capture(false); // AppleWin: Ctrl/Shift+click releases the mouse
            } else {
                self.emu.send(Command::MouseButton { button: idx, down: pressed });
            }
        } else if self.joy_cursor {
            if pressed && release {
                self.set_joy_cursor(false);
            } else {
                self.joy.set_mouse_button(usize::from(idx), pressed);
                self.update_joy();
            }
        } else if pressed && over_screen && !self.mods.control_key() {
            // The AppleMouse card takes the mouse before the joystick does.
            if self.status.mouse_available {
                self.set_mouse_capture(true);
            } else if self.joy.using_mouse() {
                self.set_joy_cursor(true); // this click only captures, as in AppleWin
                self.mouse_joy_position();
            }
        }
    }

    // ---- frame ----

    fn handle_ui_events(&mut self, events: Vec<UiEvent>) {
        for e in events {
            match e {
                UiEvent::Action(a) => self.do_action(a),
                UiEvent::Apply(s) => {
                    self.settings = (*s).clone();
                    self.joy.set_config(JoyConfig::from_settings(&self.settings));
                    if !self.joy.using_mouse() {
                        self.set_joy_cursor(false);
                    }
                    self.update_joy();
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::ApplySettings(s));
                }
                UiEvent::Volume(v) => self.emu.send(Command::SetVolume(v)),
                UiEvent::EjectDisk(d) => self.emu.send(Command::EjectDisk { drive: d }),
            }
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(gfx), Some(state)) = (self.window.clone(), self.gfx.as_mut(), self.egui_state.as_mut()) else { return };
        let seq = self.emu.shared.frame_seq();
        if seq != self.frame_seq {
            self.frame_seq = seq;
            self.emu.shared.copy_frame(&mut self.frame);
            self.status = self.emu.shared.status();
            self.ui.upload(&self.egui_ctx, &self.frame);
            if self.status.copied_seq != self.last_copied_seq {
                self.last_copied_seq = self.status.copied_seq;
                let text = self.status.copied_text.clone();
                if self.clipboard.is_none() {
                    self.clipboard = arboard::Clipboard::new().ok();
                }
                if let Some(c) = self.clipboard.as_mut() {
                    let _ = c.set_text(text);
                }
            }
            if self.status.message_seq != self.last_message_seq {
                self.last_message_seq = self.status.message_seq;
                self.ui.message = self.status.message.clone();
            }
            let t = toolbar::title(&self.status);
            if t != self.last_title {
                window.set_title(&t);
                self.last_title = t;
            }
        }
        self.ui.mouse = MouseState {
            usable: self.status.mouse_available || self.joy.using_mouse(),
            captured: self.mouse_captured || self.joy_cursor,
            over_screen: matches!((self.cursor, self.ui.screen_rect), (Some((x, y)), Some(r)) if r.contains(egui::pos2(x, y))),
        };
        let input = state.take_egui_input(&window);
        let fullscreen = window.fullscreen().is_some();
        let mut events = Vec::new();
        let status = &self.status;
        let ui = &mut self.ui;
        let help = &mut self.help;
        let output = self.egui_ctx.run(input, |ctx| {
            events = ui.draw(ctx, status, fullscreen);
            help.draw(ctx);
        });
        state.handle_platform_output(&window, output.platform_output.clone());
        let repaint = output.viewport_output.values().any(|v| v.repaint_delay.is_zero());
        if let Some((path, frames)) = &self.dump_window {
            if self.status.frames >= *frames {
                gfx.request_capture(path.clone());
                self.dump_window = None;
                self.exit_after_capture = true;
            }
        }
        gfx.render(&self.egui_ctx, output);
        if self.exit_after_capture && gfx.captured {
            self.emu.shutdown();
            std::process::exit(0);
        }
        if self.exit_after_capture || self.dump_window.is_some() {
            window.request_redraw();
        }
        if repaint {
            window.request_redraw();
        }
        self.handle_ui_events(events);
    }
}

/// 560x384 to 280x192 (Shift+Print Screen): every second pixel of every second line.
fn halve(f: &Frame) -> Frame {
    let (w, h) = (f.width / 2, f.height / 2);
    let mut out = Frame::new(w, h);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let src = ((y * 2) * f.width as usize + x * 2) * 4;
            let dst = (y * w as usize + x) * 4;
            out.rgba[dst..dst + 4].copy_from_slice(&f.rgba[src..src + 4]);
        }
    }
    out
}

fn open_command(p: &std::path::Path) -> Command {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name.ends_with(".aws.json") || name.ends_with(".aws.yaml") {
        Command::LoadState(p.to_path_buf())
    } else {
        Command::InsertDisk { drive: 0, path: p.to_path_buf() }
    }
}

fn window_size(scale: u32, toolbar: bool) -> LogicalSize<f64> {
    let s = f64::from(scale.clamp(1, 4));
    LogicalSize::new(560.0 * s + if toolbar { 92.0 + 17.0 } else { 0.0 }, 384.0 * s + if toolbar { 34.0 } else { 0.0 })
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("AppleII-Rust").with_window_icon(crate::icon::window_icon()).with_inner_size(window_size(self.settings.video.window_scale, true));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("cannot create window: {e}");
                el.exit();
                return;
            }
        };
        match Gfx::new(window.clone()) {
            Ok(g) => self.gfx = Some(g),
            Err(e) => {
                eprintln!("graphics init failed: {e}");
                el.exit();
                return;
            }
        }
        self.egui_state = Some(egui_winit::State::new(self.egui_ctx.clone(), egui::ViewportId::ROOT, &window, None, None, None));
        if self.settings.video.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        window.set_ime_allowed(false);
        self.window = Some(window);
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, _e: UserEvent) {
        self.poll_gamepads();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut egui_wants_keys = false;
        if let (Some(state), Some(window)) = (self.egui_state.as_mut(), self.window.as_ref()) {
            let r = state.on_window_event(window, &event);
            if r.repaint {
                window.request_redraw();
            }
            egui_wants_keys = r.consumed && self.egui_ctx.wants_keyboard_input();
            if r.consumed && !matches!(event, WindowEvent::KeyboardInput { .. } | WindowEvent::RedrawRequested | WindowEvent::CloseRequested) {
                // pointer events over egui widgets do not reach the Apple
                if !matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::ModifiersChanged(_) | WindowEvent::Focused(_)) {
                    return;
                }
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                self.emu.shutdown();
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(g) = self.gfx.as_mut() {
                    g.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                self.joy.set_shift(self.mods.shift_key());
                self.update_joy();
            }
            WindowEvent::Focused(false) => self.release_inputs(),
            WindowEvent::KeyboardInput { event, .. } if !egui_wants_keys => self.keyboard(&event),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x as f32 / self.egui_ctx.pixels_per_point(), position.y as f32 / self.egui_ctx.pixels_per_point()));
                if self.joy_cursor {
                    self.mouse_joy_position();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(button, state == ElementState::Pressed),
            WindowEvent::DroppedFile(path) => {
                self.emu.send(open_command(&path));
                if matches!(open_command(&path), Command::InsertDisk { .. }) {
                    self.emu.send(Command::PowerCycle);
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta }, true) = (event, self.mouse_captured) {
            self.emu.send(Command::MouseMove { dx: delta.0.round() as i32, dy: delta.1.round() as i32 });
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        // gamepads are polled with every published frame (user_event); nothing else to do
        if self.gilrs.is_some() {
            self.poll_gamepads();
        }
    }
}
