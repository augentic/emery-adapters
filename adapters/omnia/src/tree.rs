//! Holds the lent tree to the layout's rules before a build's report or a
//! verification's verdict is accepted.
//!
//! Each rule answers the findings the turn repairs, one per rule with every
//! line named, so a report or a verdict is refused once with everything the
//! tree breaks.

mod misses;
mod simulated;
mod wiring;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::{Error, bad_request, server_error};
use omnia_template::Manifest;

// The shared crate, whose tables declare a constant and publish to none.
const COMMON: &str = "crates/common/src/";

// The configuration catalog, one `pub const` per key the deployment supplies.
const CATALOG: &str = "crates/common/src/config.rs";

// The most differing lines a finding asks an edit to put back; past it the
// file is laid whole.
const LISTED: usize = 24;

pub struct Tree<'a> {
    root: &'a Path,
}

impl<'a> Tree<'a> {
    pub const fn new(root: &'a Path) -> Self {
        Self { root }
    }

    // The rules a build and a verify share: the root manifest outside its
    // lists, no simulated host, no miss answered empty, no response wrapper
    // struct, the tables spelled into their lists, every handler listed,
    // every setting read.
    pub fn held(&self) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.manifest()?);
        findings.extend(self.simulated()?);
        findings.extend(self.misses()?);
        findings.extend(self.wrappers()?);
        findings.extend(self.tables()?);
        findings.extend(self.unlisted()?);
        findings.extend(self.settings()?);
        Ok(findings)
    }

    // One finding naming every crate the turn laid under the head of a
    // hyphenated stem of the slice, `crates/order/` for `order-items`: a
    // stem is one word, so its crate is `crates/<stem>/` whole. A crate the
    // tree held before the turn is an earlier slice's, and one emptied of
    // its files is repaired, since `delete` leaves the directory.
    pub fn stems(&self, stems: &Stems) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();
        for dir in crates(self.root)? {
            if dir == "common" || stems.held.contains(&dir) || stems.named.contains(&dir) {
                continue;
            }
            let split = stems.named.iter().find(|stem| {
                stem.strip_prefix(dir.as_str()).is_some_and(|rest| rest.starts_with('-'))
            });
            if let Some(stem) = split {
                lines.push(format!(
                    "`crates/{dir}/` is laid for `{dir}`, the head of the stem `{stem}`, whose \
                     crate is `crates/{stem}/` (package `{stem}`, path `{}`)",
                    stem.replace('-', "_")
                ));
            }
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![format!(
            "a stem is one word and is never split at its hyphen into a crate and a part: lay \
             each module under the stem's own crate, `crates/<stem>/src/<part>.rs`, with the \
             stem's manifest and module list beside it, remove every file under the split \
             directory through `delete`, and put the stem's two lines in the root `Cargo.toml` \
             in place of the head's through `edit_files`:\n{}",
            lines.join("\n")
        )])
    }

    // Every static file of the template that differs: the lines where an
    // edit puts them back, the template's body whole for a file the tree
    // lacks or rewrote past `LISTED` lines, since the lines alone cannot lay
    // it.
    pub fn statics(&self) -> Result<Vec<String>, Error> {
        let drift = omnia_template::drift(self.root).map_err(|err| refusal(&err))?;
        let mut findings = Vec::new();
        for file in drift.files() {
            let finding = if self.lacks(file.path) {
                format!(
                    "`{}` is a static file of the template the tree lacks; lay it whole through \
                     `write_files`, as the template spells it:\n{}",
                    file.path,
                    template(file.path)?
                )
            } else if file.lines.len() > LISTED {
                format!(
                    "`{}` is a static file of the template and differs from it in {} lines; lay it \
                     whole through `write_files`, as the template spells it:\n{}",
                    file.path,
                    file.lines.len(),
                    template(file.path)?
                )
            } else {
                format!(
                    "`{}` is a static file of the template and differs from how the template \
                     spells it; put the `-` lines back and drop the `+` lines through \
                     `edit_files`:\n{}",
                    file.path,
                    file.lines.join("\n")
                )
            };
            findings.push(finding);
        }
        Ok(findings)
    }

    // The root manifest outside its two stem lists, the template's whole for
    // a tree that lacks it.
    fn manifest(&self) -> Result<Vec<String>, Error> {
        let drift = Manifest::new(self.root).drift().map_err(|err| refusal(&err))?;
        if drift.is_empty() {
            return Ok(Vec::new());
        }
        let finding = if self.lacks(Manifest::PATH) {
            format!(
                "the tree lacks the root `{}`; lay it whole through `write_files` as the template \
                 spells it, then add each stem's line to its two lists:\n{}",
                Manifest::PATH,
                template(Manifest::PATH)?
            )
        } else {
            format!(
                "the root `{}` changed outside its two stem lists; the pins, the lints, the `host` \
                 feature, and the target tables stand as the template spells them, so put the `-` \
                 lines back and drop the `+` lines through `edit_files`:\n{drift}",
                Manifest::PATH
            )
        };
        Ok(vec![finding])
    }

    fn lacks(&self, path: &str) -> bool {
        matches!(
            fs::symlink_metadata(self.root.join(path)),
            Err(err) if err.kind() == ErrorKind::NotFound
        )
    }

    // The text at `path`, empty where the tree holds no file there.
    fn text(&self, path: &str) -> Result<String, Error> {
        let file = self.root.join(path);
        if file.is_file() { read(&file) } else { Ok(String::new()) }
    }

    // Every `pub const` the table at `path` declares, each its 1-based line
    // and name; an absent table declares none.
    fn constants(&self, path: &str) -> Result<Vec<(usize, String)>, Error> {
        let text = self.text(path)?;
        Ok(pub_consts(&text).map(|(line, name)| (line, name.to_owned())).collect())
    }

    // Every `.rs` file under a stem's `src/`, its `tests/` unread.
    fn modules(&self) -> Result<Vec<PathBuf>, Error> {
        let mut files = Vec::new();
        for stem in entries(&self.root.join("crates"))? {
            files.extend(sources(&stem.join("src"))?);
        }
        Ok(files)
    }

    fn relative(&self, file: &Path) -> String {
        file.strip_prefix(self.root).unwrap_or(file).display().to_string()
    }
}

