//! The judgment operation template: `guidance`, `build`, and `merge`.
//!
//! Each judgment leg is bracketed by deterministic guest code. The core
//! assembles a prompt from the embedded brief plus the typed inputs,
//! issues a single-shot `create` through the [`Model`] capability with a
//! schema-gated `format`, deserializes the answer into the WIT-shaped
//! records, and then runs the validate-before-visible checks (the
//! absorbed contract validators) after the answer lands. All state
//! between calls lives in the workspace tree — the session-less shape:
//! `build` decomposes into the three format sub-flows (json-schema,
//! openapi, asyncapi) as independent `create` calls, then a bounded
//! verify-repair loop, then one report call.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use specify_guest_kit::{Format, McpGrant, Message, Model, Request, Role, SchemaFormat};

use crate::registry;
use crate::report::{Finding, REPORT_ANSWER_SCHEMA, Report, ReportAnswer, Status};
use crate::validate::{ContractFinding, validate_baseline};

/// The error returned by operations — mirrors the WIT `types.error` variant.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The request itself is malformed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// A filesystem operation failed.
    #[error("io: {0}")]
    Io(String),
    /// A judgment call or answer-handling step failed.
    #[error("internal: {0}")]
    Internal(String),
}

impl From<specify_guest_kit::Error> for Error {
    fn from(err: specify_guest_kit::Error) -> Self {
        match err {
            specify_guest_kit::Error::InvalidRequest(detail) => Self::InvalidRequest(detail),
            other => Self::Internal(other.to_string()),
        }
    }
}

/// One slice-artifact input — mirrors the WIT `target.input` variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// The slice's `proposal.md`.
    Proposal(String),
    /// The slice's `design.md`.
    Design(String),
    /// The slice's `tasks.md`.
    Tasks(String),
    /// One behavioural spec (`specs/<domain>/spec.md`).
    Spec(String),
    /// Any additional artifact.
    Other(String),
}

impl Input {
    const fn label(&self) -> &'static str {
        match self {
            Self::Proposal(_) => "proposal",
            Self::Design(_) => "design",
            Self::Tasks(_) => "tasks",
            Self::Spec(_) => "spec",
            Self::Other(_) => "other",
        }
    }

    const fn body(&self) -> &String {
        match self {
            Self::Proposal(body)
            | Self::Design(body)
            | Self::Tasks(body)
            | Self::Spec(body)
            | Self::Other(body) => body,
        }
    }
}

/// Names the tree an operation works on — mirrors the WIT `working-tree`
/// record. The adapter opens its own `"."` preopen; `subpath` scopes the
/// operation beneath the shared mount root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingTree {
    /// The snapshot the operation applies against.
    pub base: String,
    /// Optional path beneath the shared mount root.
    pub subpath: Option<String>,
}

/// One path-scoped edit — mirrors the WIT `types.edit` record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    /// The edited file's path, relative to the working tree root.
    pub path: String,
    /// The new content handle, or absent for a deletion.
    pub content: Option<String>,
}

/// A build's portable delta — mirrors the WIT `types.changeset` record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changeset {
    /// The revision the edits apply against.
    pub base: String,
    /// The per-path edits the build produced.
    pub edits: Vec<Edit>,
}

/// Call-scoped environment the shim resolves and hands to every operation.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    /// The plan-bound adapter identity this call was routed by.
    pub adapter_id: &'a str,
    /// The guest's `"."` preopen root (the shared project mount).
    pub project_root: &'a Path,
    /// The adapter's own MCP reference-shelf endpoint, granted to the
    /// spawned agent so it can fetch `doc://` references lazily. Read
    /// from `wasi:config` by the shim, never hardcoded.
    pub mcp_url: Option<&'a str>,
}

/// Maximum verify-repair iterations per the build brief's Phase 4.
const MAX_REPAIR_ITERATIONS: usize = 2;

