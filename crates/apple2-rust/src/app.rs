//! The winit application: window, input routing, actions, audio and gamepad glue.

use crate::actions::{action_for_mode_with, Action};
use crate::audio::AudioOut;
use crate::backend::BackendFactory;
use crate::emu::{self, default_state_path, Command, EmuHandle, EmuStatus};
use crate::gfx::{Gfx, GpuShared};
use crate::host;
use crate::input::{InputRouter, KeyCtx, Routed};
use crate::joy::{mouse_to_paddle, JoyConfig};
use crate::keymap::Mods;
use crate::overlay::MouseState;
use crate::pacing::Speed;
use crate::printer_hub::{self, PrinterConfig, PrinterHub};
use crate::printer_model::{CenterEvent, PrinterViewModel};
use crate::printer_window::{self, PrinterWindow, Route};
use crate::screenshot::{save_png, ScreenshotNamer};
use crate::toolbar;
use crate::ui::{UiEvent, UiState};
use crate::video::Frame;
use apple2_config::Settings;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// The virtual printer produced something the UI should look at (first byte, job end).
    Printer,
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
    /// Startup warning shown like the ROM/slot ones (e.g. the config directory migration).
    pub startup_notice: Option<String>,
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
    /// GPU context shared with the Printer Center window.
    gpu: Option<Arc<GpuShared>>,
    /// Second native window (Printer Center); never receives emulator input.
    printer: PrinterWindow,
    /// What the Central draws: a copy of the hub's view model (or the demo data).
    printer_vm: PrinterViewModel,
    /// Spool, bridge and view model of the virtual printer; started once a Generic Printer that
    /// feeds it exists. Declared after `emu` so the emulator stops first on drop.
    hub: Option<PrinterHub>,
    proxy: EventLoopProxy<UserEvent>,
    /// The main window has keyboard focus (decides whether an automatic opening gives it back).
    main_focused: bool,
    /// `APPLE2_RUST_PRINTER_DEMO=script`: scripted close / reopen smoke (stage, start, frames).
    demo_script: Option<(u8, Instant, u64)>,
    quit: bool,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    ui: UiState,
    help: crate::help::HelpViewer,
    frame: Frame,
    frame_seq: u64,
    status: EmuStatus,
    last_title: String,
    /// Screenshot file names (AppleWin names them after the last floppy inserted).
    shots: ScreenshotNamer,
    drive_images: [Option<String>; 2],
    last_message_seq: u64,
    mods: ModifiersState,
    input: InputRouter,
    started: std::time::Instant,
    gilrs: Option<gilrs::Gilrs>,
    pad_ids: Vec<gilrs::GamepadId>,
    clipboard: Option<arboard::Clipboard>,
    cursor: Option<(f32, f32)>,
    /// Last pointer movement and whether the idle full-screen pointer is hidden (GH#504).
    last_cursor_move: Instant,
    cursor_hidden: bool,
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
            startup_notice,
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
        let frame_proxy = proxy.clone();
        let emu = emu::spawn(
            settings.clone(),
            factory,
            ring,
            rate,
            Box::new(move || drop(frame_proxy.send_event(UserEvent::Frame))),
        )?;
        if let Some(n) = startup_notice {
            emu.send(Command::Notice(n));
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
        let mut app = App {
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
            gpu: None,
            printer: PrinterWindow::default(),
            printer_vm: PrinterViewModel::default(),
            hub: None,
            proxy,
            main_focused: false,
            demo_script: None,
            quit: false,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            frame: Frame::new(0, 0),
            frame_seq: 0,
            status: EmuStatus::default(),
            last_title: String::new(),
            shots: ScreenshotNamer::default(),
            drive_images: [None, None],
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
            last_cursor_move: Instant::now(),
            cursor_hidden: false,
            mouse_captured: false,
            joy_cursor: false,
            last_copied_seq: 0,
            scale_toggle: false,
        };
        app.sync_printer();
        Ok(app)
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
            Action::ToggleScale if self.window().is_some_and(|w| w.fullscreen().is_some()) => {
                // ScreenWindowResize (WinFrame.cpp:1976): in full screen Ctrl+F6 also goes back.
                self.do_action(Action::ToggleFullscreen);
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
            Action::PrinterCenter => self.printer.open(false),
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
        let path = self.shots.next_path(&dir);
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
        if crate::input::scroll_lock_flips_full_speed(
            pressed,
            repeat,
            self.settings.keyboard.scroll_lock_toggle,
        ) {
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
                    self.input
                        .kbd
                        .set_altgr_sends_char(self.settings.keyboard.altgr_sends_char);
                    self.input
                        .kbd
                        .set_hook_altgr_control(self.settings.keyboard.hook_altgr_control);
                    self.input.resend(); // the machine is rebuilt: it knows no input
                    if !self.input.joy.using_mouse() {
                        self.set_joy_cursor(false);
                    }
                    self.update_joy();
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    self.emu.send(Command::ApplySettings(s));
                    self.sync_printer();
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
                    self.sync_printer();
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
        let eligible = window.fullscreen().is_some()
            && !self.status.mouse_available
            && !self.mouse_captured
            && !self.joy_cursor;
        let hide = cursor_idle_hides(self.last_cursor_move.elapsed(), eligible);
        if hide != self.cursor_hidden {
            self.cursor_hidden = hide;
            if hide || !(self.mouse_captured || self.joy_cursor) {
                window.set_cursor_visible(!hide);
            }
        }
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
            for d in 0..2 {
                if self.status.drives[d].image != self.drive_images[d] {
                    self.drive_images[d] = self.status.drives[d].image.clone();
                    self.shots.reset(self.drive_images[d].as_deref());
                }
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

/// AppleWin asks "Are you sure you want to reboot?" when the reboot comes from the F2 button or
/// the F2 key (both call `ProcessButtonClick(.., true)`, WinFrame.cpp:1440 and :1534) and
/// "Confirm reboot" is on (`ConfirmReboot(bFromButtonUI)`, WinFrame.cpp:1999). Internal restarts
/// (`ProcessButtonClick(BTN_RUN)`, :1189 and :1955) never ask. The question itself is the
/// egui box opened by `Ui::request_reboot`.
pub fn reboot_needs_confirmation(action: Action, from_button: bool, confirm_reboot: bool) -> bool {
    matches!(action, Action::Reboot) && from_button && confirm_reboot
}

/// Largest scale (at most `wanted`, at least 1) whose window fits a screen of `avail` logical
/// pixels. AppleWin reverts 2x to 1x when the screen is too small (WinFrame.cpp:3058, GH#161);
/// here the screen is the one the window is on (GH#932 only looked at the primary display).
pub fn fit_scale(wanted: u32, avail: (f64, f64), toolbar: bool) -> u32 {
    (1..=wanted.clamp(1, 4))
        .rev()
        .find(|&s| {
            let w = window_size(s, toolbar);
            w.width <= avail.0 && w.height <= avail.1
        })
        .unwrap_or(1)
}

/// GH#504 (WinFrame.cpp:1627-1640): in full screen, with no mouse card and the mouse not driving
/// a joystick, the pointer hides after 2 s without movement and shows again when it moves.
pub fn cursor_idle_hides(idle: Duration, eligible: bool) -> bool {
    eligible && idle > Duration::from_millis(2000)
}

pub fn window_size(scale: u32, toolbar: bool) -> LogicalSize<f64> {
    let s = f64::from(scale.clamp(1, 4));
    LogicalSize::new(
        560.0 * s + if toolbar { 92.0 + 17.0 } else { 0.0 },
        384.0 * s + if toolbar { 34.0 } else { 0.0 },
    )
}

/// A file name for a job title: letters, digits, space, dash and underscore only.
pub fn file_stem_for(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "impressao".into()
    } else {
        s
    }
}

impl App {
    /// Starts the virtual printer pipeline when a Generic Printer feeds it, and tells it about the
    /// current settings (the toolbar button exists only while one does).
    fn sync_printer(&mut self) {
        let config = PrinterConfig::from_settings(&self.settings);
        if self.hub.is_none() && config.is_some() {
            let proxy = std::sync::Mutex::new(self.proxy.clone());
            let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                if let Ok(p) = proxy.lock() {
                    let _ = p.send_event(UserEvent::Printer);
                }
            });
            let root = printer_hub::archive_root()
                .unwrap_or_else(|| std::env::temp_dir().join("apple2-rust-impressoes"));
            let events = self.emu.printer_events();
            let started = PrinterHub::start(root, events.clone(), config.clone(), wake.clone())
                .or_else(|e| {
                    let fallback = std::env::temp_dir().join("apple2-rust-impressoes");
                    self.ui.message = format!(
                        "Arquivo de impressões indisponível ({e}); usando {}",
                        fallback.display()
                    );
                    PrinterHub::start(fallback, events, config.clone(), wake)
                });
            match started {
                Ok(h) => self.hub = Some(h),
                Err(e) => self.ui.message = format!("Impressora virtual indisponível: {e}"),
            }
        } else if let Some(h) = self.hub.as_mut() {
            h.configure(config.clone());
        }
        if std::env::var_os(printer_window::DEMO_ENV).is_none() {
            self.ui.printer_available = config.is_some();
        }
        self.poll_printer();
    }

    /// Reads the spool's state: view model, automatic opening, activity light.
    fn poll_printer(&mut self) {
        let Some(hub) = self.hub.as_mut() else {
            return;
        };
        let p = hub.poll(Instant::now());
        if p.activity {
            self.ui.set_printer_activity(true);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        if p.auto_open {
            self.printer.open(true);
        }
        if p.changed {
            self.printer_vm = hub.view_model().clone();
            self.printer.invalidate();
        }
    }

    /// Requests the Central raised (buttons, forms). Dialogs for file names live here because
    /// they need the main thread.
    fn handle_printer_events(&mut self) {
        for ev in self.printer.take_events() {
            match ev {
                CenterEvent::ExportPdf(req) => {
                    let picked = rfd::FileDialog::new()
                        .set_title("Salvar PDF")
                        .set_file_name(format!("{}.pdf", file_stem_for(&req.title)))
                        .add_filter("PDF", &["pdf"])
                        .save_file();
                    if let (Some(path), Some(hub)) = (picked, self.hub.as_mut()) {
                        let path = if path.extension().is_none() {
                            path.with_extension("pdf")
                        } else {
                            path
                        };
                        hub.export_pdf(req, path);
                    }
                }
                CenterEvent::SaveRaw { job } => {
                    let picked = rfd::FileDialog::new()
                        .set_title("Salvar bytes brutos")
                        .set_file_name(format!("{job}.prn"))
                        .save_file();
                    if let (Some(path), Some(hub)) = (picked, self.hub.as_mut()) {
                        hub.save_raw(&job, path);
                    }
                }
                CenterEvent::SetNextSetup(s) => {
                    for c in self.settings.slots.0.iter_mut() {
                        if let apple2_config::SlotCardSpec::GenericPrinter(p) = c {
                            if p.destination.to_virtual() {
                                printer_hub::store_setup(p, &s);
                                break;
                            }
                        }
                    }
                    if let Err(e) = self.settings.save_default() {
                        self.ui.message = format!("Could not save settings: {e}");
                    }
                    if let Some(hub) = self.hub.as_mut() {
                        hub.handle(CenterEvent::SetNextSetup(s));
                    }
                }
                other => {
                    if let Some(hub) = self.hub.as_mut() {
                        hub.handle(other);
                    }
                }
            }
        }
        self.poll_printer();
    }

    /// Creates / focuses the Printer Center when asked, and schedules its (rate-limited) redraws.
    fn drive_printer(&mut self, el: &ActiveEventLoop) {
        self.poll_printer();
        if self.printer.is_open() {
            if let Some(shared) = self.gpu.clone() {
                match self
                    .printer
                    .realize(el, &shared, crate::icon::window_icon())
                {
                    Ok(true) => {
                        // winit cannot open it unfocused here: give the focus back
                        if self.main_focused {
                            if let Some(w) = &self.window {
                                w.focus_window();
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!("cannot open the Printer Center: {e}");
                        self.printer.close();
                    }
                }
            }
        }
        let now = Instant::now();
        let hub_wake = self.hub.as_ref().and_then(|h| h.next_wake(now));
        let wake = match (self.printer.drive(now), hub_wake) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        match wake {
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }

    /// Smoke script: close the Central at 3 s, check the emulation kept running, reopen at 5 s,
    /// close the main window (clean exit) at 7 s.
    fn run_demo_script(&mut self, el: &ActiveEventLoop) {
        let Some((stage, t0, frames)) = self.demo_script else {
            return;
        };
        let secs = t0.elapsed().as_secs_f32();
        let seq = self.emu.shared.frame_seq();
        match stage {
            0 if secs > 3.0 => {
                self.printer.close();
                eprintln!("script: Central closed at frame {seq}, main window alive");
                self.demo_script = Some((1, t0, seq));
            }
            1 if secs > 5.0 => {
                eprintln!(
                    "script: emulation advanced {} frames while closed",
                    seq - frames
                );
                self.printer.open(false);
                self.demo_script = Some((2, t0, seq));
            }
            2 if secs > 7.0 => {
                eprintln!(
                    "script: Central reopened (page {} kept), closing the main window",
                    self.printer.preview.page + 1
                );
                self.emu.shutdown();
                self.release_gpu();
                el.exit();
                self.demo_script = None;
            }
            _ => {}
        }
        el.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
        ));
    }

    /// Development flag `APPLE2_RUST_PRINTER_DEMO` (see [`printer_window::DEMO_ENV`]).
    fn start_printer_demo(&mut self) {
        let Ok(v) = std::env::var(printer_window::DEMO_ENV) else {
            return;
        };
        self.ui.printer_available = true;
        self.ui.set_printer_activity(true);
        self.printer_vm = printer_window::demo_view_model(100);
        if v == "script" {
            self.demo_script = Some((0, Instant::now(), 0));
        }
        self.printer.open(v == "auto");
    }

    /// Releases GPU and window resources in dependency order: Gfx (renderer, surface, device, queue,
    /// instance), then the egui winit state, then the window. Must run while the event loop (and so
    /// the display connection) is still alive: terminating EGL on Wayland afterwards segfaults.
    fn release_gpu(&mut self) {
        // The secondary window goes first: its surface was created from the shared instance.
        self.printer.shutdown();
        self.gfx = None;
        self.gpu = None;
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
            Ok(g) => {
                self.gpu = Some(g.shared());
                self.gfx = Some(g);
            }
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
        if let Some(m) = window.current_monitor() {
            let sz = m.size().to_logical::<f64>(m.scale_factor());
            let wanted = self.settings.video.window_scale;
            let fit = fit_scale(wanted, (sz.width, sz.height), true);
            if fit != wanted.clamp(1, 4) {
                let _ = window.request_inner_size(window_size(fit, true));
            }
        }
        self.window = Some(window);
        self.main_focused = true;
        self.printer.dump = std::env::var_os("APPLE2_RUST_PRINTER_DUMP").map(PathBuf::from);
        self.start_printer_demo();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, e: UserEvent) {
        if matches!(e, UserEvent::Exit) {
            el.exit();
            return;
        }
        if matches!(e, UserEvent::Printer) {
            self.drive_printer(el);
            return;
        }
        self.poll_gamepads();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match printer_window::route(
            id,
            self.window.as_ref().map(|w| w.id()),
            self.printer.window_id(),
        ) {
            Route::Main => {}
            Route::Printer => {
                // Everything typed or clicked in the Printer Center stays there (R08).
                self.printer.window_event(event, &self.printer_vm);
                self.handle_printer_events();
                return;
            }
            Route::Unknown => return,
        }
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
                // The card's last bytes are queued: the spool takes them, closes the job as
                // Shutdown and stops, in that order.
                self.hub = None;
                self.printer.shutdown();
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
            WindowEvent::Focused(false) => {
                self.main_focused = false;
                self.release_inputs();
            }
            WindowEvent::Focused(true) => self.main_focused = true,
            WindowEvent::KeyboardInput { event, .. } => self.keyboard(&event, egui_wants_keys),
            WindowEvent::MouseWheel { delta, .. } if self.status.debugger_active => {
                self.debugger_wheel(delta)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_move = Instant::now();
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

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // gamepads are polled with every published frame (user_event)
        if self.gilrs.is_some() {
            self.poll_gamepads();
        }
        self.drive_printer(el);
        self.run_demo_script(el);
    }
}
