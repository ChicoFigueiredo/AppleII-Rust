//! The winit application: window, input routing, actions, audio and gamepad glue.

use crate::actions::{action_for_mode_with, Action};
use crate::audio::AudioOut;
use crate::backend::BackendFactory;
use crate::emu::{self, default_state_path, Command, EmuHandle, EmuStatus};
use crate::gfx::Gfx;
use crate::host;
use crate::input::{InputRouter, KeyCtx, Routed};
use crate::joy::{mouse_to_paddle, JoyConfig};
use crate::keymap::Mods;
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
    /// A termination signal (SIGINT/SIGTERM/SIGHUP, console close) asked to quit: leave the
    /// event loop so the emulation thread flushes the disks (see [`request_exit`]).
    Exit,
}

static EXIT_PROXY: std::sync::OnceLock<EventLoopProxy<UserEvent>> = std::sync::OnceLock::new();

/// Asks the running window to close as if the user had; callable from any thread (signal
/// handlers). Returns false when no event loop is running yet.
pub fn request_exit() -> bool {
    EXIT_PROXY
        .get()
        .is_some_and(|p| p.send_event(UserEvent::Exit).is_ok())
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
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let _ = EXIT_PROXY.set(proxy.clone());
    let mut app = App::new(opts, proxy)?;
    let result = event_loop.run_app(&mut app).map_err(|e| e.to_string());
    // Normally already done in `exiting()`; harmless if so.
    app.release_gpu();
    result
}

