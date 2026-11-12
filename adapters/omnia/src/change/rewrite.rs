//! Rewrites crates reached for the first time in a build and removes vanished
//! crates and their list lines before `Scaffold::plan` runs.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::{Error, NoFollow, Standing, bad_request, server_error, standing, write_whole};

use super::{Change, StemChange};
use crate::{layout, syntax};

impl Change {
    pub fn rewrite(&mut self, root: &Path) -> Result<(), Error> {
        let mut removals = Vec::new();
        for (stem, status) in &self.stems {
            let dir = layout::crate_dir(stem);
            match status {
                StemChange::Reached { first: true } | StemChange::Vanished => {
                    if let Some(removal) = Removal::plan(root, dir)? {
                        removals.push(removal);
                    }
                }
                StemChange::Reached { first: false } => {
                    let manifest = format!("crates/{dir}/Cargo.toml");
                    match standing(root, &manifest) {
                        Ok(_) => {}
                        Err(NoFollow::Io { path, source }) => {
                            return Err(server_error!("reading `{path}`: {source}"));
                        }
                        Err(err) => {
                            return Err(bad_request!(
                                "`{manifest}` {err}, where the slice's scaffold writes a file"
                            ));
                        }
                    }
                }
                StemChange::Kept => {}
            }
        }

        let vanished: Vec<String> = self
            .stems
            .iter()
            .filter(|(_, status)| **status == StemChange::Vanished)
            .map(|(stem, _)| stem.clone())
            .collect();
        let list_edits = self.plan_vanished_lists(root, &vanished)?;

        for (stem, status) in &self.stems {
            if matches!(status, StemChange::Reached { first: true }) {
                let dir = layout::crate_dir(stem);
                self.prior_exports.extend(syntax::crate_exported_fns(root, dir)?);
            }
        }

        for removal in removals {
            removal.apply(root)?;
        }
        for (path, next) in list_edits {
            write_whole(root, &path, Standing::File, next.as_bytes())
                .map_err(|err| server_error!("writing `{path}`: {err}"))?;
            self.rewritten_lists.insert(path, next);
        }
        Ok(())
    }

    fn plan_vanished_lists(
        &mut self, root: &Path, vanished: &[String],
    ) -> Result<Vec<(String, String)>, Error> {
        if vanished.is_empty() {
            return Ok(Vec::new());
        }
        let mut root_deps = BTreeSet::new();
        let mut handlers = BTreeSet::new();
        let mut stem_deps = BTreeSet::new();
        let mut stem_names = BTreeSet::new();
        for stem in vanished {
            let dir = layout::crate_dir(stem);
            let pkg = layout::package(stem);
            let ident = pkg.replace('-', "_");
            let root_dep = format!("{pkg} = {{ path = \"crates/{dir}\" }}");
            let call = format!("{ident}::handlers()");
            let call_comma = format!("{call},");
            let stem_dep = format!("{pkg} = {{ path = \"../{dir}\" }}");
            self.vanished_lines.insert(root_dep.clone());
            self.vanished_lines.insert(call.clone());
            self.vanished_lines.insert(call_comma);
            self.vanished_lines.insert(stem_dep.clone());
            root_deps.insert(root_dep);
            handlers.insert(call);
            stem_deps.insert(stem_dep);
            stem_names.insert(pkg);
            stem_names.insert(dir.to_owned());
        }

        let mut edits = Vec::new();
        if let Some(text) = read_file(root, "Cargo.toml")?
            && let Some(next) = drop_lines(&text, |line| root_deps.contains(line.trim()))
        {
            edits.push(("Cargo.toml".to_owned(), next));
        }
        if let Some(text) = read_file(root, layout::ENTRY)?
            && let Some(next) =
                drop_lines(&text, |line| handlers.contains(line.trim().trim_end_matches(',')))
        {
            edits.push((layout::ENTRY.to_owned(), next));
        }

        let removed_dirs: BTreeSet<&str> = self
            .stems
            .iter()
            .filter(|(_, status)| {
                matches!(status, StemChange::Reached { first: true } | StemChange::Vanished)
            })
            .map(|(stem, _)| layout::crate_dir(stem))
            .collect();
        for dir in layout::crates(root)? {
            if removed_dirs.contains(dir.as_str()) || !layout::is_stem_crate(&dir) {
                continue;
            }
            let path = format!("crates/{dir}/Cargo.toml");
            if let Some(text) = read_file(root, &path)?
                && let Some(next) = drop_lines(&text, |line| stem_deps.contains(line.trim()))
            {
                self.touch_points.retain(|point| {
                    !(point.file == path
                        && point
                            .item
                            .strip_prefix("dependency ")
                            .is_some_and(|name| stem_names.contains(name)))
                });
                edits.push((path, next));
            }
        }
        Ok(edits)
    }
}

