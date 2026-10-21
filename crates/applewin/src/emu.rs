//! The emulation side of the shell (ADR 0003): [`Emulator`] is the synchronous, deterministic
//! unit (machine + mixer + video source + card hooks); [`spawn`] wraps it in a thread that paces
//! it against the audio ring or a timer and talks to the UI through [`Shared`] and [`Command`]s.

use crate::audio::AudioRing;
use crate::backend::{Backend, BackendFactory, DiskSink, DriveStatus, MouseSink};
use crate::debug_view::render_grid;
use crate::joy::JoyOutput;
use crate::keymap::{DebugKey, KeyEvent};
use crate::pacing::{audio_frames_due, Speed, TimerPacer};
use crate::video::{Frame, FrameSource};
use apple2_audio::{volume_gain, AudioBus, Mixer};
use apple2_config::Settings;
use apple2_core::Machine;
use apple2_debugger::{Debugger, RunMode};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// AppleWin's volume scale: 0 = loudest, `VOLUME_MAX` = silent.
pub const VOLUME_MAX: u32 = 59;

/// UI -> emulation thread.
#[derive(Debug)]
pub enum Command {
    Key(KeyEvent),
    /// Joystick `index` (0 or 1) state.
    Joy {
        index: usize,
        out: JoyOutput,
    },
    JoyAttached {
        index: usize,
        attached: bool,
    },
    Paste(String),
    /// Ctrl+Print Screen: publish the text screen for the clipboard ([`EmuStatus::copied_text`]).
    CopyScreenText,
    Reset,
    PowerCycle,
    SetPaused(bool),
    TogglePause,
    SetSpeed(Speed),
    ToggleFullSpeed,
    /// One instruction (works while paused: the debugger's step).
    Step,
    SaveState(PathBuf),
    LoadState(PathBuf),
    /// The user picked another state file (F11/F12 dialog): used for "Save State On Exit".
    SetStatePath(PathBuf),
    /// Rebuilds the machine (power cycle) with new settings.
    ApplySettings(Box<Settings>),
    /// New settings that need no machine rebuild (video, speed, input...): swaps the settings and
    /// the renderer, keeps the machine running.
    UpdateSettings(Box<Settings>),
    SetWriteProtect {
        drive: usize,
        on: bool,
    },
    SetVolume(u32),
    InsertDisk {
        drive: usize,
        path: PathBuf,
    },
    EjectDisk {
        drive: usize,
    },
    SwapDrives,
    VideoMode(i32),
    ToggleScanlines,
    RockerSwitch,
    MouseMove {
        dx: i32,
        dy: i32,
    },
    MouseButton {
        button: u8,
        down: bool,
    },
    ToggleDebugger,
    /// Console input while the debugger is on screen.
    Debug(DebugKey),
    Shutdown,
}

/// Emulation thread -> UI, refreshed once per published frame.
#[derive(Clone, Debug, Default)]
pub struct EmuStatus {
    pub paused: bool,
    pub full_speed: bool,
    pub speed_label: String,
    pub caps_lock: bool,
    pub cycles: u64,
    pub frames: u64,
    /// Measured emulated clock in MHz (about 1.02 at 1.0x).
    pub mhz: f32,
    pub drives: [DriveStatus; 2],
    pub model: String,
    pub video: String,
    pub underruns: u64,
    pub debugger_active: bool,
    pub debugger_available: bool,
    pub mouse_available: bool,
    /// Last user-visible message and a counter that changes whenever it does.
    pub message: String,
    pub message_seq: u64,
    /// Text screen from the last [`Command::CopyScreenText`] and a counter that changes with it.
    pub copied_text: String,
    pub copied_seq: u64,
}

/// State shared with the UI thread.
#[derive(Default)]
pub struct Shared {
    frame: Mutex<Frame>,
    frame_seq: AtomicU64,
    status: Mutex<EmuStatus>,
}