// The stems a slice builds and the crates the tree held before its turn, so
// a crate the turn laid is told from one an earlier slice laid.
pub struct Stems {
    named: Vec<String>,
    held: Vec<String>,
}

impl Stems {
    // The stem of every requirement heading in `spec`, the first segment of
    // the subject's dotted id, beside the crates `root` holds now.
    pub fn read(spec: &str, root: &Path) -> Result<Self, Error> {
        let mut named: Vec<String> = Vec::new();
        for line in spec.lines() {
            let line = line.trim_start();
            if !line.starts_with('#') {
                continue;
            }
            let heading = line.trim_start_matches('#').trim_start();
            let Some(subject) = heading.strip_prefix("Requirement:") else {
                continue;
            };
            let Some(subject) = subject.split_whitespace().next() else {
                continue;
            };
            let stem = subject.split('.').next().unwrap_or(subject);
            if !stem.is_empty() && !named.iter().any(|named| named == stem) {
                named.push(stem.to_owned());
            }
        }
        Ok(Self {
            named,
            held: crates(root)?,
        })
    }
}

fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|err| server_error!("reading `{}`: {err}", file.display()))
}

// The body the template lays at `path`.
fn template(path: &str) -> Result<&'static str, Error> {
    omnia_template::entry(path)
        .map(|entry| entry.body)
        .ok_or_else(|| server_error!("the template lays no `{path}`"))
}

fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

// `text` with every comment line blanked and its newlines kept, so a
// statement a doc comment spells is none and line numbers hold.
fn code(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| if line.trim_start().starts_with("//") { "\n" } else { line })
        .collect()
}

// Whether `text` spells `word` whole: no identifier character continues it
// on either side, so `fn start` is spelled by `pub fn start<P>(` and not by
// `fn start_at(`, and `routes::http::ORDER` not by `routes::http::ORDERS`.
fn spells(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back().is_none_or(|ch| !identifier(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !identifier(ch));
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

fn identifier(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(identifier)
}

// `line` past any leading visibility (`pub` or `pub(..)`).
fn strip_vis(line: &str) -> &str {
    let rest = line.trim_start();
    let Some(after) = rest.strip_prefix("pub") else { return rest };
    if let Some(scoped) = after.strip_prefix('(') {
        return scoped.split_once(')').map_or(rest, |(_, tail)| tail.trim_start());
    }
    after.strip_prefix(char::is_whitespace).map_or(rest, str::trim_start)
}

// Every `pub const` in `text`, as its 1-based line and name.
fn pub_consts(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(index, line)| {
        let rest = line.trim_start().strip_prefix("pub const ")?;
        let name = rest.split(|ch: char| !identifier(ch)).next()?;
        (!name.is_empty()).then_some((index + 1, name))
    })
}

// The entries of `dir`, sorted; an absent `dir` has none.
fn entries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(server_error!("listing `{}`: {err}", dir.display())),
    };
    let mut paths = listing
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| server_error!("listing `{}`: {err}", dir.display()))?;
    paths.sort();
    Ok(paths)
}

// The name of every directory under `crates/` that holds a file, sorted; a
// tree with no `crates/` holds none.
fn crates(root: &Path) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    for path in entries(&root.join("crates"))? {
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
        if !meta.is_dir() || !occupied(&path)? {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

// Whether a regular file lies beneath `dir`, symbolic links left unfollowed.
fn occupied(dir: &Path) -> Result<bool, Error> {
    Ok(!files(dir)?.is_empty())
}

// Every regular file beneath `dir`, symbolic links left unfollowed, sorted.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

// Every `.rs` file beneath `dir`, symbolic links left unfollowed.
fn sources(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(files(dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect())
}
