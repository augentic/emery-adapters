//! `engine` — the shared harness wrapper binary over the first-party
//! catalog: CLI dev shim (default), HTTP (`serve`), and the
//! live-model trial (`eval`).

use std::process::ExitCode;

use engine::{Adapters, SHELL};

fn main() -> ExitCode {
    harness::entry::main::<Adapters>(&SHELL)
}