impl Default for Frame {
    fn default() -> Self {
        Frame::new(0, 0)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    pub fn frame_seq(&self) -> u64 {
        self.frame_seq.load(Ordering::Acquire)
    }

    /// Copies the latest frame into `out` (reusing its buffer).
    pub fn copy_frame(&self, out: &mut Frame) {
        let f = lock(&self.frame);
        out.width = f.width;
        out.height = f.height;
        out.rgba.clear();
        out.rgba.extend_from_slice(&f.rgba);
    }

    pub fn status(&self) -> EmuStatus {
        lock(&self.status).clone()
    }

    fn publish(&self, frame: &Frame, status: EmuStatus) {
        {
            let mut f = lock(&self.frame);
            f.width = frame.width;
            f.height = frame.height;
            f.rgba.clear();
            f.rgba.extend_from_slice(&frame.rgba);
        }
        *lock(&self.status) = status;
        self.frame_seq.fetch_add(1, Ordering::Release);
    }
}

/// Where F11/F12 keep the state unless the user picked a file (see [`crate::state_io`]).
pub fn default_state_path(settings: &Settings) -> PathBuf {
    crate::state_io::default_state_path(settings, &std::env::current_dir().unwrap_or_default())
}

/// Settings "Emulation Speed" (tenths, 40 = unbounded) to [`Speed`].
pub fn speed_from_settings(s: &Settings) -> Speed {
    if let Some(m) = s.machine.clock_multiplier {
        return Speed::Multiplier(m.max(0.05));
    }
    match s.machine.speed {
        40.. => Speed::Max,
        n => Speed::Multiplier(f64::from(n.max(1)) / 10.0),
    }
}

/// `FsHost` that serves `-debugger-auto-run <file>` when the debugger asks for its
/// `DebuggerAutoRun.txt` (Debug.cpp:9435: only once per run, not on a restart).
struct AutoRunHost {
    fs: apple2_debugger::host::FsHost,
    script: Option<PathBuf>,
}

impl apple2_debugger::host::Host for AutoRunHost {
    fn read(&mut self, name: &str) -> std::io::Result<Vec<u8>> {
        match (&self.script, name) {
            (Some(p), "DebuggerAutoRun.txt") => {
                let r = std::fs::read(p);
                self.script = None;
                r
            }
            _ => self.fs.read(name),
        }
    }
    fn write(&mut self, name: &str, data: &[u8]) -> std::io::Result<()> {
        self.fs.write(name, data)
    }
    fn create(&mut self, name: &str) -> std::io::Result<Box<dyn std::io::Write>> {
        self.fs.create(name)
    }
    fn dir(&self) -> String {
        self.fs.dir()
    }
    fn set_dir(&mut self, dir: &str) -> bool {
        self.fs.set_dir(dir)
    }
}

fn new_debugger(m: &mut Machine, auto_run: Option<&Path>) -> Debugger {
    let mut d = Debugger::new();
    let fs = apple2_debugger::host::FsHost::new(std::env::current_dir().unwrap_or_default());
    d.set_host(Box::new(AutoRunHost {
        fs,
        script: auto_run.map(Path::to_path_buf),
    }));
    d.initialize(m);
    d
}

/// Machine + everything driven synchronously. No threads, no clocks: tests and the headless
/// mode use it directly.
pub struct Emulator {
    pub settings: Settings,
    backend: Box<dyn Backend>,
    pub machine: Machine,
    bus: AudioBus,
    mixer: Mixer,
    video: Box<dyn FrameSource>,
    disks: Box<dyn DiskSink>,
    mouse: Box<dyn MouseSink>,
    debugger: Debugger,
    audio_rate: u32,
    frame: Frame,
    pub paused: bool,
    pub speed: Speed,
    pub full_speed: bool,
    caps: bool,
    message: String,
    message_seq: u64,
    copied_text: String,
    copied_seq: u64,
    mhz: f32,
    underruns: u64,
    rom_report: Vec<apple2_config::roms::RomInfo>,
}

/// Writes the ROMs in use to the start-up log (stderr) and returns them.
/// Invalid user ROMs fell back to the embedded ones: those warnings are logged and returned too.
fn log_roms(settings: &Settings) -> (Vec<apple2_config::roms::RomInfo>, Vec<String>) {
    let r = settings.resolve_roms();
    for w in &r.warnings {
        eprintln!("warning: {w}");
    }
    for i in &r.report {
        eprintln!("rom: {i}");
    }
    (r.report, r.warnings)
}

impl Emulator {
    pub fn new(
        settings: Settings,
        mut backend: Box<dyn Backend>,
        audio_rate: u32,
    ) -> Result<Self, String> {
        let bus = AudioBus::new(audio_rate, settings.to_machine_config().video_standard);
        let mut machine = backend.build_machine(&settings, &bus)?;
        let mixer = Mixer::new(&bus);
        let video = backend.frame_source(&settings);
        let disks = backend.disks_for(&machine, &settings);
        let mouse = backend.mouse_for(&machine, &settings);
        let debugger = new_debugger(&mut machine, settings.debugger.auto_run_script.as_deref());
        let (rom_report, rom_warnings) = log_roms(&settings);
        let speed = speed_from_settings(&settings);
        let settings_caps = settings.keyboard.caps_lock; // `-capslock=off`
        machine.set_caps_lock(settings_caps);
        let mut emu = Emulator {
            settings,
            backend,
            machine,
            bus,
            mixer,
            video,
            disks,
            mouse,
            debugger,
            audio_rate,
            frame: Frame::new(0, 0),
            paused: false,
            speed: if speed == Speed::Max {
                Speed::NORMAL
            } else {
                speed
            },
            full_speed: speed == Speed::Max,
            caps: settings_caps,
            message: String::new(),
            message_seq: 0,
            copied_text: String::new(),
            copied_seq: 0,
            mhz: 0.0,
            underruns: 0,
            rom_report,
        };
        if !rom_warnings.is_empty() {
            emu.say(rom_warnings.join("; "));
        }
        Ok(emu)
    }

