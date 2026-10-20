use applewin_shell::backend::DefaultBackend;
use applewin_shell::headless::{self, HeadlessOptions};

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
    let (opts, gui, rest) = match shell_args(std::env::args().skip(1).collect()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let cmd = match apple2_config::CmdLine::parse(&rest) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let Some(h) = opts {
        // headless runs ignore the user's saved settings: reproducible output for CI
        let mut settings = apple2_config::Settings::default();
        cmd.apply(&mut settings);
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
    let mut settings = apple2_config::Settings::load_default().unwrap_or_default();
    cmd.apply(&mut settings);
    let run = applewin_shell::app::run(applewin_shell::app::Options {
        settings,
        factory: Box::new(|| Box::new(DefaultBackend)),
        no_audio,
        load_state: cmd.load_state.clone(),
        open: cmd.positional.clone(),
        dump_window: gui.dump_window,
        type_text: gui.type_text,
    });
    if let Err(e) = run {
        eprintln!("{e}");
        std::process::exit(1);
    }
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
