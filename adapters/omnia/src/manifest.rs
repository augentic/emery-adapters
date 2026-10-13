//! Holds the root manifest to the seed outside its two stem lists.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::{Error, server_error};

// The seed line each list follows: the path list runs from `common`'s line
// to the blank line after it, the guest's `[dependencies]` list from
// `tracing`'s line to the end of the file. An entry is kept by its shape
// wherever it stands, and put back after its anchor.
const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
const CRATE_LIST_ANCHOR: &str = "tracing.workspace = true";

pub const FILE: &str = "Cargo.toml";

// Past this many lines the tree's manifest is reported whole rather than
// diffed, since the diff's table grows with the product of the two lengths.
const DIFFED_LINES: usize = 2_000;

// The root manifest of one tree beside the seed's: the one list file whose
// static text, the pins, the lints, the feature, and the target tables, the
// merge rules cannot hold apart from its lists.
pub struct Manifest<'a> {
    path: PathBuf,
    seed: &'a str,
}

impl<'a> Manifest<'a> {
    pub fn new(root: &str, seed: &'a str) -> Self {
        Self {
            path: Path::new(root).join(FILE),
            seed,
        }
    }

    pub fn drift(&self) -> Result<Drift, Error> {
        Ok(self.split()?.drift)
    }

    // Puts every line outside the two lists back as the seed spells it,
    // keeping each list's entries after its anchor, and returns what
    // `drift` would have.
    pub fn restore(&self) -> Result<Drift, Error> {
        let split = self.split()?;
        if !split.drift.is_empty() {
            std::fs::write(&self.path, split.render(self.seed))
                .map_err(|err| server_error!("writing `{}`: {err}", self.path.display()))?;
        }
        Ok(split.drift)
    }

    fn split(&self) -> Result<Split, Error> {
        match std::fs::read_to_string(&self.path) {
            Ok(held) => Ok(Split::of(&held, self.seed)),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Split::missing()),
            Err(err) => Err(server_error!("reading `{}`: {err}", self.path.display())),
        }
    }
}

// Each line of the tree's manifest that is neither the seed's nor an entry
// of a list, `+ ` before it, and each line of the seed the tree lacks, `- `
// before it; empty when the manifest stands.
#[derive(Debug, Default)]
pub struct Drift(Vec<String>);

impl Drift {
    // The most lines `Display` spells out before counting the rest.
    const LISTED: usize = 24;

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let listed = self.0.len().min(Self::LISTED);
        f.write_str(&self.0[..listed].join("\n"))?;
        match self.0.len() - listed {
            0 => Ok(()),
            more => write!(f, "\n… and {more} more"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Paths,
    Crates,
}

impl List {
    fn after(line: &str) -> Option<Self> {
        match line {
            PATH_LIST_ANCHOR => Some(Self::Paths),
            CRATE_LIST_ANCHOR => Some(Self::Crates),
            _ => None,
        }
    }

    // `<stem> = { path = "crates/<dir>" }` is the path list's entry,
    // `<stem>.workspace = true` the crate list's.
    fn entry(line: &str) -> Option<Self> {
        if let Some(stem) = line.strip_suffix(".workspace = true") {
            return is_stem(stem).then_some(Self::Crates);
        }
        let (stem, rest) = line.split_once(" = { path = \"crates/")?;
        let dir = rest.strip_suffix("\" }")?;
        (is_stem(stem) && is_stem(dir)).then_some(Self::Paths)
    }
}

#[derive(Default)]
struct Split {
    paths: Vec<String>,
    crates: Vec<String>,
    drift: Drift,
}

impl Split {
    // A held line shaped as an entry, and not a line of the seed, is its
    // list's, once; the rest, a second copy of an entry included, is diffed
    // against the seed.
    fn of(held: &str, seed: &str) -> Self {
        let mut split = Self::default();
        let mut rest = Vec::new();
        for line in held.lines() {
            let entry = if seed.lines().any(|own| own == line) { None } else { List::entry(line) };
            let list = match entry {
                Some(List::Paths) => &mut split.paths,
                Some(List::Crates) => &mut split.crates,
                None => {
                    rest.push(line);
                    continue;
                }
            };
            if list.iter().any(|held| held == line) {
                rest.push(line);
            } else {
                list.push(line.to_owned());
            }
        }
        split.drift = Drift(if rest.len() > DIFFED_LINES {
            vec![format!("+ {} lines in place of the seed's", rest.len())]
        } else {
            diff(&seed.lines().collect::<Vec<_>>(), &rest)
        });
        split
    }

    fn missing() -> Self {
        Self {
            drift: Drift(vec![format!("- `{FILE}`, which the tree lacks")]),
            ..Self::default()
        }
    }

    fn render(&self, seed: &str) -> String {
        let mut text = String::new();
        for line in seed.lines() {
            text.push_str(line);
            text.push('\n');
            let entries = match List::after(line) {
                Some(List::Paths) => &self.paths,
                Some(List::Crates) => &self.crates,
                None => continue,
            };
            for entry in entries {
                text.push_str(entry);
                text.push('\n');
            }
        }
        text
    }
}

// The lines of `seed` and `held` outside their longest common subsequence,
// in order, `- ` before the seed's and `+ ` before the tree's.
fn diff(seed: &[&str], held: &[&str]) -> Vec<String> {
    let (n, m) = (seed.len(), held.len());
    let mut common = vec![vec![0_usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i][j] = if seed[i] == held[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }

    let mut drift = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if seed[i] == held[j] {
            i += 1;
            j += 1;
        } else if common[i + 1][j] >= common[i][j + 1] {
            drift.push(format!("- {}", seed[i]));
            i += 1;
        } else {
            drift.push(format!("+ {}", held[j]));
            j += 1;
        }
    }
    drift.extend(seed[i..].iter().map(|line| format!("- {line}")));
    drift.extend(held[j..].iter().map(|line| format!("+ {line}")));
    drift
}

fn is_stem(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
