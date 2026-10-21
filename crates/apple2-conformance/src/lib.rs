//! Conformance harness (plan sections 4 and 17): reference traces, run manifests, declarative
//! cases and JUnit reports. It drives the machine only through `apple2-core`'s public API; no
//! hook was added to the core.

#![forbid(unsafe_code)]

pub mod audio;
pub mod case;
pub mod import_check;
pub mod manifest;
pub mod pcm;
pub mod report;
pub mod runner;
pub mod sha256;
pub mod state;
pub mod trace;

pub use case::{Case, InputEvent};
pub use manifest::{Manifest, TestResult};
pub use runner::{run_case, Env, Outcome};
pub use trace::{
    compare, compare_with, CompareOptions, Divergence, Field, Trace, TraceReader, TraceRow,
    TraceWriter,
};
