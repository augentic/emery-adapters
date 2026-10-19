//! Holds the lent tree to the layout's rules before a report or verdict is accepted.

mod lists;
mod misses;
mod process;
mod syntax;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::target::Report;
use emery_sdk::{Error, bad_request, server_error};
use omnia_template::Manifest;
use syn::visit::{self, Visit};
use syn::{ItemFn, Macro};

use self::syntax::Unparsed;
use crate::scaffold::Scaffold;

const MAX_DIFF_LINES: usize = 24;

struct Module {
    path: String,
    // The text with its comments blanked, for the rules that read a name
    // off a line and for spelling a line into a finding.
    code: String,
    parsed: Result<syn::File, Unparsed>,
}

impl Module {
    fn read(root: &Path, file: &Path) -> Result<Self, Error> {
        let path = relative(root, file);
        let text = read(file)?;
        Ok(Self {
            path,
            code: uncommented(&text),
            parsed: syntax::parse(&text),
        })
    }

    fn is_common(&self) -> bool {
        self.path.starts_with("crates/common/")
    }

    fn file(&self) -> Option<&syn::File> {
        self.parsed.as_ref().ok()
    }

    fn spelled(&self, number: usize) -> String {
        number
            .checked_sub(1)
            .and_then(|index| self.code.lines().nth(index))
            .unwrap_or_default()
            .trim()
            .trim_end_matches(',')
            .to_owned()
    }

    fn markers(&self) -> Vec<(usize, String)> {
        let mut markers = Markers::default();
        if let Some(file) = self.file() {
            markers.visit_file(file);
        }
        markers.found
    }
}

