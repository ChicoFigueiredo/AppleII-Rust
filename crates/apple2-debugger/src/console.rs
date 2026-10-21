//! Debugger console (Debugger_Console.cpp/.h): an 80-column colored scrollback, the input line with
//! its history, and the color-escape markup (`` `1 `` red, `` `2 `` green ... ``CHC_*``).

use std::collections::VecDeque;

pub const CONSOLE_WIDTH: usize = 80;
/// `CONSOLE_HEIGHT`: scrollback kept.
pub const CONSOLE_HEIGHT: usize = 768;
pub const HISTORY_HEIGHT: usize = 128;
pub const CONSOLE_COLOR_ESCAPE: char = '`';

// Color escapes (Debugger_Console.h:77).
pub const CHC_DEFAULT: &str = "`0";
pub const CHC_USAGE: &str = "`3";
pub const CHC_CATEGORY: &str = "`6";
pub const CHC_COMMAND: &str = "`2";
pub const CHC_KEY: &str = "`1";
pub const CHC_ARG_MAND: &str = "`7";
pub const CHC_ARG_OPT: &str = "`4";
pub const CHC_ARG_SEP: &str = "`9";
pub const CHC_NUM_DEC: &str = "`:";
pub const CHC_NUM_HEX: &str = "`3";
pub const CHC_SYMBOL: &str = "`2";
pub const CHC_ADDRESS: &str = "`8";
pub const CHC_ERROR: &str = "`1";
pub const CHC_WARNING: &str = "`8";
pub const CHC_INFO: &str = "`3";
pub const CHC_STRING: &str = "`6";
pub const CHC_EXAMPLE: &str = "`:";
pub const CHC_PATH: &str = "`:";
pub const CHC_REGS: &str = "`6";

/// `g_anConsoleColor` (Debugger_Display.cpp:105); index 0 is the configurable default foreground.
pub const CONSOLE_PALETTE: [(u8, u8, u8); 11] = [
    (255, 255, 255),
    (255, 32, 32),
    (0, 255, 0),
    (255, 255, 0),
    (64, 64, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
    (255, 128, 0),
    (128, 128, 128),
    (80, 192, 255),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    /// Index into [`CONSOLE_PALETTE`].
    pub color: u8,
}

pub type Line = Vec<Cell>;

/// `ConsoleOutputLevel_e` (the LOG command).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutputLevel {
    None,
    Error,
    Warn,
    Info,
    Default,
    All,
}

#[derive(Debug)]
pub struct Console {
    lines: VecDeque<Line>,
    /// Lines scrolled back from the newest (`g_iConsoleDisplayStart`).
    scroll: usize,
    input: Vec<char>,
    history: Vec<String>,
    history_pos: usize,
    prompt: char,
    pub level: OutputLevel,
    /// Console foreground color override for the default escape (`FG_CONSOLE_OUTPUT`).
    pub default_color: (u8, u8, u8),
}

impl Default for Console {
    fn default() -> Self {
        Console {
            lines: VecDeque::new(),
            scroll: 0,
            input: Vec::new(),
            history: Vec::new(),
            history_pos: 0,
            prompt: '>',
            level: OutputLevel::Default,
            default_color: CONSOLE_PALETTE[0],
        }
    }
}

/// Parses color-escape markup into cells (no wrapping).
pub fn parse_markup(s: &str, start_color: u8) -> (Line, u8) {
    let mut out = Vec::new();
    let mut color = start_color;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == CONSOLE_COLOR_ESCAPE {
            match it.peek() {
                Some(&n) if n.is_ascii_digit() => {
                    color = n as u8 - b'0';
                    it.next();
                    continue;
                }
                Some(&':') => {
                    color = 10;
                    it.next();
                    continue;
                }
                Some(&'`') => {
                    it.next();
                }
                _ => {}
            }
        }
        out.push(Cell { ch: c, color });
    }
    (out, color)
}

/// Text of a line without colors.
pub fn line_string(l: &Line) -> String {
    l.iter()
        .map(|c| c.ch)
        .collect::<String>()
        .trim_end()
        .to_string()
}

impl Console {
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    /// `ConsolePrint`: a (possibly multi-line) markup string; long lines wrap at 80 columns and the
    /// color carries over.
    pub fn print(&mut self, text: &str) {
        let mut color = 0u8;
        for piece in text.split('\n') {
            let (cells, c) = parse_markup(piece, color);
            color = c;
            if cells.is_empty() {
                self.push_line(Vec::new());
            }
            for chunk in cells.chunks(CONSOLE_WIDTH) {
                self.push_line(chunk.to_vec());
            }
        }
        self.scroll = 0;
    }