    /// The ROMs in use (embedded or file, size, SHA-256), for the conformance manifest.
    pub fn rom_report(&self) -> &[apple2_config::roms::RomInfo] {
        &self.rom_report
    }

    /// Full speed requested by the user (Scroll Lock) or by a spinning drive.
    pub fn effective_full_speed(&self) -> bool {
        self.full_speed
            || self.disks.wants_full_speed(&self.machine)
            || (self.debugger.is_stepping() && self.debugger.full_speed)
    }

    pub fn clock_hz(&self) -> f64 {
        self.machine.config().video_standard.cpu_clock_hz()
    }

    pub fn cycles_per_frame(&self) -> u64 {
        u64::from(self.machine.config().video_standard.cycles_per_frame())
    }

    /// Audio sample frames one machine frame produces (at 1.0x).
    pub fn audio_frames_per_machine_frame(&self) -> usize {
        (self.cycles_per_frame() as f64 / self.clock_hz() * f64::from(self.audio_rate)).round()
            as usize
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.message_seq += 1;
    }

    pub fn set_measured(&mut self, mhz: f32, underruns: u64) {
        self.mhz = mhz;
        self.underruns = underruns;
    }

    /// Runs one video frame and returns the audio it produced (interleaved stereo).
    pub fn run_frame(&mut self) -> (u64, Vec<f32>) {
        let cpf = self.cycles_per_frame();
        let cycles = match self.debugger.mode {
            RunMode::Running => self.video.run_frame(&mut self.machine),
            RunMode::Stepping => self.debugger.run(&mut self.machine, cpf),
            RunMode::Debug => 0,
        };
        let audio = self.mixer.produce_machine(&mut self.machine);
        (cycles, audio)
    }

    /// True while the debugger owns the machine (stopped at the prompt).
    pub fn debugger_halted(&self) -> bool {
        self.debugger.mode == RunMode::Debug
    }

    pub fn render(&mut self) -> &Frame {
        if self.debugger.mode != RunMode::Running && self.debugger.output_view.is_none() {
            let grid = self.debugger.render(&self.machine);
            let blink = (self.machine.frames() / 16).is_multiple_of(2)
                || self.debugger.mode == RunMode::Debug;
            render_grid(&grid, blink, &mut self.frame);
        } else {
            if self.debugger.mode != RunMode::Running {
                self.video.redraw(&self.machine);
            }
            self.video.render(&self.machine, &mut self.frame);
        }
        &self.frame
    }

