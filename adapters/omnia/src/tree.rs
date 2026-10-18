//! Holds the lent tree to the layout's rules before a report or verdict is accepted.

mod lists;
mod misses;
mod process;

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::{Error, bad_request, server_error};
use omnia_template::Manifest;

const MAX_DIFF_LINES: usize = 24;

struct Module {
    path: String,
    code: String,
}

impl Module {
    fn read(root: &Path, file: &Path) -> Result<Self, Error> {
        let path = relative(root, file);
        let code = uncommented(&read(file)?);
        Ok(Self { path, code })
    }

    fn is_common(&self) -> bool {
        self.path.starts_with("crates/common/")
    }
}

pub struct Tree<'a> {
    root: &'a Path,
    modules: Vec<Module>,
}

impl<'a> Tree<'a> {
    pub fn read(root: &'a Path) -> Result<Self, Error> {
        let mut modules = Vec::new();
        for stem in entries(&root.join("crates"))? {
            if is_start_crate(&stem) {
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

    fn check_common(&self) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.manifest_drift()?);
        findings.extend(self.process()?);
        findings.extend(self.stem_lints()?);
        findings.extend(self.stems_merged()?);
        findings.extend(self.unnamed_topics()?);
        findings.extend(self.empty_misses());
        findings.extend(self.response_wrappers());
        Ok(findings)
    }

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

    // The workspace's lints are the host boundary, and they reach a crate
    // through its `[lints]` table alone.
    fn stem_lints(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for dir in crates(self.root)? {
            if dir == "common" || is_start_stem(&dir) {
                continue;
            }
            let path = format!("crates/{dir}/Cargo.toml");
            let file = self.root.join(&path);
            if file.is_file() && !inherits_lints(&read(&file)?) {
                lines.push(path);
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a stem's manifest carries `[lints]` with `workspace = true`, since the workspace's \
             lints are the host boundary and reach a crate through that table alone; add the \
             table through `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

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

    fn code(&self, path: &str) -> Result<String, Error> {
        let file = self.root.join(path);
        if file.is_file() { Ok(uncommented(&read(&file)?)) } else { Ok(String::new()) }
    }

    fn constants(&self, path: &str) -> Result<Vec<(usize, String)>, Error> {
        let file = self.root.join(path);
        if !file.is_file() {
            return Ok(Vec::new());
        }
        let text = read(&file)?;
        Ok(pub_consts(&text).map(|(line, name)| (line, name.to_owned())).collect())
    }
}

pub struct Stems {
    slice: BTreeSet<String>,
    existing: BTreeSet<String>,
}

impl Stems {
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

fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|err| server_error!("reading `{}`: {err}", file.display()))
}

fn template(path: &str) -> Result<&'static str, Error> {
    omnia_template::entry(path)
        .map(|entry| entry.body)
        .ok_or_else(|| server_error!("the template lays no `{path}`"))
}

pub fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).display().to_string()
}

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

// Whether a manifest inherits the workspace's lints, as `[lints]` holding
// `workspace = true` or the dotted `lints.workspace = true`.
fn inherits_lints(manifest: &str) -> bool {
    let mut in_lints = false;
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(table) = line.strip_prefix('[') {
            in_lints = table.trim_end_matches(']').trim() == "lints";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        if value == "true" && (in_lints && key == "workspace" || key == "lints.workspace") {
            return true;
        }
    }
    false
}

fn is_ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_ident_char)
}

fn strip_vis(line: &str) -> &str {
    let rest = line.trim_start();
    let Some(after) = rest.strip_prefix("pub") else { return rest };
    if let Some(scoped) = after.strip_prefix('(') {
        return scoped.split_once(')').map_or(rest, |(_, tail)| tail.trim_start());
    }
    after.strip_prefix(char::is_whitespace).map_or(rest, str::trim_start)
}

fn pub_consts(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(index, line)| {
        let rest = line.trim_start().strip_prefix("pub const ")?;
        let name = rest.split(|ch: char| !is_ident_char(ch)).next()?;
        (!name.is_empty()).then_some((index + 1, name))
    })
}

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

fn is_start_crate(dir: &Path) -> bool {
    dir.file_name().and_then(|name| name.to_str()).is_some_and(is_start_stem)
}

fn is_start_stem(name: &str) -> bool {
    name == "start"
        || name
            .strip_prefix("start")
            .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
}

fn start_files(root: &Path) -> Result<Vec<String>, Error> {
    let mut found = Vec::new();
    for dir in entries(&root.join("crates"))? {
        if !is_start_crate(&dir) {
            continue;
        }
        for file in files(&dir)? {
            found.push(relative(root, &file));
        }
    }
    found.sort();
    Ok(found)
}

// Ignores empty directories left behind by `delete`.
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

fn sources(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(files(dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect())
}