/// One format sub-flow of the build brief's Phase 2.
struct SubFlow {
    /// Format name, used in prompts and answer-schema names.
    format: &'static str,
    /// Registry path of the format's sub-brief.
    brief: &'static str,
    /// The `contracts/` subdirectory this format owns, used to route
    /// validator findings back to the owning sub-brief for repair.
    dir: &'static str,
}

/// The three format sub-flows in the build brief's fixed Phase 2 order:
/// the schema vocabulary stabilises before the bindings reference it.
const SUB_FLOWS: [SubFlow; 3] = [
    SubFlow {
        format: "json-schema",
        brief: "briefs/build/json-schema.md",
        dir: "schemas",
    },
    SubFlow {
        format: "openapi",
        brief: "briefs/build/openapi.md",
        dir: "http",
    },
    SubFlow {
        format: "asyncapi",
        brief: "briefs/build/asyncapi.md",
        dir: "messages",
    },
];

/// Adapter-internal answer schema for one format sub-flow leg. Internal
/// legs are not part of the `augentic:specify` contract, so this schema
/// lives here rather than deriving from a canonical schema.
const SUB_FLOW_ANSWER_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "applicable": {
      "description": "Whether the slice has any surface this format owns. `false` means the sub-flow wrote nothing.",
      "type": "boolean"
    },
    "summary": {
      "description": "One-paragraph account of what was authored, imported, repaired, or why the sub-flow was skipped.",
      "minLength": 1,
      "type": "string"
    },
    "written": {
      "default": [],
      "description": "Workspace-relative paths of files this sub-flow created or modified.",
      "items": { "type": "string" },
      "type": "array"
    }
  },
  "required": ["applicable", "summary"]
}"#;

/// One format sub-flow's schema-gated answer.
#[derive(Debug, Deserialize)]
struct SubFlowAnswer {
    applicable: bool,
    summary: String,
    #[serde(default)]
    written: Vec<String>,
}

/// Guidance on the expected build artifacts for this target — the
/// embedded shape brief, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("briefs/shape.md")
}

/// Build a slice's contract deltas under `.specify/slices/<slice>/contracts/`.
///
/// Session-less decomposition: one `create` per format sub-flow (fixed
/// order), a bounded verify-repair loop over the in-core validators, and
/// one report call gated by the derived answer schema. The validators run
/// again after the report lands; residual blocking findings force
/// `status: failure` regardless of the answer.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request as
/// malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let slice_contracts_rel = format!(".specify/slices/{slice}/contracts");
    let slice_contracts = tree_root(ctx, tree).join(&slice_contracts_rel);
    let inputs_block = render_inputs(inputs);
    let build_brief = registry::body("briefs/build.md");

    // Phase 2 — author or import, fixed format order. Classification is
    // part of each sub-flow's own judgment: the brief tells it when to
    // skip, and a skipped leg answers `applicable: false` without writing.
    let mut summaries: Vec<String> = Vec::new();
    for sub_flow in &SUB_FLOWS {
        let format = sub_flow.format;
        let system = format!("{build_brief}\n\n---\n\n{}", registry::body(sub_flow.brief));
        let user = format!(
            "Run the `{format}` sub-flow of the contracts build for slice `{slice}` \
             (adapter `{}`).\n\n\
             The project workspace is lent to you. Write only `.yaml` files under \
             `{slice_contracts_rel}/`; the root `contracts/` baseline is read-only \
             context for `$ref` reuse. When the slice has no surface this format owns, \
             write nothing and answer with `applicable: false`.\n\n\
             {inputs_block}",
            ctx.adapter_id,
        );
        let answer = sub_flow_call(model, ctx, system, user, &format!("{format}-sub-flow")).await?;
        summaries.push(format!(
            "- {format}: applicable={}, wrote {:?} — {}",
            answer.applicable, answer.written, answer.summary
        ));
    }

    // Phase 4 — verify-repair loop over the in-core validators (the
    // Phase 5 tool gate, compiled in). The brief re-enters the owning
    // sub-brief per format; the session-less shape folds that into one
    // repair call per iteration carrying every finding, with the owning
    // sub-briefs inlined so repair does not depend on the MCP route.
    for _ in 0..MAX_REPAIR_ITERATIONS {
        let findings = validate_baseline(&slice_contracts);
        if findings.is_empty() {
            break;
        }
        let system = format!("{build_brief}{}", owning_sub_briefs(&findings, &slice_contracts));
        let user = format!(
            "The contract validators found blocking issues in slice `{slice}`'s delta \
             under `{slice_contracts_rel}/`. Re-enter the owning format sub-brief(s) \
             per the build brief's Phase 4 and repair the files in place.\n\n\
             {}\n\n\
             Answer `applicable: true` with a summary of the repairs.",
            render_validator_findings(&findings),
        );
        sub_flow_call(model, ctx, system, user, "repair").await?;
    }

    // Final leg — the report answer, gated by the derived answer schema.
    let system = build_brief.to_string();
    let user = format!(
        "Write the build report for slice `{slice}`. Verify the delta under \
         `{slice_contracts_rel}/` per the build brief's Phase 3, then answer with \
         the report body (`status`, `findings`, `outputs`, `ui-surface`). A \
         `success` report carries only non-blocking findings. Contract artifacts \
         declare no per-platform outputs, so `outputs` is normally empty.\n\n\
         Sub-flow outcomes:\n{}",
        summaries.join("\n"),
    );
    let report = report_call(model, ctx, system, user).await?;

    // Validate-before-visible: residual blocking findings override the
    // judgment regardless of what the answer claimed.
    Ok(enforce_validators(report, &validate_baseline(&slice_contracts)))
}

