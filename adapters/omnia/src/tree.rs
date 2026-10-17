//! Holds the lent tree to the layout's rules before a build's report or a
//! verification's verdict is accepted.
//!
//! [`Tree::read`] loads every stem module once and checks the tree in memory,
//! answering one finding per violated rule with every offending line named so
//! the turn repairs everything in one round.

mod misses;
mod simulated;
mod wiring;

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::target::Report;
use emery_sdk::{Error, bad_request, server_error, tracing};
use omnia_template::Manifest;

// The root ignore file, which the engine lays a one-line copy of at a laid
// repository's root commit.
const IGNORE: &str = ".gitignore";

// The shared crate, whose tables declare constants and publish to none.
const COMMON: &str = "crates/common/src/";

// The forbidden `start` crate directory: the seed and the host are `start`.
const START: &str = "crates/start";

// The configuration catalog, one `pub const` per key the deployment supplies.
const CATALOG: &str = "crates/common/src/config.rs";

// The most differing lines a finding asks an edit to put back; past it the
// file is laid whole.
const MAX_DIFF_LINES: usize = 24;

// One `.rs` module under `crates/<stem>/src/`, with comments stripped and line
// numbers preserved.
struct Module {
    path: String,
    name: String,
    code: String,
}

impl Module {
    fn read(root: &Path, file: &Path) -> Result<Self, Error> {
        let path = relative(root, file);
        let name = module_path(&path);
        let code = uncommented(&read(file)?);
        Ok(Self { path, name, code })
    }

    fn is_common(&self) -> bool {
        self.path.starts_with(COMMON)
    }
}

pub struct Tree<'a> {
    root: &'a Path,
    modules: Vec<Module>,
}

impl<'a> Tree<'a> {
    pub fn read(root: &'a Path) -> Result<Self, Error> {
        let start = root.join(START);
        let mut modules = Vec::new();
        for stem in entries(&root.join("crates"))? {
            if stem == start {
                continue;
            }
            for file in sources(&stem.join("src"))? {
                modules.push(Module::read(root, &file)?);
            }
        }
        Ok(Self { root, modules })
    }

