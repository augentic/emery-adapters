//! The check that every requirement's function is reached from a function
//! the host runs.

use std::collections::{BTreeMap, BTreeSet};

use syn::Item;

use super::{Module, Tree};
use crate::layout;
use crate::scaffold::Scaffold;
use crate::syntax::{self, Mentions};

// One top-level function of a stem's crate: where it stands, the covered
// requirements it is laid under, and the names its body mentions.
struct Function {
    path: String,
    line: usize,
    ids: Vec<String>,
    stub: bool,
    mentions: Mentions,
}

impl Tree<'_> {
    // A requirement's function that only its test calls is a behaviour the
    // program never performs, whatever the test proves: the host runs a
    // stem through the handlers and consumers its `handlers()` registers
    // alone. Held at the part's last slice, since an earlier slice may
    // leave the wiring to the one that builds the consumer; a function
    // another crate mentions is an export and reached; a function reached
    // through another unreached one is listed under it alone.
    pub fn unreached_functions(&self, scaffold: &Scaffold, covered: &[String]) -> Option<String> {
        if !scaffold.last() {
            return None;
        }
        let mut lines = Vec::new();
        for stem in scaffold.stems() {
            let prefix = format!("crates/{}/src/", layout::crate_dir(stem));
            let own: Vec<&Module> =
                self.modules.iter().filter(|module| module.path.starts_with(&prefix)).collect();
            let roots: BTreeSet<String> = own
                .iter()
                .filter_map(|module| module.file())
                .flat_map(syntax::registered_fns)
                .collect();
            if roots.is_empty() {
                continue;
            }
            let functions = functions(&own, covered);
            let reached = reached(&functions, roots, |name| {
                self.modules
                    .iter()
                    .filter(|module| !module.path.starts_with(&prefix))
                    .any(|module| module.mentions.contains(name))
            });
            let candidates: Vec<&str> = functions
                .iter()
                .filter(|(name, function)| {
                    !function.ids.is_empty() && !function.stub && !reached.contains(*name)
                })
                .map(|(name, _)| name.as_str())
                .collect();
            let mut heads: Vec<(&str, &Function)> = candidates
                .iter()
                .filter(|name| {
                    !candidates.iter().any(|other| {
                        other != *name && closure(&functions, (*other).to_owned()).contains(**name)
                    })
                })
                .map(|name| (*name, &functions[*name]))
                .collect();
            if heads.is_empty() {
                heads = candidates.iter().map(|name| (*name, &functions[*name])).collect();
            }
            heads.sort_by(|(_, a), (_, b)| (&a.path, a.line).cmp(&(&b.path, b.line)));
            for (name, function) in heads {
                lines.push(format!(
                    "{}:{}: `{name}` ({}) is reached by no handler or consumer `{stem}` registers",
                    function.path,
                    function.line,
                    function.ids.join(", ")
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a requirement's function is reached from a function the host runs, the registered \
             handler or consumer, or the program never performs the behaviour, whatever its \
             test proves: call each function named below from the consumer's or handler's \
             flow where the behaviour belongs, in requirement order (a rejection before what \
             it guards, a filter before the events it filters, the publish last), through \
             `edit_files`, or leave its requirement out of `covered`; a function reached \
             through one named here is not listed:\n{}",
            lines.join("\n")
        ))
    }
}

// Every top-level function of `modules` by name, each with the covered
// requirement ids of the comment run it stands under.
fn functions(modules: &[&Module], covered: &[String]) -> BTreeMap<String, Function> {
    let mut functions = BTreeMap::new();
    for module in modules {
        let Some(file) = module.file() else { continue };
        let runs = module.marker_runs();
        let mut starts: Vec<usize> = file
            .items
            .iter()
            .filter_map(|item| syntax::item_head(item).map(|(line, _)| line))
            .collect();
        starts.sort_unstable();
        for item in &file.items {
            let Item::Fn(item) = item else { continue };
            let line = syntax::line(item.sig.fn_token.span);
            let ids = runs
                .iter()
                .filter(|(from, ids)| {
                    let to = from + ids.len() - 1;
                    starts.iter().copied().find(|start| *start > to) == Some(line)
                })
                .flat_map(|(_, ids)| ids.iter())
                .filter(|id| covered.contains(id))
                .cloned()
                .collect();
            functions.insert(
                item.sig.ident.to_string(),
                Function {
                    path: module.path.clone(),
                    line,
                    ids,
                    stub: syntax::holds_todo(&item.block),
                    mentions: Mentions::of_block(&item.block),
                },
            );
        }
    }
    functions
}

// The functions reached from `roots` through the names each body
// mentions, and every function `exported` says another crate mentions.
fn reached(
    functions: &BTreeMap<String, Function>, roots: BTreeSet<String>,
    exported: impl Fn(&str) -> bool,
) -> BTreeSet<String> {
    let mut reached = roots;
    reached.extend(functions.keys().filter(|name| exported(name)).cloned());
    close(functions, &mut reached);
    reached
}

// The functions the body of `name` reaches through the names each body
// mentions, `name` among them.
fn closure(functions: &BTreeMap<String, Function>, name: String) -> BTreeSet<String> {
    let mut reached = BTreeSet::from([name]);
    close(functions, &mut reached);
    reached
}

// Every function a body in `reached` mentions, added until none is.
fn close(functions: &BTreeMap<String, Function>, reached: &mut BTreeSet<String>) {
    let mut pending: Vec<String> = reached.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        let Some(function) = functions.get(&name) else { continue };
        for other in functions.keys() {
            if !reached.contains(other) && function.mentions.contains(other) {
                reached.insert(other.clone());
                pending.push(other.clone());
            }
        }
    }
}
