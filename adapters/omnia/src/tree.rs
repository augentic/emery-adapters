//! Holds the lent tree to the layout's rules before a report or verdict is accepted.

mod lists;
mod misses;
mod process;

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use emery_sdk::Error;
use emery_sdk::target::Report;
use omnia_template::Manifest;
use syn::visit::{self, Visit};
use syn::{ItemFn, Macro};

use crate::layout::{self, Toml, TopicTable};
use crate::scaffold::Scaffold;
use crate::syntax::{self, Mentions, Unparsed};

const MAX_DIFF_LINES: usize = 24;

// One Rust file of the tree: its text, what `syn` made of it, and the names
// it mentions, none where `syn` refused it.
struct Module {
    path: String,
    text: String,
    parsed: Result<syn::File, Unparsed>,
    mentions: Mentions,
}

impl Module {
    fn read(root: &Path, file: &Path) -> Result<Self, Error> {
        let path = layout::relative(root, file);
        let text = layout::read(file)?;
        let parsed = syntax::parse(&text);
        let mentions = parsed.as_ref().ok().map(Mentions::of).unwrap_or_default();
        Ok(Self {
            path,
            text,
            parsed,
            mentions,
        })
    }

    fn is_common(&self) -> bool {
        self.path.starts_with("crates/common/")
    }

    fn file(&self) -> Option<&syn::File> {
        self.parsed.as_ref().ok()
    }

    // The line `number` stands on, trimmed and without its trailing comma.
    fn line(&self, number: usize) -> String {
        number
            .checked_sub(1)
            .and_then(|index| self.text.lines().nth(index))
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

// The Rust files of a tree, read once: the stems' sources, the stems' tests,
// and the root lists the tree holds.
pub struct Tree<'a> {
    root: &'a Path,
    modules: Vec<Module>,
    tests: Vec<Module>,
    lists: Vec<Module>,
}

impl<'a> Tree<'a> {
    pub fn read(root: &'a Path) -> Result<Self, Error> {
        let mut modules = Vec::new();
        let mut tests = Vec::new();
        for stem in layout::stem_dirs(root)? {
            for file in layout::sources(&stem.join("src"))? {
                modules.push(Module::read(root, &file)?);
            }
            for file in layout::sources(&stem.join("tests"))? {
                tests.push(Module::read(root, &file)?);
            }
        }
        let mut lists = Vec::new();
        for path in [layout::ROUTES, layout::MESSAGING] {
            let file = root.join(path);
            if file.is_file() {
                lists.push(Module::read(root, &file)?);
            }
        }
        Ok(Self {
            root,
            modules,
            tests,
            lists,
        })
    }

