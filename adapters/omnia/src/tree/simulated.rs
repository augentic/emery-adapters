//! Holds every stem's modules to the host boundary: no `crates/start/`, no
//! lifecycle function or type, no simulated host mechanism, and no struct
//! wrapping an HTTP response in a single `data` field.

use emery_sdk::Error;

use super::{CATALOG, COMMON, Tree, code, files, ident, identifier, read, spells, strip_vis};

// The forbidden `start` crate directory: the seed and the host are `start`.
const START: &str = "crates/start";
const START_PREFIX: &str = "crates/start/";

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
    // Three findings at most: any file laid under `crates/start/`, whose
    // repair removes the crate, the words of `LIFECYCLE` under a stem's
    // `src/`, whose repair removes the module, and, in the modules that
    // remain, the spellings of `SIMULATED`, whose repair removes the
    // spelling. A tree with no `crates/` answers none.
    pub(super) fn simulated(&self) -> Result<Vec<String>, Error> {
        let start: Vec<String> =
            files(&self.root.join(START))?.into_iter().map(|file| self.relative(&file)).collect();
        let mut lifecycle = Vec::new();
        let mut simulated = Vec::new();
        for file in self.modules()? {
            let path = self.relative(&file);
            if path.starts_with(START_PREFIX) {
                continue;
            }
            let text = code(&read(&file)?);
            let mut process = Vec::new();
            for (number, line) in text.lines().enumerate() {
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
            for (number, line) in text.lines().enumerate() {
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
                "`{START}/` is laid for `start`, which the seed and the host are and no crate of \
                 the tree ever is: a deployment setting a handler reads is a constant of \
                 `{CATALOG}`, a cross-cutting rule is a `pub fn` of `common`, a stated health \
                 route is its own stem's, and how the source readied itself, loaded its \
                 credentials or tables, or shut down is laid nowhere; remove every file under \
                 `{START}/` through `delete` and drop any `start` line from the root `Cargo.toml` \
                 through `edit_files`:\n{}",
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

    // One finding naming every `Deserialize` struct under a stem's `src/`
    // whose body holds a single `data` field, each `<path>:<line>: <what>`:
    // a client's `.data` property wrapped around the response body the wire
    // carries at its root.
    pub(super) fn wrappers(&self) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();
        for file in self.modules()? {
            let path = self.relative(&file);
            if path.starts_with(COMMON) || path.starts_with(START_PREFIX) {
                continue;
            }
            let text = code(&read(&file)?);
            for (number, name) in wrappers(&text) {
                lines.push(format!(
                    "{path}:{number}: response wrapper struct `{name}` wraps a single `data` field"
                ));
            }
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        lines.sort();
        Ok(vec![format!(
            "a struct wraps an HTTP response in a single `data` field, which is the source's \
             HTTP client property (`response.data`) and never a field on the wire: remove each \
             wrapper struct named through `edit_files` and deserialize `response.body()` \
             directly into the body's own shape, a top-level `Vec<T>` for a JSON array or a \
             struct with the body's fields at its root:\n{}",
            lines.join("\n")
        )])
    }
}

// Every `Deserialize` struct `text` declares whose body holds a single
// `data` field, each its 1-based line and name: a client's `.data` holder
// wrapped around the response body.
fn wrappers(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(name) = opens_struct(line) else { continue };
        if !derives_deserialize(&lines, index) {
            continue;
        }
        let Some(end) = lines[index + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with('}'))
            .map(|offset| index + 1 + offset)
        else {
            continue;
        };
        let fields: Vec<&str> =
            lines[index + 1..end].iter().filter_map(|line| field_name(line)).collect();
        if fields == ["data"] {
            found.push((index + 1, name));
        }
    }
    found
}

fn opens_struct(line: &str) -> Option<&str> {
    let rest = strip_vis(line).strip_prefix("struct ")?;
    let name = rest.split(|ch: char| !identifier(ch)).next()?;
    (!name.is_empty() && line.trim_end().ends_with('{')).then_some(name)
}

fn derives_deserialize(lines: &[&str], index: usize) -> bool {
    for line in lines[..index].iter().rev() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if text.ends_with('}') || text.ends_with(';') || text.ends_with('{') {
            return false;
        }
        if spells(text, "Deserialize") {
            return true;
        }
    }
    false
}

fn field_name(line: &str) -> Option<&str> {
    let rest = line.trim_start();
    if rest.is_empty() || rest.starts_with('#') {
        return None;
    }
    let (name, _) = strip_vis(rest).split_once(':')?;
    let name = name.trim();
    ident(name).then_some(name)
}
