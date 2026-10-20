//! The winit application: window, input routing, actions, audio and gamepad glue.

use crate::actions::{action_for, Action};
use crate::audio::AudioOut;
use crate::backend::BackendFactory;
use crate::emu::{self, default_state_path, Command, EmuHandle, EmuStatus};
use crate::gfx::Gfx;
use crate::host;
use crate::joy::{axis_to_paddle, mouse_to_paddle, JoyMode, JoyOutput, KeyJoy, CENTRE};
use crate::keymap::{KeyEvent as AppleKey, KeyboardState, Mods};
use crate::screenshot::{next_screenshot_path, save_png};
use crate::toolbar;
use crate::ui::{UiEvent, UiState};
use crate::video::Frame;
use apple2_config::settings::{Joystick0Kind, Joystick1Kind};
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

#[derive(Default)]
struct Pad {
    lx: f32,
    ly: f32,
    rx: f32,
    ry: f32,
    buttons: [bool; 2],
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
    frame: Frame,
    frame_seq: u64,
    status: EmuStatus,
    last_title: String,
    last_message_seq: u64,
    mods: ModifiersState,
    kbd: KeyboardState,
    keyjoy: [KeyJoy; 2],
    joy_kinds: [JoyMode; 2],
    gilrs: Option<gilrs::Gilrs>,
    pads: [Pad; 2],
    pad_ids: Vec<gilrs::GamepadId>,
    clipboard: Option<arboard::Clipboard>,
    cursor: Option<(f32, f32)>,
    mouse_captured: bool,
    mouse_joy_buttons: [bool; 2],
    scale_toggle: bool,
    dump_window: Option<(PathBuf, u64)>,
    exit_after_capture: bool,
}

fn joy_modes(s: &Settings) -> [JoyMode; 2] {
    let j0 = match s.joystick.joystick0 {
        Joystick0Kind::Disabled => JoyMode::Disabled,
        Joystick0Kind::HostJoystick1 => JoyMode::Gamepad,
        Joystick0Kind::KeyboardCursors => JoyMode::KeyboardCursors,
        Joystick0Kind::KeyboardNumpad => JoyMode::KeyboardNumpad,
        Joystick0Kind::Mouse => JoyMode::Mouse,
    };
    let j1 = match s.joystick.joystick1 {
        Joystick1Kind::Disabled => JoyMode::Disabled,
        Joystick1Kind::HostJoystick2 | Joystick1Kind::HostJoystick1Thumbstick2 => JoyMode::Gamepad,
        Joystick1Kind::KeyboardCursors => JoyMode::KeyboardCursors,
        Joystick1Kind::KeyboardNumpad => JoyMode::KeyboardNumpad,
        Joystick1Kind::Mouse => JoyMode::Mouse,
    };
    [j0, j1]
}