/// Merge a built slice's delta into the baseline `contracts/` tree.
///
/// One judgment leg folds the delta and answers with the report; the
/// post-merge validator gate then runs in core, with one bounded repair
/// leg when it finds blocking issues. Merge deliberately gets one repair
/// leg where build gets two: the delta was already
/// validated at build time, so post-merge findings are collision-shaped
/// (`id-unique` against the baseline) and either one pass clears them or
/// the slice needs human review.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request as
/// malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let baseline = tree_root(ctx, tree).join("contracts");
    let merge_brief = registry::body("briefs/merge.md");
    let delta_block = render_delta(delta);

    let user = format!(
        "Merge slice `{slice}`'s built contract delta into the baseline `contracts/` \
         tree (adapter `{}`). The project workspace is lent to you; the delta below \
         applies against base `{}` (a 3-way merge: the baseline is ours, the delta is \
         theirs). Fold the changes in place, resolving conflicts per the merge brief, \
         then answer with the report body.\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let mut report = report_call(model, ctx, merge_brief.to_string(), user).await?;

    // Post-merge validator gate with one bounded repair leg.
    let mut findings = validate_baseline(&baseline);
    if !findings.is_empty() {
        let user = format!(
            "The post-merge contract validators found blocking issues in the merged \
             `contracts/` baseline. Repair the files in place, then answer with the \
             corrected report body.\n\n{}",
            render_validator_findings(&findings),
        );
        report = report_call(model, ctx, merge_brief.to_string(), user).await?;
        findings = validate_baseline(&baseline);
    }

    Ok(enforce_validators(report, &findings))
}

/// Inline the sub-briefs owning the findings' files into a repair
/// prompt, routed by the `contracts/` subdirectory each format owns.
/// Findings that route nowhere pull in every sub-brief, so repair never
/// runs without the specialist material the brief's Phase 4 re-enters.
fn owning_sub_briefs(findings: &[ContractFinding], contracts_dir: &Path) -> String {
    let unrouted = findings.iter().any(|finding| {
        !SUB_FLOWS.iter().any(|sub_flow| finding.path.starts_with(contracts_dir.join(sub_flow.dir)))
    });
    let mut inlined = String::new();
    for sub_flow in &SUB_FLOWS {
        let owned_dir = contracts_dir.join(sub_flow.dir);
        if unrouted || findings.iter().any(|finding| finding.path.starts_with(&owned_dir)) {
            inlined.push_str("\n\n---\n\n");
            inlined.push_str(registry::body(sub_flow.brief));
        }
    }
    inlined
}

