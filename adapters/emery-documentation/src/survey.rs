//! Divides a documentation tree into independently mined seams: by the
//! subjects the model names in it, from the outline code read of every
//! document, or by its directories where it names none.

mod model;
mod structure;
mod subjects;

use std::collections::BTreeMap;

use emery_sdk::{Context, Error, Model, Seam, SourceContent, SourceInput};

use self::subjects::Preparation;
use crate::PROSE;

pub async fn survey<P: Model>(ctx: &Context<'_, P>) -> Result<Vec<Seam>, Error> {
    match subjects::prepare(ctx.input)? {
        Preparation::Value => Ok(vec![Seam::whole()]),
        Preparation::Workspace(prepared) if prepared.documents.is_empty() => {
            directories(prepared.input)
        }
        Preparation::Workspace(prepared) => {
            let inventory = model::subjects(ctx, PROSE, &prepared).await?;
            subjects::seams(&prepared, &inventory)
        }
    }
}

const MIN_SIZE: usize = 2;

// Over this, a directory is partitioned one level deeper, so it cannot hold a
// run behind one turn.
const MAX_SIZE: usize = 16;

// The directory cut, where no subject leads the seams.
fn directories(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(vec![Seam::whole()]);
    };

    let files = emery_sdk::workspace::list(root, |entry| !entry.hidden())?;
    let seams: Vec<Seam> = partition("", files).into_iter().map(Seam::files).collect();
    Ok(if seams.len() < 2 { vec![Seam::whole()] } else { seams })
}

// `prefix` is `""` at the root and ends in `/` beneath it.
fn partition(prefix: &str, files: Vec<String>) -> Vec<Vec<String>> {
    // bucket by the next path segment
    let mut groups: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for file in files {
        let segment = file
            .strip_prefix(prefix)
            .and_then(|rest| rest.split_once('/'))
            .map(|(segment, _)| format!("{prefix}{segment}/"));
        groups.entry(segment).or_default().push(file);
    }

    // fold segments too small to stand alone into the directory's own group
    let folded: Vec<String> = groups
        .extract_if(.., |segment, files| segment.is_some() && files.len() < MIN_SIZE)
        .flat_map(|(_, files)| files)
        .collect();
    if !folded.is_empty() {
        groups.entry(None).or_default().extend(folded);
    }

    // an own group too small to stand alone joins the first segment's
    if groups.len() > 1 && groups.get(&None).is_some_and(|own| own.len() < MIN_SIZE) {
        let own = groups.remove(&None).unwrap_or_default();
        if let Some(first) = groups.values_mut().next() {
            first.extend(own);
        }
    }

    // partition a top-level directory over the cap once more
    groups
        .into_iter()
        .flat_map(|(segment, files)| match segment {
            Some(segment) if prefix.is_empty() && files.len() > MAX_SIZE => {
                partition(&segment, files)
            }
            _ => vec![files],
        })
        .collect()
}
