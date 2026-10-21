//! A ui-ir design-system directory to the Vue app's files.
//!
//! Validate, parse, derive the copy and event contracts, refuse a contract no generated file could
//! satisfy, then render the views and the theme. Nothing is written here; [`crate::write()`] and
//! [`crate::drift`] reconcile the files with a workspace, and the caller copies [`Emit::assets`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use serde_json::Value;

use crate::copy::Contract;
use crate::tokens::Theme;
use crate::view::vue::Glyph;
use crate::{Error, GeneratedFile, contract, model, spec, theme, view};

/// What an emit produces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Emit {
    /// Every generated file, relative to the app root.
    pub files: Vec<GeneratedFile>,

    /// Each asset's name to its web source, relative to the design-system directory; the caller
    /// copies each to the same path under [`crate::PUBLIC`].
    pub assets: BTreeMap<String, String>,

    /// Assets that have no web source, so the Vue app cannot serve them.
    pub unserved: Vec<String>,
}

/// Renders the Vue app a ui-ir design-system directory implies.
///
/// `allow_holes` emits over `TODO(...)` markers, which is only ever useful for looking at partial
/// output.
///
/// # Errors
///
/// When a document cannot be read, the design system does not validate or still has holes (unless
/// allowed), or the contract it implies cannot be satisfied: an event argument nothing provides, an
/// event raised two ways, or a field bound as two kinds of value.
pub fn vue(dir: &Path, allow_holes: bool) -> Result<Emit, Error> {
    let report = spec::validate(dir)?;
    if !report.is_clean() {
        return Err(Error::Malformed(format!(
            "{} does not validate:\n{}",
            dir.display(),
            report.render()
        )));
    }
    if !report.is_filled() && !allow_holes {
        let holes: Vec<String> =
            report.holes.iter().map(|hole| format!("- {}#{}", hole.file, hole.path)).collect();
        return Err(Error::Malformed(format!(
            "{} still has holes in it:\n{}",
            dir.display(),
            holes.join("\n")
        )));
    }

    let surface = model::parse(&crate::read(&dir.join(spec::COMPOSITION))?)?;
    let tokens = dir.join(spec::TOKENS);
    let theme = Theme::parse(&fs::read_to_string(&tokens).map_err(|source| Error::Io {
        path: tokens.clone(),
        source,
    })?)?;
    let copy = Contract::derive(&surface);
    let contract = contract::derive(&surface);
    refuse_an_uncompilable_contract(&contract, dir)?;

    let inventory = optional(&dir.join(spec::ASSETS))?;
    let (assets, unserved) = web_assets(inventory.as_ref())?;
    let inline = inline_icons(dir, inventory.as_ref(), &assets);
    let glyphs = glyph_assets(inventory.as_ref());

    let mut files = view::vue::files(&surface, &copy, &contract, &assets, &inline, &glyphs);
    files.extend(theme::vue::files(&theme));
    Ok(Emit {
        files,
        assets,
        unserved,
    })
}

/// Stops before anything is rendered for the three things a composition can say that no
/// generated file could then satisfy. Reported one kind at a time, since the second is usually a
/// consequence of fixing the first.
fn refuse_an_uncompilable_contract(contract: &contract::Contract, dir: &Path) -> Result<(), Error> {
    let listed = |lines: Vec<String>| lines.join("\n");
    let refuse = |what: &str, lines: Vec<String>| {
        Err(Error::Malformed(format!("{} {what}:\n{}", dir.display(), listed(lines))))
    };
    if !contract.gaps.is_empty() {
        return refuse(
            "raises events carrying values nothing provides",
            contract.gaps.iter().map(|gap| format!("  {gap}")).collect(),
        );
    }
    if !contract.clashes.is_empty() {
        return refuse(
            "raises one event two ways",
            contract.clashes.iter().map(|clash| format!("  {clash}")).collect(),
        );
    }
    if !contract.conflicts.is_empty() {
        return refuse(
            "binds one field as two kinds of value",
            contract.conflicts.iter().map(|conflict| format!("  {conflict}")).collect(),
        );
    }
    Ok(())
}

fn optional(path: &Path) -> Result<Option<Value>, Error> {
    if path.is_file() { crate::read(path).map(Some) } else { Ok(None) }
}

fn entries(inventory: Option<&Value>) -> impl Iterator<Item = (&String, &Value)> {
    inventory
        .and_then(|document| document.get("assets"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
}

/// Each asset's name to its `sources.web` path, and the names that have none.
///
/// A path that leaves the design-system directory is refused rather than served: the caller reads
/// from it and writes under the app, and neither should reach outside its tree.
fn web_assets(inventory: Option<&Value>) -> Result<(BTreeMap<String, String>, Vec<String>), Error> {
    let mut served = BTreeMap::new();
    let mut unserved = Vec::new();
    for (name, entry) in entries(inventory) {
        let Some(web) = entry.pointer("/sources/web").and_then(Value::as_str) else {
            unserved.push(name.clone());
            continue;
        };
        let relative = Path::new(web);
        if relative.is_absolute()
            || relative.components().any(|part| matches!(part, Component::ParentDir))
        {
            return Err(Error::Malformed(format!(
                "asset `{name}`'s web source `{web}` leaves the design-system directory"
            )));
        }
        served.insert(name.clone(), web.to_owned());
    }
    Ok((served, unserved))
}

/// Each vector icon asset's SVG markup, by name, for the views to draw inline (so the icon takes
/// its item's colour through `currentColor`). An unreadable file is left to the `<img>` fallback.
fn inline_icons(
    dir: &Path, inventory: Option<&Value>, served: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    entries(inventory)
        .filter(|(_, entry)| entry["kind"] == "vector" && entry["role"] == "icon")
        .filter_map(|(name, _)| {
            let markup = fs::read_to_string(dir.join(served.get(name)?)).ok()?;
            markup.trim_start().starts_with("<svg").then(|| (name.clone(), markup))
        })
        .collect()
}

/// Each glyph asset (`kind: glyph`) by name: the text and the font family it is drawn in.
fn glyph_assets(inventory: Option<&Value>) -> BTreeMap<String, Glyph> {
    entries(inventory)
        .filter(|(_, entry)| entry["kind"] == "glyph")
        .filter_map(|(name, entry)| {
            let glyph = &entry["glyph"];
            Some((
                name.clone(),
                Glyph {
                    text: glyph["text"].as_str()?.to_owned(),
                    family: glyph["family"].as_str()?.to_owned(),
                    weight: glyph["weight"].as_u64().and_then(|w| u32::try_from(w).ok()),
                },
            ))
        })
        .collect()
}
