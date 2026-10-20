//! `--headless-frames N --dump-screen out.png`: runs the same [`Emulator`] without a window or
//! audio device, as fast as possible, and writes the final frame. Used by CI to check the
//! machine -> renderer -> framebuffer -> PNG path end to end.

use crate::backend::Backend;
use crate::emu::{Command, Emulator};
use crate::screenshot::save_png;
use apple2_config::Settings;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct HeadlessOptions {
    pub frames: u64,
    pub dump_screen: Option<PathBuf>,
    /// Text to type (as a paste) once the machine had time to boot.
    pub type_text: Option<String>,
    pub type_at_frame: u64,
    /// Print the text screen to stdout at the end.
    pub dump_text: bool,
    pub load_state: Option<PathBuf>,
}

pub fn run(settings: Settings, backend: Box<dyn Backend>, opts: &HeadlessOptions) -> Result<Vec<String>, String> {
    let mut emu = Emulator::new(settings, backend, 48_000)?;
    if let Some(p) = &opts.load_state {
        emu.apply(Command::LoadState(p.clone()));
    }
    let mut typed = false;
    for f in 0..opts.frames {
        if !typed && f >= opts.type_at_frame {
            if let Some(t) = &opts.type_text {
                emu.apply(Command::Paste(t.clone()));
            }
            typed = true;
        }
        emu.run_frame();
    }
    let frame = emu.render().clone();
    if let Some(path) = &opts.dump_screen {
        save_png(path, &frame)?;
    }
    Ok(apple2_core::text_screen::text_screen(&emu.machine))
}