/// Resolve the operation's tree root beneath the shared mount.
fn tree_root(ctx: &Context<'_>, tree: &WorkingTree) -> PathBuf {
    tree.subpath
        .as_deref()
        .map_or_else(|| ctx.project_root.to_path_buf(), |sub| ctx.project_root.join(sub))
}

/// The MCP grants offered on every judgment leg: the adapter's own
/// reference shelf, when the shim resolved its endpoint.
fn grants(ctx: &Context<'_>) -> Vec<McpGrant> {
    ctx.mcp_url
        .map(|url| McpGrant {
            name: "contracts-references".to_string(),
            tools: Vec::new(),
            url: url.to_string(),
        })
        .into_iter()
        .collect()
}

/// Issue one internal sub-flow leg and deserialize its answer.
async fn sub_flow_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String, name: &str,
) -> Result<SubFlowAnswer, Error> {
    let reply = model
        .create(Request {
            model: None,
            system: Some(system),
            messages: vec![Message {
                role: Role::User,
                content: user,
            }],
            format: Format::Schema(SchemaFormat {
                name: name.to_string(),
                schema: SUB_FLOW_ANSWER_SCHEMA.to_string(),
            }),
            mcp: grants(ctx),
            lend_workspace: true,
        })
        .await?;
    serde_json::from_str(&reply.answer)
        .map_err(|err| Error::Internal(format!("sub-flow answer did not deserialize: {err}")))
}

/// Issue one report leg gated by the derived answer schema and project
/// the answer onto the seam-facing report.
async fn report_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String,
) -> Result<Report, Error> {
    let reply = model
        .create(Request {
            model: None,
            system: Some(system),
            messages: vec![Message {
                role: Role::User,
                content: user,
            }],
            format: Format::Schema(SchemaFormat {
                name: "report".to_string(),
                schema: REPORT_ANSWER_SCHEMA.to_string(),
            }),
            mcp: grants(ctx),
            lend_workspace: true,
        })
        .await?;
    let answer = ReportAnswer::parse(&reply.answer)
        .map_err(|err| Error::Internal(format!("report answer did not deserialize: {err}")))?;
    Ok(answer.into_report())
}

/// Deterministic guard after the answer lands: residual validator
/// findings force `failure` and are appended to the report; a `success`
/// answer carrying blocking findings is downgraded the same way.
fn enforce_validators(mut report: Report, residual: &[ContractFinding]) -> Report {
    if !residual.is_empty() {
        report.status = Status::Failure;
        report.findings.extend(residual.iter().map(Finding::from_validator));
    }
    if report.status == Status::Success
        && report.findings.iter().any(|finding| finding.severity.blocking())
    {
        report.status = Status::Failure;
    }
    report
}

/// Render the typed inputs as labeled prompt sections.
fn render_inputs(inputs: &[Input]) -> String {
    if inputs.is_empty() {
        return "(no slice artifacts were provided)".to_string();
    }
    inputs
        .iter()
        .map(|input| format!("### input: {}\n\n{}", input.label(), input.body()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render validator findings for a repair prompt.
fn render_validator_findings(findings: &[ContractFinding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!("- [{}] {}: {}", finding.rule_id, finding.path.display(), finding.detail)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render a changeset's edits for the merge prompt.
fn render_delta(delta: &Changeset) -> String {
    if delta.edits.is_empty() {
        return "### delta\n\n(empty changeset — the slice wrote no contract files)".to_string();
    }
    let edits = delta
        .edits
        .iter()
        .map(|edit| {
            edit.content.as_ref().map_or_else(
                || format!("- {} (deleted)", edit.path),
                |content| format!("- {} (content: {content})", edit.path),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("### delta (base {})\n\n{edits}", delta.base)
}