#[derive(Default)]
struct Markers {
    found: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for Markers {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.found.extend(syntax::todo_marker(mac));
        visit::visit_macro(self, mac);
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

    pub fn check_build(&self, scaffold: &Scaffold, report: &Report) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.split_stems(scaffold)?);
        findings.extend(self.strays(scaffold)?);
        findings.extend(self.markers_left(&report.covered));
        findings.extend(self.scenarios_tested(scaffold, &report.covered)?);
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
        findings.extend(self.unparsed());
        findings.extend(self.manifest_drift()?);
        findings.extend(self.process()?);
        findings.extend(self.stem_lints()?);
        findings.extend(self.unused_dependencies()?);
        findings.extend(self.stems_merged()?);
        findings.extend(self.unnamed_topics()?);
        findings.extend(self.unreached_publishers()?);
        findings.extend(self.empty_misses());
        findings.extend(self.response_wrappers());
        findings.extend(self.swallowed_rejections());
        findings.extend(self.format_settings());
        Ok(findings)
    }

    // The crates the turn wrote that are no stem of the slice's and none the
    // tree held; `crates/start/` is the process rule's.
    fn foreign_crates(&self, scaffold: &Scaffold) -> Result<Vec<String>, Error> {
        let stems: Vec<&str> = scaffold.stems().collect();
        Ok(crates(self.root)?
            .into_iter()
            .filter(|dir| {
                !(dir == "common"
                    || is_start_stem(dir)
                    || scaffold.existing().contains(dir)
                    || stems.iter().any(|stem| crate::scaffold::crate_dir(stem) == dir))
            })
            .collect())
    }

    // A crate the turn wrote is one of the slice's stems' or none of the
    // tree's.
    fn split_stems(&self, scaffold: &Scaffold) -> Result<Option<String>, Error> {
        let stems: Vec<&str> = scaffold.stems().collect();
        let mut lines = Vec::new();
        for dir in self.foreign_crates(scaffold)? {
            let split = stems.iter().find(|stem| {
                stem.strip_prefix(dir.as_str()).is_some_and(|rest| rest.starts_with('-'))
            });
            lines.push(split.map_or_else(
                || {
                    if stems.is_empty() {
                        format!(
                            "`crates/{dir}/` is written for `{dir}`, and this slice has no stem \
                             of its own: every requirement of it is `start`'s"
                        )
                    } else {
                        format!(
                            "`crates/{dir}/` is written for `{dir}`, which is no stem of this \
                             slice ({})",
                            stems
                                .iter()
                                .map(|stem| format!("`{stem}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                },
                |stem| {
                    let stem_dir = crate::scaffold::crate_dir(stem);
                    let pkg = crate::scaffold::package(stem);
                    format!(
                        "`crates/{dir}/` is written for `{dir}`, the head of the stem `{stem}`, \
                         whose crate is `crates/{stem_dir}/` (package `{pkg}`, path `{}`)",
                        pkg.replace('-', "_")
                    )
                },
            ));
        }
        if lines.is_empty() {
            return Ok(None);
        }
        let rule = if stems.is_empty() {
            "the seed and the host are `start`, so a slice of `start` requirements alone \
             scaffolds no crate and writes none: a stated route lands in the crate of the stem \
             its resource names (`GET /health/live` in `crates/health/`), a cross-cutting rule is \
             a `pub fn` of the stem it guards, a setting is a `pub(crate) const` of the part that \
             reads it, and how the process readied itself is written nowhere and left out of \
             `covered`, as the guardrails' table says; remove every file under each directory \
             named through `delete` and drop its lines from the root `Cargo.toml` through \
             `edit_files`"
        } else {
            "a slice writes modules under the crates of its own stems alone, and a stem is one \
             word and is never split at its hyphen into a crate and a part: write each module \
             under `crates/<stem>/src/<part>.rs` for the stem its requirements lead with, remove \
             every file under the other directory through `delete`, and drop its lines from the \
             root `Cargo.toml` through `edit_files`"
        };
        Ok(Some(format!("{rule}:\n{}", lines.join("\n"))))
    }

    // A file the turn added outside the slice's crates, beyond the files the
    // tree held once the skeleton was written: a crate `split_stems` or the
    // process rule names is that finding's, so one problem is one finding.
    fn strays(&self, scaffold: &Scaffold) -> Result<Option<String>, Error> {
        let mut own: Vec<String> = scaffold
            .stems()
            .map(|stem| format!("crates/{}/", crate::scaffold::crate_dir(stem)))
            .collect();
        own.extend(self.foreign_crates(scaffold)?.into_iter().map(|dir| format!("crates/{dir}/")));
        own.extend(
            crates(self.root)?
                .into_iter()
                .filter(|dir| is_start_stem(dir))
                .map(|dir| format!("crates/{dir}/")),
        );
        let lines: Vec<String> = listing(self.root)?
            .difference(scaffold.listing())
            .filter(|path| path.as_str() != "Cargo.lock")
            .filter(|path| !own.iter().any(|dir| path.starts_with(dir.as_str())))
            .map(|path| format!("{path}: written beside the slice"))
            .collect();
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a slice lands in its own stems' crates and the lines it adds to the lists, and \
             nothing beside them: a file written elsewhere in the tree is a stray the engine \
             would seal; remove each through `delete`:\n{}",
            lines.join("\n")
        )))
    }

    // The rules after this one read the syntax tree, so a module `syn`
    // refuses is reported here and read by none of them.
    fn unparsed(&self) -> Option<String> {
        let lines: Vec<String> = self
            .modules
            .iter()
            .filter_map(|module| {
                let unparsed = module.parsed.as_ref().err()?;
                Some(format!("{}:{}: {}", module.path, unparsed.line, unparsed.message))
            })
            .collect();
        unparsed_finding(&lines)
    }

    fn markers_left(&self, covered: &[String]) -> Option<String> {
        let mut lines = Vec::new();
        for module in &self.modules {
            for (number, marker) in module.markers() {
                for id in marker_ids(&marker) {
                    if covered.iter().any(|covered| covered == id) {
                        lines.push(format!(
                            "{}:{number}: {id} is covered and its marker stands",
                            module.path
                        ));
                    }
                }
            }
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a covered requirement is implemented, so no marker of its stands: fill each \
             `todo!(\"REQ-..\")` the adapter scaffolded for a covered requirement through \
             `edit_files` — a handler's body in place of its `let _ = (&input, &context);` line \
             and the marker, a function's body — or leave the requirement out of `covered`:\n{}",
            lines.join("\n")
        ))
    }

    // The test files are read here alone, since the tree's modules are a
    // stem's sources; one `syn` refuses is reported as a module is, its
    // scenarios unread.
    fn scenarios_tested(
        &self, scaffold: &Scaffold, covered: &[String],
    ) -> Result<Vec<String>, Error> {
        let expected: Vec<_> = scaffold
            .expected()
            .filter(|expected| covered.iter().any(|covered| covered == expected.id))
            .collect();
        let mut files: BTreeMap<&str, Result<syn::File, Unparsed>> = BTreeMap::new();
        for expected in &expected {
            if !files.contains_key(expected.path) {
                let file = self.root.join(expected.path);
                let text = if file.is_file() { read(&file)? } else { String::new() };
                files.insert(expected.path, syntax::parse(&text));
            }
        }

        let mut lines = Vec::new();
        for expected in &expected {
            let Some(Ok(file)) = files.get(expected.path) else { continue };
            let Some(found) = test_of(file, expected.test) else {
                lines.push(format!(
                    "{}: {} is covered and its scenario `{}` has no test `{}`",
                    expected.path, expected.id, expected.scenario, expected.test
                ));
                continue;
            };
            if found.ignored {
                lines.push(format!(
                    "{}:{}: {} is covered and the test `{}` of its scenario `{}` is ignored",
                    expected.path, found.line, expected.id, expected.test, expected.scenario
                ));
            }
            if let Some(line) = found.marker {
                lines.push(format!(
                    "{}:{line}: {} is covered and the test `{}` of its scenario `{}` holds its \
                     marker",
                    expected.path, expected.id, expected.test, expected.scenario
                ));
            }
        }

        let unparsed: Vec<String> = files
            .iter()
            .filter_map(|(path, parsed)| {
                let unparsed = parsed.as_ref().err()?;
                Some(format!("{path}:{}: {}", unparsed.line, unparsed.message))
            })
            .collect();
        let mut findings = Vec::new();
        findings.extend(unparsed_finding(&unparsed));
        if !lines.is_empty() {
            findings.push(format!(
                "a covered requirement's scenarios are tests that run and pass: each scenario of \
                 a covered requirement is the `#[tokio::test]` the adapter scaffolded for it in \
                 the part's test file, filled, with its `#[ignore = ..]` line removed; fill each \
                 through `edit_files`, or leave the requirement out of `covered`:\n{}",
                lines.join("\n")
            ));
        }
        Ok(findings)
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
                    "`{}` is a static file of the template the tree lacks; write it whole through \
                     `write_files`, as the template spells it:\n{}",
                    file.path,
                    template(file.path)?
                )
            } else if file.lines.len() > MAX_DIFF_LINES {
                format!(
                    "`{}` is a static file of the template and differs from it in {} lines; write \
                     it whole through `write_files`, as the template spells it:\n{}",
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
                "the tree lacks the root `{}`; write it whole through `write_files` as the \
                 template spells it, then add each stem's line to its two lists:\n{}",
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

struct Found {
    line: usize,
    ignored: bool,
    marker: Option<usize>,
}

// The test `name` declares in a test file: its line, whether an `#[ignore]`
// stands among its attributes, and the line of a marker left in its body.
fn test_of(file: &syn::File, name: &str) -> Option<Found> {
    struct Test<'n, 'ast> {
        name: &'n str,
        found: Option<&'ast ItemFn>,
    }

    impl<'ast> Visit<'ast> for Test<'_, 'ast> {
        fn visit_item_fn(&mut self, node: &'ast ItemFn) {
            if self.found.is_some() {
                return;
            }
            if node.sig.ident == self.name {
                self.found = Some(node);
            } else {
                visit::visit_item_fn(self, node);
            }
        }
    }

    let mut test = Test { name, found: None };
    test.visit_file(file);
    let item = test.found?;
    let mut markers = Markers::default();
    markers.visit_block(&item.block);
    Some(Found {
        line: syntax::line(item.sig.fn_token.span),
        ignored: item.attrs.iter().any(|attr| attr.path().is_ident("ignore")),
        marker: markers.found.first().map(|(line, _)| *line),
    })
}

// The requirement ids a marker names: its text up to the colon, one
// `REQ-<n> <subject>` per comma.
fn marker_ids(marker: &str) -> Vec<&str> {
    let named = marker.split(':').next().unwrap_or(marker);
    named
        .split(',')
        .filter_map(|entry| entry.split_whitespace().next())
        .filter(|id| {
            id.strip_prefix("REQ-").is_some_and(|digits| {
                !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
            })
        })
        .collect()
}

// The finding over the files `syn` refused, each `<path>:<line>: <message>`.
fn unparsed_finding(lines: &[String]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "a module that does not parse is read by no check and refused by the compiler; repair \
         its syntax through `edit_files`:\n{}",
        lines.join("\n")
    ))
}