impl App {
    fn new(opts: Options, proxy: EventLoopProxy<UserEvent>) -> Result<App, String> {
        let Options { settings, factory, no_audio, load_state, open, dump_window, type_text } = opts;
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
        let modes = joy_modes(&settings);
        for (i, m) in modes.iter().enumerate() {
            emu.send(Command::JoyAttached { index: i, attached: *m != JoyMode::Disabled });
        }
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
            kbd: KeyboardState::new(),
            keyjoy: [KeyJoy::new(modes[0]), KeyJoy::new(modes[1])],
            joy_kinds: modes,
            gilrs: gilrs::Gilrs::new().map_err(|e| eprintln!("gamepads unavailable: {e}")).ok(),
            pads: Default::default(),
            pad_ids: Vec::new(),
            clipboard: None,
            cursor: None,
            mouse_captured: false,
            mouse_joy_buttons: [false; 2],
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
            Action::Help => self.ui.message = "Help: https://github.com/AppleWin/AppleWin/wiki (F1)".into(),
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
                self.ui.toggle_settings(&self.settings);
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
            Action::Screenshot => self.screenshot(),
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
            self.ui.disk.note_inserted(&path);
            self.emu.send(Command::InsertDisk { drive, path });
        }
    }

    fn screenshot(&mut self) {
        let mut f = Frame::new(0, 0);
        self.emu.shared.copy_frame(&mut f);
        let dir = std::env::current_dir().unwrap_or_default();
        let path = next_screenshot_path(&dir);
        self.ui.message = match save_png(&path, &f) {
            Ok(()) => format!("Screenshot: {}", path.display()),
            Err(e) => format!("Screenshot failed: {e}"),
        };
    }

    fn paste(&mut self) {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(text) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
            self.emu.send(Command::Paste(text.replace("\r\n", "\r").replace('\n', "\r")));
        }
    }

    // ---- input ----

    fn send_joy(&self, index: usize) {
        let out = match self.joy_kinds[index] {
            JoyMode::Disabled => return,
            JoyMode::KeyboardNumpad | JoyMode::KeyboardCursors => self.keyjoy[index].output(),
            JoyMode::Gamepad => {
                let (x, y, b) = if index == 0 {
                    (self.pads[0].lx, self.pads[0].ly, self.pads[0].buttons)
                } else if self.settings.joystick.joystick1 == Joystick1Kind::HostJoystick1Thumbstick2 {
                    (self.pads[0].rx, self.pads[0].ry, self.pads[0].buttons)
                } else {
                    (self.pads[1].lx, self.pads[1].ly, self.pads[1].buttons)
                };
                JoyOutput { x: axis_to_paddle(x, 0.1), y: axis_to_paddle(y, 0.1), buttons: b }
            }
            JoyMode::Mouse => {
                let (x, y) = match (self.cursor, self.ui.screen_rect) {
                    (Some((cx, cy)), Some(r)) => {
                        (mouse_to_paddle(cx - r.min.x, r.width()), mouse_to_paddle(cy - r.min.y, r.height()))
                    }
                    _ => (CENTRE, CENTRE),
                };
                JoyOutput { x, y, buttons: self.mouse_joy_buttons }
            }
        };
        self.emu.send(Command::Joy { index, out });
    }

    fn poll_gamepads(&mut self) {
        use gilrs::{Axis, Button, EventType};
        let Some(g) = self.gilrs.as_mut() else { return };
        let mut changed = false;
        while let Some(ev) = g.next_event() {
            if !self.pad_ids.contains(&ev.id) && self.pad_ids.len() < 2 {
                self.pad_ids.push(ev.id);
            }
            let Some(slot) = self.pad_ids.iter().position(|i| *i == ev.id) else { continue };
            let p = &mut self.pads[slot];
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
            changed = true;
        }
        if changed {
            self.send_joy(0);
            self.send_joy(1);
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
        // keyboard-as-joystick takes its keys first (AppleWin JoyProcessKey)
        for i in 0..2 {
            if self.keyjoy[i].process(input, pressed) {
                self.send_joy(i);
                return;
            }
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
        for i in 0..2 {
            self.keyjoy[i].release_all();
            self.send_joy(i);
        }
        self.set_mouse_capture(false);
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

    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        let idx = match button {
            MouseButton::Left => 0u8,
            MouseButton::Right => 1,
            _ => return,
        };
        let over_screen = matches!((self.cursor, self.ui.screen_rect), (Some((x, y)), Some(r)) if r.contains(egui::pos2(x, y)));
        if self.mouse_captured {
            if pressed && (self.mods.control_key() || self.mods.shift_key()) {
                self.set_mouse_capture(false); // AppleWin: Ctrl/Shift+click releases the mouse
            } else {
                self.emu.send(Command::MouseButton { button: idx, down: pressed });
            }
        } else if pressed && over_screen && self.status.mouse_available && !self.mods.control_key() {
            self.set_mouse_capture(true);
        }
        if self.joy_kinds.contains(&JoyMode::Mouse) && over_screen {
            self.mouse_joy_buttons[usize::from(idx)] = pressed;
            for i in 0..2 {
                self.send_joy(i);
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
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::ApplySettings(s));
                }
                UiEvent::ApplyLive(s) => {
                    self.settings = (*s).clone();
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::UpdateSettings(s));
                }
                UiEvent::Volume(v) => self.emu.send(Command::SetVolume(v)),
                UiEvent::EjectDisk(d) => self.emu.send(Command::EjectDisk { drive: d }),
                UiEvent::InsertDisk { drive, path } => {
                    self.ui.disk.note_inserted(&path);
                    self.emu.send(Command::InsertDisk { drive, path });
                }
                UiEvent::SetWriteProtect { drive, on } => self.emu.send(Command::SetWriteProtect { drive, on }),
                UiEvent::Paste => self.paste(),
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
        let input = state.take_egui_input(&window);
        let fullscreen = window.fullscreen().is_some();
        let mut events = Vec::new();
        let status = &self.status;
        let ui = &mut self.ui;
        let output = self.egui_ctx.run(input, |ctx| events = ui.draw(ctx, status, fullscreen));
        state.handle_platform_output(&window, output.platform_output.clone());
        let repaint = output.viewport_output.values().any(|v| v.repaint_delay.is_zero());
        if let Some((path, frames)) = &self.dump_window {
            // egui windows are invisible during their first (sizing) pass: wait a few passes
            if self.status.frames >= *frames && self.egui_ctx.cumulative_pass_nr() > 6 {
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
        let attrs = Window::default_attributes().with_title("AppleII-Rust").with_inner_size(window_size(self.settings.video.window_scale, true));
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
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::Focused(false) => self.release_inputs(),
            WindowEvent::KeyboardInput { event, .. } if !egui_wants_keys => self.keyboard(&event),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x as f32 / self.egui_ctx.pixels_per_point(), position.y as f32 / self.egui_ctx.pixels_per_point()));
                if self.joy_kinds.contains(&JoyMode::Mouse) {
                    for i in 0..2 {
                        self.send_joy(i);
                    }
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
