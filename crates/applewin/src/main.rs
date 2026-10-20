use applewin_shell::backend::DefaultBackend;
use applewin_shell::headless::{self, HeadlessOptions};

fn main() {
    let (opts, rest) = match shell_args(std::env::args().skip(1).collect()) {
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
    let mut settings = apple2_config::Settings::load_default().unwrap_or_default();
    cmd.apply(&mut settings);
    if let Some(h) = opts {
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
    }
}

/// Splits the shell's own flags from the AppleWin command line.
fn shell_args(args: Vec<String>) -> Result<(Option<HeadlessOptions>, Vec<String>), String> {
    let mut h = HeadlessOptions { type_at_frame: 180, ..Default::default() };
    let mut headless = false;
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
            "--type" => h.type_text = Some(val(&a)?.replace("\\r", "\r").replace("\\n", "\r")),
            "--type-at-frame" => h.type_at_frame = val(&a)?.parse().map_err(|_| "bad frame".to_string())?,
            _ => rest.push(a),
        }
    }
    Ok((headless.then_some(h), rest))
}