struct App {
    settings: Settings,
    emu: EmuHandle,
    _audio: Option<crate::audio::AudioOut>,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    quit: bool,
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
    input: InputRouter,
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
    bench_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

impl App {
    fn new(opts: Options, proxy: EventLoopProxy<UserEvent>) -> Result<App, String> {
        let Options {
            settings,
            factory,
            no_audio,
            load_state,
            open,
            dump_window,
            type_text,
        } = opts;
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
        let emu = emu::spawn(
            settings.clone(),
            factory,
            ring,
            rate,
            Box::new(move || drop(proxy.send_event(UserEvent::Frame))),
        )?;
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
            bench_rx: None,
            ui: UiState::new(&settings),
            help: crate::help::HelpViewer::new(),
            settings,
            emu,
            _audio: audio,
            window: None,
            gfx: None,
            quit: false,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            frame: Frame::new(0, 0),
            frame_seq: 0,
            status: EmuStatus::default(),
            last_title: String::new(),
            last_message_seq: 0,
            mods: ModifiersState::empty(),
            input: InputRouter::new(&settings_for_joy),
            started: std::time::Instant::now(),
            gilrs: gilrs::Gilrs::new()
                .map_err(|e| eprintln!("gamepads unavailable: {e}"))
                .ok(),
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
        if crate::input::releases_keys(a) {
            self.release_keys();
        }
        match a {
            Action::Help => self.help.toggle(),
            Action::Reboot => {
                if self.ui.request_reboot(self.settings.misc.confirm_reboot) {
                    self.emu.send(Command::PowerCycle)
                }
            }
            Action::SoftReset => self.emu.send(Command::Reset),
            Action::DriveSelect(d) => self.pick_disk(usize::from(d), false),
            Action::DriveMenu(d) => self.ui.drive_menu = Some(usize::from(d)),
            Action::SwapDrives => {
                self.remember_media(&Command::SwapDrives);
                self.emu.send(Command::SwapDrives)
            }
            Action::ToggleFullscreen => {
                if let Some(w) = self.window() {
                    let fs = if w.fullscreen().is_some() {
                        None
                    } else {
                        Some(Fullscreen::Borderless(None))
                    };
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
            Action::SaveState => self.state_dialog(true),
            Action::LoadState => self.state_dialog(false),
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

    /// F11 / F12 (`SaveStateSelectImage`): the file dialog starts at the current state file; the
    /// chosen file becomes the current one (and is remembered, like AppleWin's registry value).
    fn state_dialog(&mut self, save: bool) {
        let was_paused = self.status.paused;
        self.emu.send(Command::SetPaused(true));
        let picked = crate::state_io::pick_file(save, &default_state_path(&self.settings));
        self.emu.send(Command::SetPaused(was_paused));
        let Some(path) = picked else { return };
        if let Err(e) = crate::state_io::remember_path(&mut self.settings, &path) {
            self.ui.message = format!("Could not save settings: {e}");
        }
        self.emu.send(Command::SetStatePath(path.clone()));
        self.emu.send(if save {
            Command::SaveState(path)
        } else {
            Command::LoadState(path)
        });
    }

    /// AppleWin saves the image of a floppy drive as soon as it changes (`SaveLastDiskImage`), so
    /// a restart from the settings dialog and the next run keep the disks (GH#98, GH#511, GH#960).
    fn remember_media(&mut self, c: &Command) {
        if crate::hotswap::remember_media(&mut self.settings, c) {
            if let Err(e) = self.settings.save_default() {
                self.ui.message = format!("Could not save settings: {e}");
            }
        }
    }

    /// `read_only`: "Open as Read Only" (the disk is mounted write-protected).
    fn pick_disk(&mut self, drive: usize, read_only: bool) {
        // Pause while the modal dialog is open so sound does not stutter and time does not pass.
        let was_paused = self.status.paused;
        self.emu.send(Command::SetPaused(true));
        let mut dialog = rfd::FileDialog::new()
            .set_title(format!(
                "Select disk image for drive {}{}",
                drive + 1,
                if read_only { " (read only)" } else { "" }
            ))
            .add_filter(
                "Disk images",
                &[
                    "dsk", "do", "po", "nib", "woz", "2mg", "2img", "hdv", "gz", "zip", "iie",
                ],
            );
        if let Some(dir) = &self.settings.storage.disk_dir {
            dialog = dialog.set_directory(dir);
        }
        let picked = dialog.pick_file();
        self.emu.send(Command::SetPaused(was_paused));
        if let Some(path) = picked {
            self.ui.disk.note_inserted(&path);
            let c = Command::InsertDisk {
                drive,
                path,
                read_only,
            };
            self.remember_media(&c);
            self.emu.send(c);
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
            self.emu.send(Command::Paste(
                text.replace("\r\n", "\r").replace('\n', "\r"),
            ));
        }
    }

    /// Ctrl+0: custom speed <-> full speed.
    fn toggle_max_speed(&mut self) {
        let custom = match emu::speed_from_settings(&self.settings) {
            Speed::Max => Speed::NORMAL,
            s => s,
        };
        self.emu.send(Command::SetSpeed(if self.status.full_speed {
            custom
        } else {
            Speed::Max
        }));
    }

    // ---- input ----

    fn now_us(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }

    /// Sends the machine whatever changed in the joystick / pad-card state (also advances autofire).
    fn update_joy(&mut self) {
        let now = self.now_us();
        for c in self.input.update(now) {
            self.emu.send(c);
        }
    }

    fn poll_gamepads(&mut self) {
        use gilrs::EventType;
        if let Some(g) = self.gilrs.as_mut() {
            while let Some(ev) = g.next_event() {
                if let EventType::Disconnected = ev.event {
                    if let Some(slot) = self.pad_ids.iter().position(|i| *i == ev.id) {
                        self.pad_ids.remove(slot);
                        if slot == 0 {
                            self.input.joy.pads[0] = self.input.joy.pads[1];
                        }
                        self.input.joy.pads[1] = Default::default();
                        self.input.joy.set_pads_connected(self.pad_ids.len());
                    }
                    continue;
                }
                if !self.pad_ids.contains(&ev.id) && self.pad_ids.len() < 2 {
                    self.pad_ids.push(ev.id);
                    self.input.joy.set_pads_connected(self.pad_ids.len());
                }
                let Some(slot) = self.pad_ids.iter().position(|i| *i == ev.id) else {
                    continue;
                };
                let p = &mut self.input.joy.pads[slot];
                match ev.event {
                    EventType::AxisChanged(axis, v, _) => p.axis(axis, v),
                    EventType::ButtonPressed(b, _) => p.button(b, true),
                    EventType::ButtonReleased(b, _) => p.button(b, false),
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

    /// The mouse wheel over the debugger is Up/Down with the current modifiers (WinFrame.cpp:1647).
    fn debugger_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        let dy = match delta {
            winit::event::MouseScrollDelta::LineDelta(_, y) => y,
            winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
        };
        if let Some(k) = crate::keymap::wheel_debug_key(dy, self.mods()) {
            self.emu.send(Command::Debug(k));
        }
    }

    fn keyboard(&mut self, ev: &winit::event::KeyEvent, ui_wants_keys: bool) {
        let pressed = ev.state == ElementState::Pressed;
        let input = host::key_input(ev, self.mods);
        let mods = self.mods();
        let alt_enter = self.settings.video.alt_enter_toggles_fullscreen;
        let action = action_for_mode_with(input.key, mods, self.status.debugger_active, alt_enter);
        if pressed && !ev.repeat {
            if let Some(a) = action {
                self.do_action(a);
                return;
            }
        } else if pressed && action.is_some() {
            return; // repeats of bound keys
        }
        let ctx = KeyCtx {
            mods,
            debugger_active: self.status.debugger_active,
            ui_wants_keys,
        };
        let now = self.now_us();
        for r in self.input.key(input, pressed, ev.repeat, ctx, now) {
            match r {
                Routed::Cmd(c) => self.emu.send(c),
                Routed::Paste => self.paste(),
                Routed::ScrollLock { pressed, repeat } => self.scroll_lock(pressed, repeat),
            }
        }
    }

    /// "Benchmark Emulator": runs on its own machine in a thread and reports when done.
    fn start_benchmark(&mut self) {
        if self.bench_rx.is_some() {
            return;
        }
        let settings = self.settings.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.bench_rx = Some(rx);
        self.ui.benchmark_running = true;
        std::thread::spawn(move || {
            let mut backend = crate::backend::DefaultBackend;
            let r =
                crate::benchmark::run(&settings, &mut backend, std::time::Duration::from_secs(1))
                    .map(|r| r.report());
            let _ = tx.send(r);
        });
    }

    fn poll_benchmark(&mut self) {
        let Some(rx) = &self.bench_rx else { return };
        if let Ok(r) = rx.try_recv() {
            self.bench_rx = None;
            self.ui.benchmark_running = false;
            self.ui.benchmark_report = Some(r.unwrap_or_else(|e| format!("Benchmark failed: {e}")));
        }
    }

    /// A file dropped on the window (WinFrame.cpp `WM_DROPFILES`): a state file loads; a disk
    /// goes to drive 2 if dropped on the drive 2 button, else to drive 1, and the machine
    /// reboots unless it was dropped on the drive 1 button.
    fn dropped_file(&mut self, path: PathBuf) {
        if crate::state_io::is_state_file(&path) {
            self.emu.send(open_command(&path));
            return;
        }
        let t = toolbar::drop_target(self.cursor, self.ui.drive_rects);
        self.ui.disk.note_inserted(&path);
        let c = toolbar::drop_command(&t, path);
        self.remember_media(&c);
        self.emu.send(c);
    }

    /// Lets go of every held key and key-driven joystick (F7, full screen, focus lost).
    fn release_keys(&mut self) {
        let now = self.now_us();
        for c in self.input.release_all(now) {
            self.emu.send(c);
        }
    }

    fn release_inputs(&mut self) {
        self.release_keys();
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
                let _ = w
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined));
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
            let _ = w.set_cursor_grab(if on {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            });
            w.set_cursor_visible(!on);
        }
        if !on {
            self.input.joy.set_mouse_button(0, false);
            self.input.joy.set_mouse_button(1, false);
            self.update_joy();
        }
    }

    /// Pointer position inside the Apple screen as paddle values.
    fn mouse_joy_position(&mut self) {
        if let (Some((cx, cy)), Some(r)) = (self.cursor, self.ui.screen_rect) {
            self.input.joy.set_mouse_position(
                mouse_to_paddle(cx - r.min.x, r.width()),
                mouse_to_paddle(cy - r.min.y, r.height()),
            );
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
                self.emu.send(Command::MouseButton {
                    button: idx,
                    down: pressed,
                });
            }
        } else if self.joy_cursor {
            if pressed && release {
                self.set_joy_cursor(false);
            } else {
                self.input.joy.set_mouse_button(usize::from(idx), pressed);
                self.update_joy();
            }
        } else if pressed && over_screen && !self.mods.control_key() {
            // The AppleMouse card takes the mouse before the joystick does.
            if self.status.mouse_available {
                self.set_mouse_capture(true);
            } else if self.input.joy.using_mouse() {
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
                    self.input
                        .joy
                        .set_config(JoyConfig::from_settings(&self.settings));
                    self.input.resend(); // the machine is rebuilt: it knows no input
                    if !self.input.joy.using_mouse() {
                        self.set_joy_cursor(false);
                    }
                    self.update_joy();
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::ApplySettings(s));
                }
                UiEvent::ApplyLive(s) => {
                    // image changes of the Disk II / hard disk cards are applied to the running machine
                    for c in crate::hotswap::commands(&self.settings, &s) {
                        self.emu.send(c);
                    }
                    self.settings = (*s).clone();
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::UpdateSettings(s));
                }
                UiEvent::Volume(v) => self.emu.send(Command::SetVolume(v)),
                UiEvent::EjectDisk(d) => {
                    let c = Command::EjectDisk { drive: d };
                    self.remember_media(&c);
                    self.emu.send(c);
                }
                UiEvent::InsertDisk { drive, path } => {
                    self.ui.disk.note_inserted(&path);
                    let c = Command::InsertDisk {
                        drive,
                        path,
                        read_only: false,
                    };
                    self.remember_media(&c);
                    self.emu.send(c);
                }
                UiEvent::PickDiskReadOnly(drive) => self.pick_disk(drive, true),
                UiEvent::SetWriteProtect { drive, on } => {
                    self.emu.send(Command::SetWriteProtect { drive, on })
                }
                UiEvent::Paste => self.paste(),
                UiEvent::RebootConfirmed => self.emu.send(Command::PowerCycle),
                UiEvent::Benchmark => self.start_benchmark(),
            }
        }
    }

