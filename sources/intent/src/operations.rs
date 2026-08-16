//! Intent bindings carry the operator's free-form brief — inline
//! (delivery `value:` bindings) or as a single-file locator the engine
//! materialized as a one-file tree — so both legs echo rather than infer.

use std::path::{Path, PathBuf};

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{
    Context, Error, Evidence, Lead, SourceContent, SourceInput, SourceMetadata, SurveyResult,
};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

/// Read the one regular file in the prepared tree as the intent string.
fn single_file_intent(root: &Path) -> Result<String, Error> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    match files.as_slice() {
        [file] => std::fs::read_to_string(file).map_err(|err| Error::Io(err.to_string())),
        _ => Err(Error::InvalidRequest(format!(
            "intent reads an inline `value:` binding or a single-file location; the \
             prepared tree holds {} files",
            files.len()
        ))),
    }
}

/// Collect every regular file beneath `dir` (the one-file tree encoding
/// may nest).
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir).map_err(|err| Error::Io(err.to_string()))? {
        let entry = entry.map_err(|err| Error::Io(err.to_string()))?;
        let file_type = entry.file_type().map_err(|err| Error::Io(err.to_string()))?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

/// Inline intent binding → one lead and one `kind: intent` claim.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Source for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "intent",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> SourceMetadata {
        SourceMetadata {
            emery_floor: Some("0.38.0".to_string()),
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<SurveyResult, Error> {
        let system = registry::body("prompts/survey.md").to_string();
        repaired(
            model,
            ctx,
            system,
            survey_user(ctx, input)?,
            "leads",
            LEADS_ANSWER_SCHEMA,
            leads_tail,
        )
        .await
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Evidence, Error> {
        let lead = terminal(input)?;
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract Evidence from the intent source bound to adapter `{id}` \
             (source key `{key}`) for this lead:\n\n{lead}\n\n\
             {content}\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority: \"intent\"`, one `kind: \"intent\"` claim whose `id` equals the \
             lead id and whose `statement` carries the operator's intent string verbatim, \
             per the prompt), without the envelope `lead` key — this call names the lead. \
             The caller persists the document; do not write it yourself.",
            id = ctx.adapter_id,
            key = input.key,
            lead = lead.render(),
            content = content_note(input)?,
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}

fn terminal(input: &SourceInput) -> Result<&Lead, Error> {
    input.focus.as_ref().ok_or_else(|| {
        Error::InvalidRequest("extract requires a terminal lead on input.focus".into())
    })
}

fn content_note(input: &SourceInput) -> Result<String, Error> {
    match &input.content {
        SourceContent::Value(value) => Ok(format!(
            "The bound material is this inline value; no `$SOURCE_DIR` is lent:\n\n{value}\n\n\
             The change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, \
             `leads.md`, or `slices/`."
        )),
        SourceContent::Workspace(view) => {
            let intent = single_file_intent(Path::new(&view.root))?;
            Ok(format!(
                "The bound material is a one-file tree at `{}`; the operator's intent \
                 string is:\n\n{intent}\n\n\
                 The change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, \
                 `leads.md`, or `slices/`.",
                view.root
            ))
        }
    }
}

fn survey_user(ctx: &Context<'_>, input: &SourceInput) -> Result<String, Error> {
    let content = content_note(input)?;
    Ok(input.focus.as_ref().map_or_else(
        || {
            format!(
                "Survey the intent source bound to adapter `{id}` (source key `{key}`).\n\n\
             {content}\n\n\
             This is an unfocused survey: emit the single current lead whose synopsis is \
             the operator's intent string, verbatim. Re-running replaces the prior lead \
             by its `(source, lead)` pair.\n\n\
             Answer with one JSON object matching the gated schema: a `leads` array \
             carrying exactly one lead (leave `children` empty). The caller persists \
             the catalog; do not write it yourself.",
                id = ctx.adapter_id,
                key = input.key,
            )
        },
        |parent| {
            format!(
                "Survey the intent source bound to adapter `{id}` (source key `{key}`).\n\n\
             {content}\n\n\
             This is a focused survey under the parent lead below. Inherit parent/focus \
             context from this record — do not look it up in `leads.md` or slice files. \
             Return stable child leads under this parent (intent is degenerate: typically \
             none).\n\n{parent}\n\n\
             Answer with one JSON object matching the gated schema: a `children` array \
             (leave `leads` empty). Stamp each child's `parent` and `focus` to the \
             focused lead id. The caller persists the catalog; do not write it yourself.",
                id = ctx.adapter_id,
                key = input.key,
                parent = parent.render(),
            )
        },
    ))
}
