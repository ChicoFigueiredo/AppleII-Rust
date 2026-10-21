//! Keyboard shortcuts of the debugger (`DebuggerProcessKey`, Debug.cpp:9653-10098).
//!
//! The frontend translates host key events into [`Key`] + [`KeyMods`] and calls
//! [`Debugger::process_key`]; every decision AppleWin makes (an empty input line navigates, typed
//! text edits, the console window scrolls) lives here so it can be tested without a window.
//!
//! In AppleWin the key code (`WM_KEYDOWN`) and the typed character (`WM_CHAR`) arrive separately.
//! Here one event carries both, so [`Key::Char`] first runs the `DebuggerProcessKey` rules
//! (Space, Ctrl+digit, Ctrl+`~`) and only then falls through to `DebuggerInputConsoleChar`.

use apple2_core::Machine;

use crate::debugger::*;
use crate::display::{CONSOLE_LINES, GRID_H};

/// A key the debugger reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A printable character (`WM_CHAR`), or the key's own character when Ctrl is held.
    Char(char),
    Enter,
    Backspace,
    Escape,
    Tab,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
}

/// `KeybGetShiftStatus`, `KeybGetCtrlStatus`, `KeybGetAltStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyMods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Where the cursor line sits after a jump (`CURSOR_ALIGN_TOP` / `CURSOR_ALIGN_CENTER`; the
/// commands `.`, `RET`... pass nothing and keep the current line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Keep,
    Top,
    Center,
}

impl Debugger {
    /// Applies `align` to the cursor line (`g_nDisasmCurLine = 0` / `WindowUpdateDisasmSize`).
    pub(crate) fn align_cursor(&mut self, align: Align) {
        match align {
            Align::Keep => {}
            Align::Top => self.disasm_cur_line = 0,
            Align::Center => self.update_window_size(),
        }
    }

    /// `CmdCursorJumpPC( align )`.
    pub fn cursor_jump_pc_at(&mut self, m: &Machine, align: Align) -> Update {
        self.disasm_cur = m.cpu().pc;
        self.align_cursor(align);
        self.calc_top_bot(m);
        upd::ALL
    }

    /// `CmdCursorJumpRetAddr( align )`.
    pub fn cursor_jump_ret_at(&mut self, m: &Machine, align: Align) -> Update {
        self.disasm_cur = Self::stack_return_address(m);
        self.align_cursor(align);
        self.calc_top_bot(m);
        upd::ALL
    }

    /// `CmdCursorRunUntil`: `GG <cursor>`.
    pub fn cursor_run_until(&mut self, m: &mut Machine) -> Update {
        let until = self.disasm_cur;
        self.begin_go(m.cpu().pc, Some(until), None, true);
        self.attach(m);
        upd::CONSOLE_DISPLAY
    }

    /// `_CmdWindowViewFull` + `WindowUpdateSizes`.
    fn switch_window(&mut self, w: Window) {
        if self.window != w {
            self.last_window = self.window;
        }
        self.window = w;
        self.update_window_size();
    }

    /// `ToggleFullScreenConsole`.
    pub fn toggle_full_screen_console(&mut self) -> Update {
        if self.window != Window::Console {
            self.switch_window(Window::Console);
        } else {
            let back = self.last_window;
            self.switch_window(back);
        }
        upd::ALL
    }

    /// `CmdWindowCycleNext` / `CmdWindowCyclePrev` over the windows the port has (code, data,
    /// console: the AppleWin's `WINDOW_INFO` and `WINDOW_NULL` entries have nothing to show).
    pub fn cycle_window(&mut self, next: bool) -> Update {
        const ORDER: [Window; 3] = [Window::Code, Window::Data, Window::Console];
        let i = ORDER.iter().position(|w| *w == self.window).unwrap_or(0);
        let j = if next { (i + 1) % 3 } else { (i + 2) % 3 };
        self.switch_window(ORDER[j]);
        upd::ALL
    }

    /// Rows a console page scroll moves (`g_nConsoleDisplayLines - CONSOLE_FIRST_LINE`).
    fn console_page(&self) -> usize {
        if self.window == Window::Console {
            GRID_H - 2
        } else {
            CONSOLE_LINES - 1
        }
    }

    fn console_scroll(&mut self, up: bool, n: usize) -> Update {
        if up {
            self.console.scroll_up(n)
        } else {
            self.console.scroll_down(n)
        }
        upd::CONSOLE_DISPLAY
    }

    /// Leaves or changes the "view output" screen (`DebugVideoMode`, Debug.cpp:9658-9740).
    /// Digits 0-8 pick a page like `HGR0`..`HGR8`; `9` shows the current mode; any other key
    /// returns to the debugger screen.
    fn key_in_output_view(&mut self, key: Key) -> Update {
        if let Key::Char(c @ '0'..='9') = key {
            self.output_view = Some(match c {
                '9' => OutputView::Current,
                n => OutputView::HgrPseudo(n as u8 - b'0'),
            });
            return upd::NOTHING;
        }
        self.output_view = None;
        upd::ALL
    }

