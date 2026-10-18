//! Holds every stem's modules to what `start` becomes: no `crates/start/`
//! and no lifecycle function or type, since the seed and the host are the
//! process the source was.

use emery_sdk::Error;

use super::{Tree, spells, start_files};

const LIFECYCLE: &[(&str, &str)] = &[
    ("fn start", "a start of its own"),
    ("fn stop", "a stop of its own"),
    ("fn run", "a run of its own"),
    ("fn listen", "a listener of its own"),
    ("fn serve", "a server of its own"),
    ("fn bootstrap", "a bootstrap of its own"),
    ("fn shutdown", "a shutdown of its own"),
    ("struct Main", "a lifecycle type of its own"),
    ("struct App", "a lifecycle type of its own"),
    ("struct Server", "a lifecycle type of its own"),
    ("struct Application", "a lifecycle type of its own"),
];

impl Tree<'_> {
    pub(super) fn process(&self) -> Result<Vec<String>, Error> {
        let start = start_files(self.root)?;
        let mut lifecycle = Vec::new();
        for module in &self.modules {
            for (number, line) in module.code.lines().enumerate() {
                for (word, what) in LIFECYCLE {
                    if spells(line, word) {
                        lifecycle.push(format!(
                            "{}:{}: {what} (`{word}`)",
                            module.path,
                            number + 1
                        ));
                    }
                }
            }
        }
        let mut findings = Vec::new();
        if !start.is_empty() {
            findings.push(format!(
                "`crates/start/` is laid for `start`, which the seed and the host are and no crate \
                 of the tree ever is: a deployment setting a handler reads is a `pub(crate) \
                 const` of the part that reads it, a cross-cutting rule is a `pub fn` of the \
                 stem that owns it, a stated health route is its own stem's, and how the source \
                 readied itself, loaded its credentials or tables, or shut down is laid nowhere; \
                 remove every file named under `crates/start` through `delete` and drop any \
                 `start` line from the root `Cargo.toml` through `edit_files`:\n{}",
                start.join("\n")
            ));
        }
        if !lifecycle.is_empty() {
            lifecycle.sort();
            findings.push(format!(
                "a module is the process the source was, which the host is and the tree never \
                 simulates; remove each module named, with its `pub mod` line and its test, \
                 through `delete` rather than rename it, and leave what it stood for to the \
                 report:\n{}",
                lifecycle.join("\n")
            ));
        }
        Ok(findings)
    }
}
