use applewin_shell::backend::DefaultBackend;
use applewin_shell::emu::{Command, Emulator};
use applewin_shell::headless::{self, HeadlessOptions};
use apple2_config::{CmdLine, Settings};

fn main() {
    let no_audio = std::env::args().any(|a| a == "--no-audio");
    // Informational flags answer before anything touches a window or the saved settings.
    // `-h` / `-V` are shell conveniences (AppleWin itself has -help / -version).
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| matches!(a.as_str(), "--help" | "-help" | "-?" | "-h")) {
        print!("{}", apple2_config::cmdline::HELP);
        return;
    }
    if argv.iter().any(|a| matches!(a.as_str(), "--version" | "-version" | "-V")) {
        println!("applewin {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let (opts, gui, rest) = match shell_args(argv) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mut cmd = match CmdLine::parse(&rest) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    // `-current-dir`: where relative paths (images, printer file, save-states) resolve.
    if let Some(dir) = &cmd.current_dir {
        if let Err(e) = std::env::set_current_dir(dir) {
            eprintln!("-current-dir {}: {e}", dir.display());
            std::process::exit(2);
        }
    }
    // Relative paths given before `-current-dir` took effect are not a concern: AppleWin has the same order.
    for w in &cmd.warnings {
        eprintln!("warning: {w}");
    }
    if !cmd.unsupported.is_empty() {
        eprintln!("warning: unsupported command line options: {}", cmd.unsupported.join(" "));
    }
    // `-load-state` is the current save-state file for this session (Snapshot_SetFilename).
    let apply = |settings: &mut Settings, cmd: &CmdLine| {
        cmd.apply(settings);
        if let Some(p) = &cmd.load_state {
            settings.storage.savestate_filename = Some(p.clone());
        }
    };

    if let Some(path) = cmd.screenshot_and_exit.clone() {
        // Reproducible: no saved settings (unless -conf names a file).
        let mut settings = conf_settings(&cmd).unwrap_or_default();
        apply(&mut settings, &cmd);
        if let Err(e) = screenshot_and_exit(settings, &cmd, &path) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(mut h) = opts {
        // headless runs ignore the user's saved settings: reproducible output for CI
        let mut settings = conf_settings(&cmd).unwrap_or_default();
        apply(&mut settings, &cmd);
        h.load_state = cmd.load_state.clone();
        match headless::run(settings, Box::new(DefaultBackend), &h) {
            Ok(text) => {
                if h.dump_text {
                    for l in text {
                        println!("{l}");
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let mut settings = conf_settings(&cmd).unwrap_or_else(|| Settings::load_default().unwrap_or_default());
    apply(&mut settings, &cmd);
    // `-power-on` starts the machine running; this shell always does (there is no logo screen).
    cmd.power_on = true;
    let run = applewin_shell::app::run(applewin_shell::app::Options {
        settings,
        factory: Box::new(|| Box::new(DefaultBackend)),
        no_audio,
        load_state: cmd.load_state.clone(),
        open: std::mem::take(&mut cmd.positional),
        dump_window: gui.dump_window,
        type_text: gui.type_text,
    });
    if let Err(e) = run {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// `-conf <file>`: an AppleWin `.ini` / `.reg` is imported once; any other file is our TOML
/// settings, used (and saved to) instead of the default location.
fn conf_settings(cmd: &CmdLine) -> Option<Settings> {
    let path = cmd.conf_file.as_ref()?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "ini" | "reg" => match std::fs::read(path) {
            Ok(b) if ext == "ini" => Some(apple2_config::import::import_ini(&String::from_utf8_lossy(&b))),
            Ok(b) => Some(apple2_config::import::import_reg(&b)),
            Err(e) => {
                eprintln!("-conf {}: {e}", path.display());
                std::process::exit(2);
            }
        },
        _ => {
            apple2_config::store::set_path_override(path.clone());
            match Settings::load(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("-conf {}: {e}", path.display());
                    std::process::exit(2);
                }
            }
        }
    }
}

/// `-screenshot-and-exit <file>` (with `-load-state`): builds the machine, loads the state,
/// repaints the screen once and writes it as PNG (`Video_RedrawAndTakeScreenShot`).
fn screenshot_and_exit(settings: Settings, cmd: &CmdLine, path: &std::path::Path) -> Result<(), String> {
    let mut emu = Emulator::new(settings, Box::new(DefaultBackend), 48_000)?;
    if let Some(p) = &cmd.load_state {
        emu.apply(Command::LoadState(p.clone()));
        let msg = emu.status().message;
        if msg.starts_with("Load state failed") {
            return Err(msg);
        }
    }
    let frame = emu.redraw_now().clone();
    applewin_shell::screenshot::save_png(path, &frame)
}

/// Splits the shell's own flags from the AppleWin command line.
struct GuiArgs {
    dump_window: Option<(std::path::PathBuf, u64)>,
    type_text: Option<String>,
}

fn shell_args(args: Vec<String>) -> Result<(Option<HeadlessOptions>, GuiArgs, Vec<String>), String> {
    let mut h = HeadlessOptions { type_at_frame: 180, ..Default::default() };
    let mut headless = false;
    let mut dump_window: Option<std::path::PathBuf> = None;
    let mut window_frames = 120u64;
    let mut rest = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut val = |n: &str| it.next().ok_or(format!("{n} needs a value"));
        match a.as_str() {
            "--headless-frames" => {
                h.frames = val(&a)?.parse().map_err(|_| "bad frame count".to_string())?;
                headless = true;
            }
            "--dump-screen" => h.dump_screen = Some(val(&a)?.into()),
            "--dump-text" => h.dump_text = true,
            "--no-audio" => {}
            "--dump-window" => dump_window = Some(val(&a)?.into()),
            "--window-frames" => window_frames = val(&a)?.parse().map_err(|_| "bad frame count".to_string())?,
            "--type" => h.type_text = Some(val(&a)?.replace("\\r", "\r").replace("\\n", "\r")),
            "--type-at-frame" => h.type_at_frame = val(&a)?.parse().map_err(|_| "bad frame".to_string())?,
            _ => rest.push(a),
        }
    }
    let gui = GuiArgs { dump_window: dump_window.map(|p| (p, window_frames)), type_text: h.type_text.clone() };
    Ok((headless.then_some(h), gui, rest))
}