    fn push_line(&mut self, l: Line) {
        if self.lines.len() >= CONSOLE_HEIGHT {
            self.lines.pop_front();
        }
        self.lines.push_back(l);
    }

    /// `ConsoleDisplayError`: red text.
    pub fn error(&mut self, text: &str) {
        self.print(&format!("{CHC_ERROR}{text}"));
    }

    /// Print only if the LOG level is at least `lvl` (`ConsolePrintFormat` families).
    pub fn print_level(&mut self, lvl: OutputLevel, text: &str) {
        if self.level >= lvl && self.level != OutputLevel::None {
            self.print(text);
        }
    }

    /// The `height` newest-to-scroll lines ending `scroll` lines above the newest.
    pub fn visible(&self, height: usize) -> Vec<&Line> {
        let end = self.lines.len().saturating_sub(self.scroll);
        let start = end.saturating_sub(height);
        self.lines.range(start..end).collect()
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = (self.scroll + n).min(self.lines.len().saturating_sub(1));
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    pub fn scroll_home(&mut self) {
        self.scroll = self.lines.len().saturating_sub(1);
    }

    pub fn scroll_end(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    // ---- input line ----

    pub fn prompt(&self) -> char {
        self.prompt
    }

    /// `>` for commands, `!` while the assembler is active (`g_aConsolePrompt`).
    pub fn set_prompt(&mut self, p: char) {
        self.prompt = p;
    }

    pub fn input(&self) -> String {
        self.input.iter().collect()
    }

    pub fn set_input(&mut self, s: &str) {
        self.input = s.chars().take(CONSOLE_WIDTH - 2).collect();
    }

    /// `ConsoleInputChar`: lower case is folded to upper outside quotes unless `keep_case`.
    pub fn input_char(&mut self, c: char) -> bool {
        if self.input.len() >= CONSOLE_WIDTH - 2 || c.is_control() {
            return false;
        }
        self.input.push(c);
        true
    }

    pub fn backspace(&mut self) -> bool {
        self.input.pop().is_some()
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
    }

    /// Enter: returns the line, stores it in the history and echoes it (`prompt` + text).
    pub fn submit(&mut self) -> String {
        let line = self.input();
        self.input.clear();
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            if self.history.len() >= HISTORY_HEIGHT {
                self.history.remove(0);
            }
            self.history.push(line.clone());
        }
        self.history_pos = self.history.len();
        self.scroll = 0;
        let echo = format!("{CHC_DEFAULT}{}{line}", self.prompt);
        let (cells, _) = parse_markup(&echo, 0);
        self.push_line(cells);
        line
    }

    pub fn history_prev(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            let h = self.history[self.history_pos].clone();
            self.set_input(&h);
        }
    }

    pub fn history_next(&mut self) {
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            let h = self.history[self.history_pos].clone();
            self.set_input(&h);
        } else {
            self.history_pos = self.history.len();
            self.clear_input();
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_sets_colors_and_wraps() {
        let mut c = Console::default();
        c.print("`1red `2green\nplain");
        let l: Vec<&Line> = c.lines().collect();
        assert_eq!(line_string(l[0]), "red green");
        assert_eq!(l[0][0].color, 1);
        assert_eq!(l[0][4].color, 2);
        assert_eq!(l[1][0].color, 2, "the color carries over to the next line");
        c.print(&"x".repeat(100));
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn input_history_and_scroll() {
        let mut c = Console::default();
        for ch in "bp fded".chars() {
            c.input_char(ch);
        }
        assert_eq!(c.submit(), "bp fded");
        assert_eq!(line_string(c.lines().last().unwrap()), ">bp fded");
        c.history_prev();
        assert_eq!(c.input(), "bp fded");
        c.history_next();
        assert_eq!(c.input(), "");
        for i in 0..30 {
            c.print(&format!("line {i}"));
        }
        assert_eq!(line_string(c.visible(3)[2]), "line 29");
        c.scroll_up(5);
        assert_eq!(line_string(c.visible(3)[2]), "line 24");
        c.scroll_end();
        assert_eq!(c.scroll(), 0);
    }
}
