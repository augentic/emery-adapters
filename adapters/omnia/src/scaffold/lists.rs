//! Adds a stem's lines to the lists of the layout: where each list takes a
//! line, what each list holds, and the topic table's names.

use std::collections::BTreeMap;
use std::path::Path;

use emery_sdk::{Error, NoFollow, Standing, bad_request, server_error, standing};

use super::names::{Names, screaming};
use crate::layout::{self, PATH_LIST_ANCHOR, ROUTES_ARRAY_ANCHOR, TOPICS_ARRAY_ANCHOR};

// A line a stem adds to a list, and where the list stands with it.
pub struct Line {
    pub path: String,
    pub text: String,
    pub place: Place,
    pub placement: Placement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Held,
    Add,
    Unanchored,
}

// Where a list takes a line.
pub enum Place {
    // The root manifest's path list: after `common`'s line and the stems'
    // after it.
    PathList,
    // The root manifest's crate list, and the topic table: the end of the
    // file.
    FileEnd,
    // Before the line that starts with the anchor, as the root route and
    // topic lists take a statement.
    Before(&'static str),
    // Among the `pub mod` lines of a stem's entry, in the order the
    // formatter keeps them; before its first `use`, a blank line between,
    // where the entry holds none.
    PubModList,
    // At the end of the routes array of a stem's entry.
    RoutesArray,
    // At the end of the topics array of a stem's entry.
    TopicsArray,
}

// The text of each list a line is placed in, read once; a list the tree
// lacks is `None`.
#[derive(Default)]
pub struct Lists {
    texts: BTreeMap<String, Option<String>>,
}

// The topic table as the tree holds it, and the names the slice's topics
// take beside its constants.
pub struct Topics {
    declared: Vec<(String, String)>,
    names: Names,
}

impl Line {
    pub fn new(path: &str, text: String, place: Place) -> Self {
        Self {
            path: path.to_owned(),
            text,
            place,
            placement: Placement::Unanchored,
        }
    }

    // Where the list stands with the line: holding it, taking it at its
    // anchor, or lacking the anchor or the list.
    pub fn place(&mut self, root: &Path, lists: &mut Lists) -> Result<(), Error> {
        self.placement = match lists.text(root, &self.path)? {
            Some(text) if text.lines().any(|held| held.trim() == self.key()) => Placement::Held,
            Some(text) if insert(text, &self.text, &self.place).is_some() => Placement::Add,
            _ => Placement::Unanchored,
        };
        Ok(())
    }

    // The line that tells whether the file holds this one: the last of a
    // text of several, since a constant's doc line tells nothing.
    fn key(&self) -> &str {
        self.text.lines().last().unwrap_or(&self.text).trim()
    }
}

impl Lists {
    // The text of the list at `path`, read on the first ask.
    pub fn text(&mut self, root: &Path, path: &str) -> Result<Option<&str>, Error> {
        if !self.texts.contains_key(path) {
            let text = match standing(root, path) {
                Ok(Standing::File) => Some(layout::read(&root.join(path))?),
                Ok(Standing::Absent) => None,
                Err(NoFollow::Io { path, source }) => {
                    return Err(server_error!("reading `{path}`: {source}"));
                }
                Err(err) => {
                    return Err(bad_request!(
                        "`{path}` {err}, so the slice's line cannot be added to it"
                    ));
                }
            };
            self.texts.insert(path.to_owned(), text);
        }
        Ok(self.texts.get(path).and_then(Option::as_deref))
    }

    // Keeps the text a stem already read at `path`, so the list is read
    // once.
    pub fn cache(&mut self, path: &str, text: &str) {
        self.texts.entry(path.to_owned()).or_insert_with(|| Some(text.to_owned()));
    }

    // Adds `line` to its list where the list stands to take it, answering
    // whether it did.
    pub fn add(&mut self, line: &Line) -> bool {
        let Some(Some(text)) = self.texts.get_mut(&line.path) else { return false };
        let Some(next) = insert(text, &line.text, &line.place) else { return false };
        *text = next;
        true
    }

    // The text of the list at `path` as read and added to so far; none for
    // a list never read or one the tree lacks.
    pub fn get(&self, path: &str) -> Option<&str> {
        self.texts.get(path).and_then(Option::as_deref)
    }
}

impl Topics {
    // Every constant of the table reserves its name, and each holding a
    // string literal declares a topic.
    pub fn read(root: &Path) -> Result<Self, Error> {
        let table = layout::TopicTable::read(root)?;
        let taken: Vec<&str> = table.constants.iter().map(|held| held.name.as_str()).collect();
        Ok(Self {
            names: Names::taken(&taken),
            declared: table
                .constants
                .into_iter()
                .filter_map(|held| Some((held.name, held.value?)))
                .collect(),
        })
    }

    // The table's constant for `topic`, and whether the table declares it
    // already; a topic no constant spells is named for it, once.
    pub fn constant(&mut self, topic: &str) -> (String, bool) {
        if let Some((name, _)) = self.declared.iter().find(|(_, held)| held == topic) {
            return (name.clone(), true);
        }
        let name = self.names.reserve(&screaming(topic), "_TOPIC");
        self.declared.push((name.clone(), topic.to_owned()));
        (name, false)
    }
}

// `text` with `line` where `place` says, or none where the text holds no
// anchor for it.
pub fn insert(text: &str, line: &str, place: &Place) -> Option<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    let at = match place {
        Place::PathList => {
            let mut at = lines.iter().position(|held| *held == PATH_LIST_ANCHOR)? + 1;
            while lines.get(at).is_some_and(|held| held.contains(" = { path = \"crates/")) {
                at += 1;
            }
            at
        }
        Place::FileEnd => lines.len(),
        Place::Before(anchor) => {
            lines.iter().position(|held| held.trim_start().starts_with(anchor))?
        }
        Place::PubModList => {
            let is_mod = |held: &&str| held.trim_start().starts_with("pub mod ");
            if let (Some(first), Some(last)) =
                (lines.iter().position(is_mod), lines.iter().rposition(is_mod))
            {
                lines[first..=last]
                    .iter()
                    .position(|held| is_mod(held) && *held > line)
                    .map_or(last + 1, |offset| first + offset)
            } else {
                let at = lines.iter().position(|held| held.starts_with("use "))?;
                lines.insert(at, "");
                at
            }
        }
        Place::RoutesArray => array_end(&lines, ROUTES_ARRAY_ANCHOR)?,
        Place::TopicsArray => array_end(&lines, TOPICS_ARRAY_ANCHOR)?,
    };
    lines.insert(at, line);
    Some(lines.join("\n") + "\n")
}

// The index of the `])` closing the array `anchor` opens.
fn array_end(lines: &[&str], anchor: &str) -> Option<usize> {
    let opened = lines.iter().position(|held| held.trim() == anchor)? + 1;
    Some(opened + lines[opened..].iter().position(|held| held.trim() == "])")?)
}
