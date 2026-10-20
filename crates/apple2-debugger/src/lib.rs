//! UI-less port of the AppleWin debugger (Debugger/*.cpp): command console, expression parser,
//! disassembler/assembler, symbols, breakpoints and the debugger screen as a text grid.
//!
//! The frontend owns a [`Machine`](apple2_core::Machine) and a [`Debugger`]:
//! - F7: [`Debugger::enter`]; while `mode == Debug` it draws [`Debugger::render`] and feeds typed
//!   characters to `console.input_char`, Enter calls [`Debugger::input_enter`].
//! - While [`Debugger::is_stepping`] it calls [`Debugger::run`] instead of `run_cycles`.

#![forbid(unsafe_code)]

pub mod args;
pub mod assembler;
pub mod bp;
mod cmd_bp;
mod cmd_exec;
mod cmd_mem;
mod cmd_misc;
mod cmd_sym;
pub mod cmdtable;
pub mod color;
pub mod commands;
pub mod console;
pub mod debugger;
pub mod disasm;
pub mod display;
pub mod help;
mod helptext;
pub mod host;
pub mod mem;
pub mod opcodes;
pub mod symbols;

pub use debugger::{upd, Debugger, RunMode, Update};
pub use display::{Grid, GridCell};