fn read_file(root: &Path, path: &str) -> Result<Option<String>, Error> {
    match standing(root, path) {
        Ok(Standing::File) => Ok(Some(layout::read(&root.join(path))?)),
        Ok(Standing::Absent) => Ok(None),
        Err(NoFollow::Io { path, source }) => Err(server_error!("reading `{path}`: {source}")),
        Err(err) => Err(bad_request!("`{path}` {err}")),
    }
}

fn drop_lines(text: &str, mut drop: impl FnMut(&str) -> bool) -> Option<String> {
    let mut kept = Vec::new();
    let mut changed = false;
    for line in text.lines() {
        if drop(line) {
            changed = true;
        } else {
            kept.push(line);
        }
    }
    if !changed {
        return None;
    }
    let mut next = kept.join("\n");
    if text.ends_with('\n') && !next.is_empty() {
        next.push('\n');
    }
    Some(next)
}

struct Removal {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Removal {
    fn plan(root: &Path, dir: &str) -> Result<Option<Self>, Error> {
        let crate_rel = format!("crates/{dir}");
        if !require_dir(root, "crates", &crate_rel, true)? {
            return Ok(None);
        }
        if !require_dir(root, &crate_rel, &crate_rel, false)? {
            return Ok(None);
        }

        let target = root.join(&crate_rel);
        let mut files = Vec::new();
        let mut dirs = vec![target.clone()];
        let mut pending = vec![target];
        while let Some(current) = pending.pop() {
            let rel_dir = layout::relative(root, &current);
            let listing = fs::read_dir(&current)
                .map_err(|err| server_error!("listing `{rel_dir}`: {err}"))?;
            let mut entries = listing
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| server_error!("listing `{rel_dir}`: {err}"))?;
            entries.sort();
            for entry in entries {
                let rel = layout::relative(root, &entry);
                let meta = fs::symlink_metadata(&entry)
                    .map_err(|err| server_error!("reading `{rel}`: {err}"))?;
                let kind = meta.file_type();
                if kind.is_symlink() {
                    return Err(bad_request!("`{rel}` is a symlink"));
                } else if kind.is_dir() {
                    dirs.push(entry.clone());
                    pending.push(entry);
                } else if kind.is_file() {
                    files.push(entry);
                } else {
                    return Err(bad_request!("`{rel}` is not a regular file"));
                }
            }
        }
        files.sort();
        Ok(Some(Self { files, dirs }))
    }

    fn apply(self, root: &Path) -> Result<(), Error> {
        for file in self.files {
            let rel = layout::relative(root, &file);
            fs::remove_file(&file).map_err(|err| server_error!("removing `{rel}`: {err}"))?;
        }
        for dir in self.dirs.into_iter().rev() {
            let rel = layout::relative(root, &dir);
            fs::remove_dir(&dir).map_err(|err| server_error!("removing `{rel}`: {err}"))?;
        }
        Ok(())
    }
}

fn require_dir(root: &Path, rel: &str, subject: &str, ancestor: bool) -> Result<bool, Error> {
    match fs::symlink_metadata(root.join(rel)) {
        Ok(meta) => {
            let kind = meta.file_type();
            if kind.is_symlink() {
                if ancestor {
                    return Err(bad_request!("`{subject}` leads through the symlink `{rel}`"));
                }
                return Err(bad_request!("`{subject}` is a symlink"));
            }
            if !kind.is_dir() {
                if ancestor {
                    return Err(bad_request!(
                        "`{subject}` leads through `{rel}`, which is not a directory"
                    ));
                }
                return Err(bad_request!("`{subject}` is not a directory"));
            }
            Ok(true)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(server_error!("reading `{rel}`: {err}")),
    }
}
