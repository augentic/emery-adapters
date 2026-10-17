//! Holds every stem's modules to the host boundary: no `crates/start/`, no
//! lifecycle function or type, and no simulated host mechanism.

use emery_sdk::Error;

use super::{CATALOG, Tree, spells, start_files};

// What no module of a stem spells, each the process the source was: a
// function that starts, stops, runs, listens, serves, bootstraps, or shuts
// it down, or a type that is it. Each is matched as a word, so
// `fn start_at` and `struct AppError` are none.
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

// What no module of a stem spells, each a mechanism the host owns: the
// workspace's lints silenced for the crate, a task, a timer, a thread, a
// reach past the component to the system, a loop that polls or retries, or
// a flag machine standing in for a lifecycle.
const SIMULATED: &[(&str, &str)] = &[
    ("#![allow(", "the workspace's lints silenced"),
    ("tokio::spawn", "a task of its own"),
    ("tokio::time", "a timer of its own"),
    ("std::thread", "a thread of its own"),
    ("std::env", "the environment read past the host"),
    ("std::fs", "the file system reached past the host"),
    ("std::net", "a socket of its own"),
    ("std::process", "a process of its own"),
    ("loop {", "a loop that polls or retries"),
    ("_STARTED_KEY", "a start-up flag of its own"),
    ("_INITIALIZED_KEY", "an initialisation flag of its own"),
    ("_LOADED_KEY", "a load-done flag of its own"),
    ("STOP_REQUESTED", "a stop request of its own"),
    ("EXIT_DELAY", "an exit delay of its own"),
    ("b\"1\"", "a step-done write of its own"),
];

impl Tree<'_> {
    // Three findings at most: any file laid under `crates/start/` (or a
    // `crates/start-*` crate), whose repair removes the crate, the words of
    // `LIFECYCLE` under a stem's `src/`, whose repair removes the module, and,
    // in the modules that remain, the spellings of `SIMULATED`, whose repair
    // removes the spelling. A tree with no `crates/` answers none.
    pub(super) fn simulated_host(&self) -> Result<Vec<String>, Error> {
        let start = start_files(self.root)?;
        let mut lifecycle = Vec::new();
        let mut simulated = Vec::new();
        for module in &self.modules {
            let path = &module.path;
            let mut process = Vec::new();
            for (number, line) in module.code.lines().enumerate() {
                for (word, what) in LIFECYCLE {
                    if spells(line, word) {
                        process.push(format!("{path}:{}: {what} (`{word}`)", number + 1));
                    }
                }
            }
            if !process.is_empty() {
                lifecycle.extend(process);
                continue;
            }
            for (number, line) in module.code.lines().enumerate() {
                for (needle, what) in SIMULATED {
                    let found = if *needle == "loop {" {
                        spells(line, needle)
                    } else {
                        line.contains(needle)
                    };
                    if found {
                        simulated.push(format!("{path}:{}: {what} (`{needle}`)", number + 1));
                    }
                }
            }
        }
        let mut findings = Vec::new();
        if !start.is_empty() {
            findings.push(format!(
                "`crates/start/` is laid for `start`, which the seed and the host are and no crate \
                 of the tree ever is: a deployment setting a handler reads is a constant of \
                 `{CATALOG}`, a cross-cutting rule is a `pub fn` of `common`, a stated health \
                 route is its own stem's, and how the source readied itself, loaded its \
                 credentials or tables, or shut down is laid nowhere; remove every file named \
                 under `crates/start` through `delete` and drop any `start` line from the root \
                 `Cargo.toml` through `edit_files`:\n{}",
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
        if !simulated.is_empty() {
            simulated.sort();
            findings.push(format!(
                "a module simulates the host, which the translation lays nothing for; remove each \
                 spelling from its module through `edit_files`, never by renaming it: a loop's \
                 actions run once, back to back, a failure carried as the classified error, and \
                 never as a function that calls itself in the loop's place; a payload of \
                 repeated elements, XML among them, is read through `serde` into a type deriving \
                 `Deserialize` (`quick_xml::de::from_str`), never walked by an event loop; a \
                 flag's step is read at the moment of use; a task's, a timer's, or a reach past \
                 the host's work is laid as the translation says or left out:\n{}",
                simulated.join("\n")
            ));
        }
        Ok(findings)
    }
}