    pub fn check_build(&self, scaffold: &Scaffold, report: &Report) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.split_stems(scaffold)?);
        findings.extend(self.strays(scaffold)?);
        findings.extend(self.markers_left(&report.covered));
        findings.extend(self.untested_scenarios(scaffold, &report.covered));
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
        findings.extend(self.dangling_entries()?);
        findings.extend(self.simulated_process()?);
        findings.extend(self.unlinted_stems()?);
        findings.extend(self.unused_dependencies()?);
        findings.extend(self.unmerged_stems());
        findings.extend(self.undeclared_parts());
        findings.extend(self.unnamed_topics());
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
        Ok(layout::crates(self.root)?
            .into_iter()
            .filter(|dir| {
                !(dir == "common"
                    || layout::is_start_stem(dir)
                    || scaffold.existing().contains(dir)
                    || stems.iter().any(|stem| layout::crate_dir(stem) == dir))
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
                    format!(
                        "`crates/{dir}/` is written for `{dir}`, which is no stem of this slice \
                         ({})",
                        stems.iter().map(|stem| format!("`{stem}`")).collect::<Vec<_>>().join(", ")
                    )
                },
                |stem| {
                    let stem_dir = layout::crate_dir(stem);
                    let pkg = layout::package(stem);
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
        Ok(Some(format!(
            "a slice writes modules under the crates of its own stems alone, and a stem is one \
             word and is never split at its hyphen into a crate and a part: write each module \
             under `crates/<stem>/src/<part>.rs` for the stem its requirements lead with, remove \
             every file under the other directory through `delete`, and drop its lines from the \
             root `Cargo.toml` through `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    // A file the turn added outside the slice's crates, beyond the files the
    // tree held once the skeleton was written: a crate `split_stems` or the
    // process rule names is that finding's, so one problem is one finding.
    fn strays(&self, scaffold: &Scaffold) -> Result<Option<String>, Error> {
        let mut own: Vec<String> =
            scaffold.stems().map(|stem| format!("crates/{}/", layout::crate_dir(stem))).collect();
        own.extend(self.foreign_crates(scaffold)?.into_iter().map(|dir| format!("crates/{dir}/")));
        own.extend(
            layout::crates(self.root)?
                .into_iter()
                .filter(|dir| layout::is_start_stem(dir))
                .map(|dir| format!("crates/{dir}/")),
        );
        let lines: Vec<String> = layout::listing(self.root)?
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

    // The rules after this one read the syntax tree, so a file `syn`
    // refuses is reported here and read by none of them.
    fn unparsed(&self) -> Option<String> {
        let lines: Vec<String> = self
            .files()
            .filter_map(|module| {
                let unparsed = module.parsed.as_ref().err()?;
                Some(format!("{}:{}: {}", module.path, unparsed.line, unparsed.message))
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a module that does not parse is read by no check and refused by the compiler; repair \
             its syntax through `edit_files`:\n{}",
            lines.join("\n")
        ))
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

    // A test file `syn` refuses is `unparsed`'s finding, its scenarios
    // unread here.
    fn untested_scenarios(&self, scaffold: &Scaffold, covered: &[String]) -> Option<String> {
        let mut lines = Vec::new();
        for expected in scaffold
            .expected()
            .filter(|expected| covered.iter().any(|covered| covered == expected.id))
        {
            let test = match self.module(expected.path) {
                Some(module) => match module.file() {
                    Some(file) => test_of(file, expected.test),
                    None => continue,
                },
                None => None,
            };
            let Some(found) = test else {
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
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a covered requirement's scenarios are tests that run and pass: each scenario of a \
             covered requirement is the `#[tokio::test]` the adapter scaffolded for it in the \
             part's test file, filled, with its `#[ignore = ..]` line removed; fill each through \
             `edit_files`, or leave the requirement out of `covered`:\n{}",
            lines.join("\n")
        ))
    }

    // The workspace's lints are the host boundary, and they reach a crate
    // through its `[lints]` table alone.
    fn unlinted_stems(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for dir in layout::crates(self.root)? {
            if dir == "common" || layout::is_start_stem(&dir) {
                continue;
            }
            let path = format!("crates/{dir}/Cargo.toml");
            let file = self.root.join(&path);
            if file.is_file() && !Toml::read(&layout::read(&file)?).inherits_workspace_lints() {
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
        let drift = omnia_template::drift(self.root).map_err(|err| layout::refusal(&err))?;
        let mut findings = Vec::new();
        for file in drift.files() {
            let finding = if self.lacks(file.path) {
                format!(
                    "`{}` is a static file of the template the tree lacks; write it whole through \
                     `write_files`, as the template spells it:\n{}",
                    file.path,
                    layout::template(file.path)?
                )
            } else if file.lines.len() > MAX_DIFF_LINES {
                format!(
                    "`{}` is a static file of the template and differs from it in {} lines; write \
                     it whole through `write_files`, as the template spells it:\n{}",
                    file.path,
                    file.lines.len(),
                    layout::template(file.path)?
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
        let drift = Manifest::new(self.root).drift().map_err(|err| layout::refusal(&err))?;
        if drift.is_empty() {
            return Ok(None);
        }
        let finding = if self.lacks(Manifest::PATH) {
            format!(
                "the tree lacks the root `{}`; write it whole through `write_files` as the \
                 template spells it, then add each stem's line to its two lists:\n{}",
                Manifest::PATH,
                layout::template(Manifest::PATH)?
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

    fn files(&self) -> impl Iterator<Item = &Module> {
        self.modules.iter().chain(&self.tests).chain(&self.lists)
    }

    fn module(&self, path: &str) -> Option<&Module> {
        self.files().find(|module| module.path == path)
    }

    // The topic table as the tree holds it; empty where the tree lacks it
    // or `syn` refused it, which `unparsed` reports.
    fn topics(&self) -> TopicTable {
        self.module(layout::TOPICS).and_then(Module::file).map(TopicTable::of).unwrap_or_default()
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
