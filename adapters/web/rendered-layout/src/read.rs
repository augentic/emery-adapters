//! Reads the lent facts folder into placement claims and one whole layout.

use std::path::Path;

use emery_sdk::{
    Claim, ClaimKind, Error, Evidence, Layout, LayoutView, SourceContent, SourceInput, bad_request,
    server_error,
};
use web_rendered_layout::{
    BUNDLE_MEDIA_TYPE, Facts, FactsFolder, REVIEW_WIDTHS, Role, draw, outline,
};

pub fn evidence(input: &SourceInput) -> Result<Evidence, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Err(bad_request!(
            "`{}`: rendered-layout reads a facts folder, not an inline value",
            input.name
        ));
    };
    let folder = FactsFolder::read(Path::new(root))
        .map_err(|err| bad_request!("`{}`: {err}", input.name))?;
    let facts = Facts::read(&folder).map_err(|err| {
        bad_request!("`{}`: the tree is not rendered-layout facts: {err}", input.name)
    })?;

    let claims = outline(&facts)
        .iter()
        .map(|placement| {
            let kind = match placement.role {
                Role::Region => ClaimKind::Region,
                Role::Container => ClaimKind::Container,
                Role::Leaf => ClaimKind::Leaf,
            };
            Claim::new(kind).at(placement.file.as_str()).with_synopsis(placement.synopsis())
        })
        .collect();
    let views = REVIEW_WIDTHS
        .into_iter()
        .map(|width| LayoutView {
            label: format!("{width} wide"),
            media_type: "image/svg+xml".to_owned(),
            body: draw(&facts, width),
        })
        .collect();
    let layout = Layout {
        media_type: BUNDLE_MEDIA_TYPE.to_owned(),
        body: folder.to_bundle().map_err(|err| server_error!("`{}`: {err}", input.name))?,
        views,
    };
    Ok(Evidence {
        claims,
        layouts: vec![layout],
    })
}