    /// `Video_RedrawAndTakeScreenShot`: repaints the whole screen from the machine's current
    /// state (no frames are run) and returns it. Used by `-screenshot-and-exit`.
    pub fn redraw_now(&mut self) -> &Frame {
        self.video.redraw(&self.machine);
        self.render()
    }

    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    pub fn status(&self) -> EmuStatus {
        EmuStatus {
            paused: self.paused,
            full_speed: self.full_speed,
            speed_label: if self.full_speed {
                "max".into()
            } else if let Speed::Multiplier(m) = self.speed {
                format!("{m:.1}x")
            } else {
                "max".into()
            },
            caps_lock: self.caps,
            cycles: self.machine.cycles(),
            frames: self.machine.frames(),
            mhz: self.mhz,
            drives: self.disks.status(&self.machine),
            model: format!("{:?}", self.settings.machine.model),
            video: self.video.description(),
            underruns: self.underruns,
            debugger_active: self.debugger.mode != RunMode::Running,
            debugger_available: true,
            mouse_available: self.mouse.available(),
            message: self.message.clone(),
            message_seq: self.message_seq,
            copied_text: self.copied_text.clone(),
            copied_seq: self.copied_seq,
        }
    }

    fn rebuild(&mut self, settings: Settings) {
        // The new machine reopens the image files: the old one must write its pending track first
        // (AUD-C-030; AppleWin's EjectDisk -> FlushCurrentTrack).
        self.disks.flush(&mut self.machine);
        // The cards hold the bus they were built with, so build everything again with one bus.
        let bus = AudioBus::new(self.audio_rate, settings.to_machine_config().video_standard);
        match self.backend.build_machine(&settings, &bus) {
            Ok(m) => {
                self.machine = m;
                self.machine.set_caps_lock(self.caps);
                self.mixer = Mixer::new(&bus);
                self.bus = bus;
                self.video = self.backend.frame_source(&settings);
                self.disks = self.backend.disks_for(&self.machine, &settings);
                self.mouse = self.backend.mouse_for(&self.machine, &settings);
                self.debugger = new_debugger(&mut self.machine, None);
                let sp = speed_from_settings(&settings);
                self.full_speed = sp == Speed::Max;
                if let Speed::Multiplier(_) = sp {
                    self.speed = sp;
                }
                let (report, warnings) = log_roms(&settings);
                self.rom_report = report;
                self.settings = settings;
                if warnings.is_empty() {
                    self.say("Settings applied");
                } else {
                    self.say(warnings.join("; "));
                }
            }
            Err(e) => self.say(format!("Cannot apply settings: {e}")),
        }
    }

    pub fn save_state(&mut self, path: &Path) {
        self.disks.flush(&mut self.machine);
        let r = crate::state_io::save_machine(&self.machine).and_then(|saved| {
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            apple2_savestate::write_atomic(path, saved.text.as_bytes())
                .map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(saved.warnings)
        });
        match r {
            Ok(w) if w.is_empty() => self.say(format!("State saved: {}", path.display())),
            Ok(w) => self.say(format!(
                "State saved: {} ({})",
                path.display(),
                w.join("; ")
            )),
            Err(e) => self.say(format!("Save state failed: {e}")),
        }
    }

    pub fn load_state(&mut self, path: &Path) {
        match self.try_load_state(path) {
            Ok(w) if w.is_empty() => self.say(format!("State loaded: {}", path.display())),
            Ok(w) => self.say(format!(
                "State loaded: {} ({})",
                path.display(),
                w.join("; ")
            )),
            Err(e) => self.say(format!("Load state failed: {e}")),
        }
    }

    /// Replaces the machine by the one in the `.aws.yaml` at `path`. Returns the loader's warnings.
    fn try_load_state(&mut self, path: &Path) -> Result<Vec<String>, String> {
        crate::state_io::check_loadable(path)?;
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        // The loaded machine reopens the images the old one may still have a dirty track for
        // (AUD-C-031; AppleWin flushes in LoadSnapshot through EjectDisk).
        self.disks.flush(&mut self.machine);
        // The cards the state builds hold their own audio bus; the mixer must read the same one.
        let bus = AudioBus::new(
            self.audio_rate,
            self.settings.to_machine_config().video_standard,
        );
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .map(Path::to_path_buf);
        let loaded = crate::state_io::load_machine(&text, &self.settings, &bus, dir.as_deref())?;
        let mut machine = loaded.machine;
        let cfg = machine.config().clone();
        bus.set_clock(apple2_audio::SampleClock::new(
            self.audio_rate,
            cfg.video_standard,
        ));
        self.settings.machine.model = cfg.model;
        self.settings.machine.refresh_rate = match cfg.video_standard {
            apple2_core::VideoStandard::Pal => apple2_config::settings::RefreshRate::Hz50,
            apple2_core::VideoStandard::Ntsc => apple2_config::settings::RefreshRate::Hz60,
        };
        self.debugger = new_debugger(&mut machine, None);
        self.disks = self.backend.disks_for(&machine, &self.settings);
        self.mouse = self.backend.mouse_for(&machine, &self.settings);
        self.mixer = Mixer::new(&bus);
        self.bus = bus;
        self.machine = machine;
        self.video.reset();
        Ok(loaded.warnings)
    }