    /// `DebuggerProcessKey` (plus `DebuggerInputConsoleChar` for printable characters).
    pub fn process_key(&mut self, m: &mut Machine, key: Key, mods: KeyMods) -> Update {
        if self.mode == RunMode::Running {
            return upd::NOTHING;
        }
        if self.output_view.is_some() {
            return self.key_in_output_view(key);
        }
        let has_input = !self.console.input().is_empty();
        let in_console = self.window == Window::Console;
        match key {
            Key::Escape => {
                self.console.clear_input();
                upd::CONSOLE_INPUT
            }
            Key::Backspace => {
                self.console.backspace();
                upd::CONSOLE_INPUT
            }
            Key::Enter => {
                if has_input {
                    self.console.scroll_end();
                    self.input_enter(m) | upd::ALL
                } else if self.asm_active {
                    // "Pressing enter on blank line while in assembler wouldn't exit it" (2.6.1.35).
                    self.execute_line(m, "", false)
                } else {
                    self.toggle_full_screen_console()
                }
            }
            Key::Char(c) if mods.ctrl && matches!(c, '~' | '`' | '\u{ac}') => {
                self.toggle_full_screen_console()
            }
            Key::Tab => {
                if has_input {
                    // ConsoleInputTabCompletion() is an empty stub in AppleWin.
                    upd::CONSOLE_INPUT
                } else if mods.ctrl && mods.shift {
                    self.cycle_window(false)
                } else if mods.ctrl {
                    self.cycle_window(true)
                } else {
                    self.cursor_jump_pc_at(m, Align::Center)
                }
            }
            Key::Char(' ') => {
                if has_input || self.asm_active {
                    // `DebuggerInputConsoleChar`: the assembler and a non-empty line keep it.
                    self.console.input_char(' ');
                    upd::CONSOLE_INPUT
                } else if mods.shift {
                    self.cmd_step_out(m)
                } else if mods.ctrl {
                    self.step_over_n(m, 1)
                } else {
                    self.trace_n(m, 1)
                }
            }
            Key::Home => {
                if in_console || (has_input && mods.shift) {
                    self.console.scroll_home();
                } else if !has_input {
                    self.disasm_cur = 0x0000;
                    self.calc_top_bot(m);
                }
                upd::DISASM | upd::CONSOLE_DISPLAY
            }
            Key::End => {
                if in_console || (has_input && mods.shift) {
                    self.console.scroll_end();
                } else if !has_input {
                    self.disasm_cur = 0x8000;
                    self.calc_top_bot(m);
                }
                upd::DISASM | upd::CONSOLE_DISPLAY
            }
            Key::PageUp | Key::PageDown => {
                let up = key == Key::PageUp;
                if in_console || (has_input && mods.shift) {
                    let n = self.console_page();
                    self.console_scroll(up, n)
                } else if has_input {
                    // "Scroll through console input history": 3 console lines.
                    self.console_scroll(up, 3)
                } else {
                    let align = if mods.shift {
                        256
                    } else if mods.ctrl {
                        4096
                    } else {
                        0
                    };
                    self.cursor_page(m, up, align)
                }
            }
            Key::Up => {
                if in_console || (has_input && mods.shift) {
                    self.console_scroll(true, 1)
                } else if has_input {
                    upd::NOTHING // AppleWin: "TODO: FIXME: Scroll through console input history"
                } else if mods.shift {
                    self.cursor_line_up_bytes(m, 1)
                } else {
                    self.cursor_line_up(m)
                }
            }
            Key::Down => {
                if in_console || (has_input && mods.shift) {
                    self.console_scroll(false, 1)
                } else if has_input {
                    upd::NOTHING
                } else if mods.ctrl {
                    self.cursor_run_until(m)
                } else if mods.shift {
                    self.cursor_line_down_bytes(m, 1)
                } else {
                    self.cursor_line_down(m)
                }
            }
            Key::Right => {
                if mods.ctrl {
                    self.cmd_cursor_set_pc(m)
                } else if mods.shift {
                    self.cursor_jump_pc_at(m, Align::Top)
                } else if mods.alt {
                    self.cursor_jump_pc_at(m, Align::Center)
                } else {
                    self.cursor_follow_target_at(m, Align::Top)
                }
            }
            Key::Left => {
                let align = if mods.shift {
                    Align::Top
                } else {
                    Align::Center
                };
                self.cursor_jump_ret_at(m, align)
            }
            Key::Char(c) if mods.ctrl && c.is_ascii_digit() => {
                let n = c as u32 - '0' as u32;
                if mods.shift {
                    let line = format!("BMA {n} ${:04X}", self.disasm_cur);
                    self.execute_line(m, &line, false)
                } else {
                    self.execute_line(m, &format!("BMG {n}"), false)
                }
            }
            Key::Char(c) => {
                if !mods.ctrl {
                    self.console.input_char(c);
                }
                upd::CONSOLE_INPUT
            }
        }
    }
}
