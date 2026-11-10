//! Adds a stem's lines to the lists of the layout: where each list takes a
//! line, what each list holds, and the topic table's names.

use std::collections::BTreeMap;
use std::path::Path;

use emery_sdk::{Error, NoFollow, Standing, bad_request, server_error, standing};

use super::names::{Names, screaming};
use super::render::{MAX_WIDTH, TAB, pad};
use crate::layout::{self, insert_after};

// A line a stem adds to a list, and where the list stands with it.
pub struct Line {
    pub path: String,
    pub text: String,
    pub place: Place,
    pub placement: Placement,
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
            Some(text) if self.held_by(text) => Placement::Held,
            Some(text) if insert(text, &self.text, &self.place).is_some() => Placement::Add,
            _ => Placement::Unanchored,
        };
        Ok(())
    }

    // Whether the list holds the line already: as a line of its own, or,
    // for an array's entry, anywhere in the text, since the formatter lays
    // a short array on the anchor's own line.
    fn held_by(&self, text: &str) -> bool {
        let key = self.key();
        text.lines().any(|held| held.trim() == key)
            || matches!(self.place, Place::After(anchor) if anchor.ends_with('['))
                && text.contains(key.trim_end_matches(','))
    }

    // The line that tells whether the file holds this one: the last of a
    // text of several, since a constant's doc line tells nothing.
    fn key(&self) -> &str {
        self.text.lines().last().unwrap_or(&self.text).trim()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Held,
    Add,
    Unanchored,
}

// Where a list takes a line.
pub enum Place {
    // After the template's anchor, as the template's own insertion lays it:
    // on the line after the anchor's, or, for an anchor opening an array,
    // as the array's last entry.
    After(&'static str),
    // The topic table: the end of the file.
    FileEnd,
    // Among the `pub mod` lines of a stem's entry, in the order the
    // formatter keeps them; before its first `use`, a blank line between,
    // where the entry holds none.
    PubModList,
}

// The text of each list a line is placed in, read once; a list the tree
// lacks is `None`.
#[derive(Default)]
pub struct Lists {
    texts: BTreeMap<String, Option<String>>,
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
        if !self.texts.contains_key(path) {
            self.texts.insert(path.to_owned(), Some(text.to_owned()));
        }
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

// The topic table as the tree holds it, and the names the slice's topics
// take beside its constants.
pub struct Topics {
    declared: Vec<(String, String)>,
    names: Names,
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

    // Whether the table, or a slice adding to it, names a constant `name`.
    pub fn is_named(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

// `text` with `line` where `place` says, or none where the text holds no
// anchor for it.
pub fn insert(text: &str, line: &str, place: &Place) -> Option<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    let at = match place {
        Place::After(anchor) => {
            let text = insert_after(text, anchor, line)?;
            return if anchor.ends_with('[') { laid(&text, anchor) } else { Some(text) };
        }
        Place::FileEnd => lines.len(),
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
    };
    lines.insert(at, line);
    Some(lines.join("\n") + "\n")
}

// `text` with the array `anchor` opens laid as the formatter lays it: on
// the anchor's line where that fits the width, else one entry per line
// under it, a call chained on the array moved under its closing bracket.
// An array holding a comment stands as it is, since the formatter keeps
// one vertical.
fn laid(text: &str, anchor: &str) -> Option<String> {
    let (before, after) = text.split_once(anchor)?;
    let (inner, rest) = after.split_once(']')?;
    if inner.contains("//") {
        return Some(text.to_owned());
    }
    let entries: Vec<&str> =
        inner.split(',').map(str::trim).filter(|entry| !entry.is_empty()).collect();
    let line_start = before.rfind('\n').map_or(0, |at| at + 1);
    let lead = &before[line_start..];
    let indent: String = lead.chars().take_while(|ch| ch.is_whitespace()).collect();
    let (tail, remainder) =
        rest.split_once('\n').map_or((rest, None), |(tail, rest)| (tail, Some(rest)));
    let joined = format!("{lead}{anchor}{}]{tail}", entries.join(", "));
    let array = if entries.is_empty() || joined.chars().count() <= MAX_WIDTH {
        joined
    } else {
        let mut lines = vec![format!("{lead}{anchor}")];
        lines.extend(entries.iter().map(|entry| format!("{indent}{}{entry},", pad(TAB))));
        let (close, chain) =
            tail.split_once('.').map_or((tail, None), |(close, chain)| (close, Some(chain)));
        lines.push(format!("{indent}]{close}"));
        lines.extend(chain.map(|chain| format!("{indent}.{chain}")));
        lines.join("\n")
    };
    let mut out = before[..line_start].to_owned();
    out.push_str(&array);
    if let Some(remainder) = remainder {
        out.push('\n');
        out.push_str(remainder);
    }
    Some(out)
}