    /// `Snapshot_Startup`: with "Save State On Exit" the previous session's state comes back.
    pub fn startup_state(&mut self) {
        if self.settings.storage.save_state_on_exit {
            let p = default_state_path(&self.settings);
            if p.is_file() {
                self.load_state(&p);
            }
        }
    }

    /// `Snapshot_Shutdown`: with "Save State On Exit" the state is saved when quitting.
    pub fn shutdown_state(&mut self) {
        if self.settings.storage.save_state_on_exit {
            let p = default_state_path(&self.settings);
            self.save_state(&p);
        }
    }

    /// Applies a UI command. Returns false on [`Command::Shutdown`].
    pub fn apply(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Key(k) => match k {
                KeyEvent::Ascii(c) => {
                    self.machine.key_press(c);
                }
                KeyEvent::Nav(n) => {
                    self.machine.key_press_nav(n);
                }
                KeyEvent::OpenApple(d) => self.machine.set_open_apple(d),
                KeyEvent::ClosedApple(d) => self.machine.set_closed_apple(d),
                KeyEvent::AnyKeyDown(d) => self.machine.set_any_key_down(d),
                KeyEvent::CapsLock(on) => {
                    self.caps = on;
                    self.machine.set_caps_lock(on);
                }
                KeyEvent::ScrollLock(on) => self.machine.set_tk3000_mode(on),
                KeyEvent::Reset => self.soft_reset(),
                KeyEvent::Paste => {}
            },
            Command::Joy { index, out } => {
                let i = index.min(1);
                self.machine.set_paddle((i * 2) as u8, out.x);
                self.machine.set_paddle((i * 2 + 1) as u8, out.y);
                if i == 0 {
                    self.machine.set_button(0, out.buttons[0]);
                    self.machine.set_button(1, out.buttons[1]);
                } else {
                    self.machine.set_button(2, out.buttons[0]);
                }
            }
            Command::JoyAttached { index, attached } => {
                self.machine.set_joystick_attached(index.min(1), attached)
            }
            Command::CopyScreenText => {
                self.copied_text =
                    apple2_core::text_screen::text_screen(&self.machine).join("\n") + "\n";
                self.copied_seq += 1;
                self.say("Text screen copied to the clipboard");
            }
            Command::Paste(t) => {
                self.machine.paste(&t);
            }
            Command::Reset => self.soft_reset(),
            Command::PowerCycle => {
                self.machine.power_cycle();
                self.mixer.reset();
                self.video.reset();
                self.say("Machine rebooted");
            }
            Command::SetPaused(p) => self.paused = p,
            Command::TogglePause => self.paused = !self.paused,
            Command::SetSpeed(s) => match s {
                Speed::Max => self.full_speed = true,
                m => {
                    self.speed = m;
                    self.full_speed = false;
                }
            },
            Command::ToggleFullSpeed => self.full_speed = !self.full_speed,
            Command::Step => {
                if self.debugger.mode == RunMode::Debug {
                    self.debugger.execute(&mut self.machine, "T");
                } else {
                    self.video.step(&mut self.machine);
                    self.mixer.produce_machine(&mut self.machine);
                }
            }
            Command::SaveState(p) => self.save_state(&p),
            Command::LoadState(p) => self.load_state(&p),
            Command::SetStatePath(p) => self.settings.storage.savestate_filename = Some(p),
            Command::ApplySettings(s) => self.rebuild(*s),
            Command::UpdateSettings(s) => {
                self.video = self.backend.frame_source(&s);
                let sp = speed_from_settings(&s);
                self.full_speed = sp == Speed::Max;
                if let Speed::Multiplier(_) = sp {
                    self.speed = sp;
                }
                apple2_storage::disk2::manager::set_enhance_disk(
                    &mut self.machine,
                    s.storage.enhance_disk_speed,
                );
                self.settings = *s;
            }
            Command::SetWriteProtect { drive, on } => {
                self.disks.set_write_protect(&mut self.machine, drive, on)
            }
            Command::SetVolume(v) => {
                self.settings.sound.master_volume = v;
            }
            Command::InsertDisk { drive, path } => {
                match self.disks.insert(&mut self.machine, drive, &path) {
                    Ok(()) => self.say(format!("Drive {}: {}", drive + 1, path.display())),
                    Err(e) => self.say(e),
                }
            }
            Command::EjectDisk { drive } => self.disks.eject(&mut self.machine, drive),
            Command::SwapDrives => self.disks.swap(&mut self.machine),
            Command::VideoMode(d) => {
                self.video.cycle_video_mode(d);
                let d = self.video.description();
                self.say(format!("Video: {d}"));
            }
            Command::ToggleScanlines => self.video.toggle_scanlines(),
            Command::RockerSwitch => self.video.toggle_rocker_switch(),
            Command::MouseMove { dx, dy } => self.mouse.motion(&mut self.machine, dx, dy),
            Command::MouseButton { button, down } => {
                self.mouse.button(&mut self.machine, button, down)
            }
            Command::ToggleDebugger => {
                if self.debugger.mode == RunMode::Running {
                    self.debugger.enter(&mut self.machine);
                } else if self.debugger.mode == RunMode::Stepping {
                    self.debugger.stop_stepping();
                } else {
                    self.debugger.exit(&mut self.machine);
                }
            }
            Command::Debug(k) => {
                let d = &mut self.debugger;
                match k {
                    DebugKey::Char(c) => {
                        d.console.input_char(c);
                    }
                    DebugKey::Backspace => {
                        d.console.backspace();
                    }
                    DebugKey::Enter => {
                        d.input_enter(&mut self.machine);
                    }
                    DebugKey::HistoryPrev => d.console.history_prev(),
                    DebugKey::HistoryNext => d.console.history_next(),
                    DebugKey::ClearInput => d.console.clear_input(),
                    DebugKey::ScrollUp => d.console.scroll_up(4),
                    DebugKey::ScrollDown => d.console.scroll_down(4),
                }
            }
            Command::Shutdown => {
                self.shutdown_state();
                self.disks.flush(&mut self.machine);
                return false;
            }
        }
        true
    }