pub fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|err| server_error!("reading `{}`: {err}", file.display()))
}

fn template(path: &str) -> Result<&'static str, Error> {
    omnia_template::entry(path)
        .map(|entry| entry.body)
        .ok_or_else(|| server_error!("the template holds no `{path}`"))
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
// `workspace = true` or a top-level `lints.workspace = true`.
fn inherits_lints(manifest: &str) -> bool {
    let mut table: Option<&str> = None;
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(header) = line.strip_prefix('[') {
            table = Some(header.strip_suffix(']').map_or("", str::trim));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        if value == "true"
            && (table == Some("lints") && key == "workspace"
                || table.is_none() && key == "lints.workspace")
        {
            return true;
        }
    }
    false
}

fn is_ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_ident_char)
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

pub fn is_start_stem(name: &str) -> bool {
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

// Every file of the tree, root-relative with `/` between segments, as a
// commit would carry it: nothing under `.git` or `.emery`, and nothing
// under the root's `target/`, which the checks fill.
pub fn listing(root: &Path) -> Result<BTreeSet<String>, Error> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if name == ".git" || name == ".emery" || (dir == root && name == "target") {
                continue;
            }
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_dir() {
                pending.push(path);
            } else {
                let relative: Vec<String> = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.insert(relative.join("/"));
            }
        }
    }
    Ok(found)
}

// Ignores empty directories left behind by `delete`.
pub fn crates(root: &Path) -> Result<Vec<String>, Error> {
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