    fn redraw(&mut self) {
        self.poll_benchmark();
        let (Some(window), Some(gfx), Some(state)) = (
            self.window.clone(),
            self.gfx.as_mut(),
            self.egui_state.as_mut(),
        ) else {
            return;
        };
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
            usable: self.status.mouse_available || self.input.joy.using_mouse(),
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
        let repaint = output
            .viewport_output
            .values()
            .any(|v| v.repaint_delay.is_zero());
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
            self.quit = true;
            return;
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
    if crate::state_io::is_state_file(p) {
        Command::LoadState(p.to_path_buf())
    } else {
        Command::InsertDisk {
            drive: 0,
            path: p.to_path_buf(),
            read_only: false,
        }
    }
}

fn window_size(scale: u32, toolbar: bool) -> LogicalSize<f64> {
    let s = f64::from(scale.clamp(1, 4));
    LogicalSize::new(
        560.0 * s + if toolbar { 92.0 + 17.0 } else { 0.0 },
        384.0 * s + if toolbar { 34.0 } else { 0.0 },
    )
}

impl App {
    /// Releases GPU and window resources in dependency order: Gfx (renderer, surface, device, queue,
    /// instance), then the egui winit state, then the window. Must run while the event loop (and so
    /// the display connection) is still alive: terminating EGL on Wayland afterwards segfaults.
    fn release_gpu(&mut self) {
        self.gfx = None;
        self.egui_state = None;
        self.window = None;
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.release_gpu();
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("AppleII-Rust")
            .with_window_icon(crate::icon::window_icon())
            .with_inner_size(window_size(self.settings.video.window_scale, true));
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
                self.release_gpu();
                el.exit();
                return;
            }
        }
        self.egui_state = Some(egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        ));
        if self.settings.video.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        window.set_ime_allowed(false);
        self.window = Some(window);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, e: UserEvent) {
        if matches!(e, UserEvent::Exit) {
            el.exit();
            return;
        }
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
            if r.consumed
                && !matches!(
                    event,
                    WindowEvent::KeyboardInput { .. }
                        | WindowEvent::RedrawRequested
                        | WindowEvent::CloseRequested
                )
            {
                // pointer events over egui widgets do not reach the Apple
                if !matches!(
                    event,
                    WindowEvent::CursorMoved { .. }
                        | WindowEvent::ModifiersChanged(_)
                        | WindowEvent::Focused(_)
                ) {
                    return;
                }
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                self.emu.shutdown();
                self.release_gpu();
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(g) = self.gfx.as_mut() {
                    g.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.quit {
                    self.release_gpu();
                    el.exit();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                self.input.joy.set_shift(self.mods.shift_key());
                self.update_joy();
            }
            WindowEvent::Focused(false) => self.release_inputs(),
            WindowEvent::KeyboardInput { event, .. } => self.keyboard(&event, egui_wants_keys),
            WindowEvent::MouseWheel { delta, .. } if self.status.debugger_active => {
                self.debugger_wheel(delta)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((
                    position.x as f32 / self.egui_ctx.pixels_per_point(),
                    position.y as f32 / self.egui_ctx.pixels_per_point(),
                ));
                if self.joy_cursor {
                    self.mouse_joy_position();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_button(button, state == ElementState::Pressed)
            }
            WindowEvent::DroppedFile(path) => self.dropped_file(path),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta }, true) = (event, self.mouse_captured) {
            self.emu.send(Command::MouseMove {
                dx: delta.0.round() as i32,
                dy: delta.1.round() as i32,
            });
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        // gamepads are polled with every published frame (user_event); nothing else to do
        if self.gilrs.is_some() {
            self.poll_gamepads();
        }
    }
}