    fn soft_reset(&mut self) {
        self.machine.reset();
        self.video.reset();
    }

    /// Volume gain for the host-side master control.
    pub fn master_gain(&self) -> f32 {
        volume_gain(self.settings.sound.master_volume, VOLUME_MAX)
    }
}

/// Handle to the emulation thread.
pub struct EmuHandle {
    pub tx: Sender<Command>,
    pub shared: Arc<Shared>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl EmuHandle {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for EmuHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Starts the emulation thread. `ring` is the audio ring (None: no sound device); `notify` is
/// called whenever a new frame was published (it wakes the UI).
pub fn spawn(
    settings: Settings,
    factory: BackendFactory,
    ring: Option<Arc<AudioRing>>,
    audio_rate: u32,
    notify: Box<dyn Fn() + Send>,
) -> Result<EmuHandle, String> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let shared = Arc::new(Shared::default());
    let sh = shared.clone();
    let join = std::thread::Builder::new()
        .name("emulation".into())
        .spawn(
            move || match Emulator::new(settings, factory(), audio_rate) {
                Ok(mut emu) => {
                    emu.startup_state();
                    let _ = ready_tx.send(Ok(()));
                    drive(emu, rx, &sh, ring, &*notify);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            },
        )
        .map_err(|e| e.to_string())?;
    ready_rx.recv().map_err(|e| e.to_string())??;
    Ok(EmuHandle {
        tx,
        shared,
        join: Some(join),
    })
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Paused,
    Max,
    AudioPaced,
    Timer,
}

const PUBLISH_PERIOD: Duration = Duration::from_millis(15);
const AUDIO_TARGET_SECS: f64 = 0.08;

fn drive(
    mut emu: Emulator,
    rx: Receiver<Command>,
    shared: &Shared,
    ring: Option<Arc<AudioRing>>,
    notify: &dyn Fn(),
) {
    let start = Instant::now();
    let mut pacer = TimerPacer::new(emu.clock_hz());
    let mut done: u64 = 0;
    let mut mode = Mode::Timer;
    let mut last_publish = Instant::now() - PUBLISH_PERIOD;
    let mut rate_anchor = (Instant::now(), emu.machine.cycles());
    let mut running = true;
    let mut dirty = true;
    if let Some(r) = &ring {
        r.set_gain(emu.master_gain());
    }

    while running {
        // 1. commands
        while let Ok(c) = rx.try_recv() {
            if let Command::SetVolume(v) = &c {
                if let Some(r) = &ring {
                    r.set_gain(volume_gain(*v, VOLUME_MAX));
                }
            }
            if matches!(c, Command::ApplySettings(_)) {
                if let Some(r) = &ring {
                    r.clear();
                }
            }
            running &= emu.apply(c);
            dirty = true;
        }
        if !running {
            break;
        }

        // 2. pick the pacing mode
        let new_mode = if emu.paused || emu.debugger_halted() {
            Mode::Paused
        } else if emu.effective_full_speed() {
            Mode::Max
        } else if emu.speed.is_normal() && ring.is_some() {
            Mode::AudioPaced
        } else {
            Mode::Timer
        };
        let now = start.elapsed();
        if new_mode != mode {
            mode = new_mode;
            pacer.set_clock(now, done, emu.clock_hz());
            if let Speed::Multiplier(m) = emu.speed {
                pacer.set_multiplier(now, m);
            }
            if let Some(r) = &ring {
                // Leaving audio pacing fades the output out and then empties the ring.
                r.set_active(mode == Mode::AudioPaced);
            }
        }

        // 3. run
        let mut ran = false;
        match mode {
            Mode::Paused => {
                wait_command(&rx, &mut emu, Duration::from_millis(8), &mut running);
            }
            Mode::Max => {
                let t0 = Instant::now();
                while t0.elapsed() < Duration::from_millis(12) {
                    let (c, _) = emu.run_frame();
                    done += c;
                    ran = true;
                }
            }
            Mode::AudioPaced => {
                let r = ring.as_ref().expect("audio mode needs a ring");
                let per = emu.audio_frames_per_machine_frame();
                let target = (f64::from(emu.audio_rate) * AUDIO_TARGET_SECS) as usize;
                let n = audio_frames_due(r.fill_frames(), target, per, 4);
                for _ in 0..n {
                    let (c, audio) = emu.run_frame();
                    done += c;
                    r.push(&audio);
                    ran = true;
                }
                if n == 0 {
                    wait_command(&rx, &mut emu, Duration::from_millis(2), &mut running);
                }
            }
            Mode::Timer => {
                let cpf = emu.cycles_per_frame();
                let mut due = pacer.due(now, done);
                let mut n = 0;
                while due >= cpf && n < 8 {
                    let (c, _) = emu.run_frame();
                    done += c;
                    due = due.saturating_sub(c);
                    ran = true;
                    n += 1;
                }
                if !ran {
                    wait_command(&rx, &mut emu, Duration::from_millis(2), &mut running);
                }
            }
        }

        // 4. publish
        if (ran || dirty)
            && last_publish.elapsed()
                >= if mode == Mode::Max {
                    PUBLISH_PERIOD
                } else {
                    Duration::from_millis(8)
                }
            || (mode == Mode::Paused && last_publish.elapsed() >= Duration::from_millis(33))
        {
            let t = rate_anchor.0.elapsed().as_secs_f32();
            if t >= 0.5 {
                let mhz = (emu.machine.cycles().saturating_sub(rate_anchor.1)) as f32 / t / 1e6;
                emu.set_measured(mhz, ring.as_ref().map_or(0, |r| r.underruns()));
                rate_anchor = (Instant::now(), emu.machine.cycles());
            }
            emu.render();
            let st = emu.status();
            shared.publish(emu.frame(), st);
            last_publish = Instant::now();
            dirty = false;
            notify();
        }
    }
}

/// Sleeps up to `dur` but wakes (and applies the command) as soon as one arrives.
fn wait_command(rx: &Receiver<Command>, emu: &mut Emulator, dur: Duration, running: &mut bool) {
    match rx.recv_timeout(dur) {
        Ok(c) => *running &= emu.apply(c),
        Err(RecvTimeoutError::Timeout) => {}
        Err(RecvTimeoutError::Disconnected) => *running = false,
    }
}
