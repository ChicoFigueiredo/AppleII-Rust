//! `applewin-dap`: Debug Adapter Protocol server over stdio. See the crate docs.

use apple2_dap::protocol::{read_message, write_message};
use apple2_dap::Session;
use serde_json::Value;
use std::io::{self, BufReader};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

fn main() {
    // A reader thread lets the main loop notice `pause` while the machine is running.
    let (tx, rx) = mpsc::channel::<Value>();
    thread::spawn(move || {
        let mut input = BufReader::new(io::stdin().lock());
        while let Ok(Some(msg)) = read_message(&mut input) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    let mut out = io::stdout().lock();
    let mut session = Session::new();
    loop {
        let msg = if session.is_running() {
            match rx.try_recv() {
                Ok(m) => Some(m),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            }
        };
        if let Some(m) = msg {
            if m["type"] == "request" {
                session.handle(&m);
            }
        }
        if session.is_running() {
            session.run_slice();
            // Give the reader thread a chance on a single core.
            thread::yield_now();
        }
        for o in session.take_output() {
            if write_message(&mut out, &o).is_err() {
                return;
            }
        }
        if session.should_exit() {
            break;
        }
    }
}