    pub fn check_build(&self, stems: &Stems) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.split_stems(stems)?);
        findings.extend(self.check_common()?);
        Ok(findings)
    }

    pub fn check_verify(&self) -> Result<Vec<String>, Error> {
        let mut findings = self.static_drift()?;
        findings.extend(self.check_common()?);
        Ok(findings)
    }

    // The rules a build and a verify share: the root manifest outside its
    // lists, no simulated host, no miss answered empty, no response wrapper
    // struct, the tables spelled into their lists, every handler listed, and
    // every setting read.
    fn check_common(&self) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.manifest_drift()?);
        findings.extend(self.simulated_host()?);
        findings.extend(self.empty_misses());
        findings.extend(self.response_wrappers());
        findings.extend(self.unwired_tables()?);
        findings.extend(self.unlisted_handlers()?);
        findings.extend(self.unread_settings()?);
        Ok(findings)
    }

    // Names every crate the turn laid under the head of a hyphenated stem of
    // the slice, `crates/order/` for `order-items`: a stem is one word, so its
    // crate is `crates/<stem>/` whole. A crate the tree had before the turn is
    // an earlier slice's, and one emptied of its files is repaired, since
    // `delete` leaves the directory.
    fn split_stems(&self, stems: &Stems) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for dir in crates(self.root)? {
            if dir == "common" || stems.existing.contains(&dir) || stems.slice.contains(&dir) {
                continue;
            }
            let split = stems.slice.iter().find(|stem| {
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
            return Ok(None);
        }
        Ok(Some(format!(
            "a stem is one word and is never split at its hyphen into a crate and a part: lay \
             each module under the stem's own crate, `crates/<stem>/src/<part>.rs`, with the \
             stem's manifest and module list beside it, remove every file under the split \
             directory through `delete`, and put the stem's two lines in the root `Cargo.toml` \
             in place of the head's through `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    // Every static file of the template that differs: the lines where an edit
    // puts them back, or the template's body whole for a file the tree lacks
    // or rewrote past `MAX_DIFF_LINES` lines.
    fn static_drift(&self) -> Result<Vec<String>, Error> {
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
            } else if file.lines.len() > MAX_DIFF_LINES {
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

    // The root manifest outside its two stem lists, the template's whole for a
    // tree that lacks it.
    fn manifest_drift(&self) -> Result<Option<String>, Error> {
        let drift = Manifest::new(self.root).drift().map_err(|err| refusal(&err))?;
        if drift.is_empty() {
            return Ok(None);
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
        Ok(Some(finding))
    }

    fn lacks(&self, path: &str) -> bool {
        matches!(
            fs::symlink_metadata(self.root.join(path)),
            Err(err) if err.kind() == ErrorKind::NotFound
        )
    }

    // The uncommented text at `path`, empty where the tree holds no file there.
    fn code(&self, path: &str) -> Result<String, Error> {
        let file = self.root.join(path);
        if file.is_file() { Ok(uncommented(&read(&file)?)) } else { Ok(String::new()) }
    }

    // Every `pub const` the table at `path` declares, each its 1-based line
    // and name; an absent table declares none.
    fn constants(&self, path: &str) -> Result<Vec<(usize, String)>, Error> {
        let file = self.root.join(path);
        if !file.is_file() {
            return Ok(Vec::new());
        }
        let text = read(&file)?;
        Ok(pub_consts(&text).map(|(line, name)| (line, name.to_owned())).collect())
    }
}

// The stems a slice builds and the crate directories the tree already had
// before its turn, so a crate the turn laid is told from an earlier slice's.
pub struct Stems {
    slice: BTreeSet<String>,
    existing: BTreeSet<String>,
}

impl Stems {
    // The stem of every requirement heading in `spec`, the first segment of
    // the subject's dotted id, beside the crates `root` has before the turn.
    pub fn read(spec: &str, root: &Path) -> Result<Self, Error> {
        let mut slice = BTreeSet::new();
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
            if !stem.is_empty() {
                slice.insert(stem.to_owned());
            }
        }
        Ok(Self {
            slice,
            existing: crates(root)?.into_iter().collect(),
        })
    }
}

// Overwrites a held `.gitignore` (such as the engine's root-commit `.emery/`
// rule) with the template's, since `seed` only lays missing files.
pub fn restore_ignore(root: &Path, slice: &str) -> Result<bool, Error> {
    let body = template(IGNORE)?;
    let path = root.join(IGNORE);
    let current = match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {
            fs::read(&path).map_err(|err| server_error!("reading `{IGNORE}`: {err}"))?
        }
        Ok(_) => return Err(bad_request!("a directory or symbolic link stands at `{IGNORE}`")),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(server_error!("reading `{IGNORE}`: {err}")),
    };
    if current == body.as_bytes() {
        return Ok(false);
    }
    fs::write(&path, body).map_err(|err| server_error!("writing `{IGNORE}`: {err}"))?;
    tracing::debug!(%slice, "the root ignore file put back as the template spells it");
    Ok(true)
}

pub fn record_ignore(report: &mut Report) {
    if let Err(at) = report.written.binary_search_by(|path| path.as_str().cmp(IGNORE)) {
        report.written.insert(at, IGNORE.to_owned());
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

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).display().to_string()
}

// The Rust module path of a `.rs` file under `crates/<stem>/src/`,
// `orders::reading` for `crates/orders/src/reading.rs`.
fn module_path(path: &str) -> String {
    let rest = path.strip_prefix("crates/").unwrap_or(path);
    let rest = rest.strip_suffix(".rs").unwrap_or(rest);
    let Some((stem, module)) = rest.split_once("/src/") else {
        return rest.replace(['-', '/'], "_");
    };
    let stem = stem.replace('-', "_");
    let module = module.strip_suffix("/mod").unwrap_or(module);
    if module == "lib" { stem } else { format!("{stem}::{}", module.replace('/', "::")) }
}

// `text` with every `//` comment stripped and its newlines kept, so a
// statement or keyword a comment spells is none and line numbers hold.
fn uncommented(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let newline = line.ends_with('\n');
        let content = line.strip_suffix('\n').unwrap_or(line);
        let code = match comment_start(content) {
            Some(0) => "",
            Some(at) => content[..at].trim_end(),
            None => content,
        };
        out.push_str(code);
        if newline {
            out.push('\n');
        }
    }
    out
}

// The byte offset where a `//` comment begins outside a string literal.
fn comment_start(line: &str) -> Option<usize> {
    if line.trim_start().starts_with("//") {
        return Some(0);
    }
    if !line.contains("//") {
        return None;
    }
    let mut in_string = false;
    let mut escaped = false;
    for (at, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        } else if line[at..].starts_with("//") {
            return Some(at);
        }
    }
    None
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
        let before = text[..start].chars().next_back().is_none_or(|ch| !is_ident_char(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !is_ident_char(ch));
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn is_ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_ident_char)
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
        let name = rest.split(|ch: char| !is_ident_char(ch)).next()?;
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
        if !meta.is_dir() || !has_files(&path)? {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

// Whether a regular file lies beneath `dir`, symbolic links left unfollowed.
fn has_files(dir: &Path) -> Result<bool, Error> {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_file() {
                return Ok(true);
            } else if meta.is_dir() {
                pending.push(path);
            }
        }
    }
    Ok(false)
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
