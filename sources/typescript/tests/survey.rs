//! Asserts what the TypeScript adapter decides before the SDK's fan-out.
//!
//! - Which entries are production source, and so where a surface may be
//!   entered: an inventory entered anywhere else goes back as a finding.
//! - How the model's inventory becomes seams: one note per surface, in
//!   answer order, each naming its surface and entry and lent the root —
//!   however many surfaces enter at one module.
//! - What it refuses: a tree the model finds no surface in, after one turn.
//! - When no survey turn is spent at all: an inline value is the bound input
//!   whole.
//!
//! What the survey call itself does for every adapter — the request shape,
//! the check against the tree, the corrections — is the SDK's suite's.

use std::path::Path;

use emery_sdk::{Context, Doc, Error, Seam, SourceInput};
use omnia_test::guest::Scripted;
use typescript::survey::survey;

// The adapter's own corpus, so the turn runs under its `prompts/survey.md`.
static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

const KEY: &str = "legacy-monolith";

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:typescript",
        input,
    }
}

// An empty module at each relative path, directories made on the way; the
// root as the engine lends it.
fn tree<'a, const N: usize>(root: &'a Path, files: [&str; N]) -> &'a str {
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(path, "").expect("the module is written");
    }
    root.to_str().expect("a UTF-8 scratch root")
}

// Each seam's note; anything else is not this adapter's survey.
fn notes(seams: &[Seam]) -> Vec<&str> {
    seams
        .iter()
        .map(|seam| match seam {
            Seam::Note(note) => note.as_str(),
            other => panic!("a typescript seam is a note, got {other:?}"),
        })
        .collect()
}

// The surface a note tells its call to mine, as `(name, entry)`.
fn surface(note: &str) -> (&str, &str) {
    let name = note
        .lines()
        .find_map(|line| line.strip_prefix("- surface: "))
        .expect("the note names its surface");
    let entry = note
        .lines()
        .find_map(|line| line.strip_prefix("- entry: `")?.strip_suffix('`'))
        .expect("the note names its entry");
    (name, entry)
}

// A tree under one `src/` directory is surveyed all the same — the boundary
// is the model's to find, not the layout's — under the embedded survey prompt
// with the root lent. Each surface the model finds is one note, in answer
// order, naming the surface and its entry and lending the root, so the
// extract call follows imports into siblings.
#[tokio::test]
async fn single_directory() {
    let model = Scripted::answering([r#"{"surfaces":[
        {"name":"POST /orders","entry":"src/routes.ts"},
        {"name":"nightly reconciliation job","entry":"src/jobs.ts"}
    ]}"#]);
    let scratch = tempfile::tempdir().expect("a scratch tree");
    let root = tree(
        scratch.path(),
        ["src/index.ts", "src/routes.ts", "src/orders.ts", "src/jobs.ts", "src/db.ts"],
    );

    let input = SourceInput::workspace(KEY, root);
    let seams = survey(&model, &ctx(&input), DOCS).await.expect("the inventory is accepted");

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "one survey turn");
    let request = &seen[0];
    let prompt = emery_sdk::prose::body(DOCS, "prompts/survey.md").expect("embedded");
    assert_eq!(request.system.as_deref(), Some(prompt), "the survey prompt is the system");
    assert_eq!(request.workspace.as_deref(), Some(root), "the root is lent");

    let notes = notes(&seams);
    let surfaces: Vec<_> = notes.iter().map(|note| surface(note)).collect();
    assert_eq!(
        surfaces,
        [("POST /orders", "src/routes.ts"), ("nightly reconciliation job", "src/jobs.ts")]
    );
    let lend = format!("read-only view at `{root}`");
    for note in notes {
        assert!(note.contains(&lend), "the root is lent: {note}");
    }
    model.assert_exhausted();
}

// Several surfaces may enter at one module — a router registering its
// routes — and each is its own seam: the extract call mines the surface, not
// the file, so nothing is grouped and no module is a seam of its own.
#[tokio::test]
async fn shared_entry() {
    let model = Scripted::answering([r#"{"surfaces":[
        {"name":"POST /users","entry":"src/users/router.ts"},
        {"name":"GET /users/:id","entry":"src/users/router.ts"}
    ]}"#]);
    let scratch = tempfile::tempdir().expect("a scratch tree");
    let root =
        tree(scratch.path(), ["src/index.ts", "src/users/router.ts", "src/users/repository.ts"]);

    let input = SourceInput::workspace(KEY, root);
    let seams = survey(&model, &ctx(&input), DOCS).await.expect("the inventory is accepted");

    let surfaces: Vec<_> = notes(&seams).into_iter().map(surface).collect();
    assert_eq!(
        surfaces,
        [("POST /users", "src/users/router.ts"), ("GET /users/:id", "src/users/router.ts")]
    );
    model.assert_exhausted();
}

// Dependencies, build output, tests, declaration files, and dot entries are
// not production source, and a file this adapter does not read is not a
// module: an inventory entered at any of them — present in the tree though
// they are — goes back as findings, one per entry, and only a production
// module is accepted.
#[tokio::test]
async fn non_production() {
    let model = Scripted::answering([
        r#"{"surfaces":[
            {"name":"mail delivery","entry":"services/mail.test.ts"},
            {"name":"mail spec","entry":"services/mail.spec.ts"},
            {"name":"types","entry":"services/types.d.ts"},
            {"name":"left-pad","entry":"node_modules/left-pad/index.js"},
            {"name":"bundle","entry":"dist/bundle.js"},
            {"name":"e2e","entry":"tests/orders.e2e.ts"},
            {"name":"git","entry":".git/HEAD"},
            {"name":"readme","entry":"routes/README.md"}
        ]}"#,
        r#"{"surfaces":[{"name":"POST /orders","entry":"routes/orders.ts"}]}"#,
    ]);
    let scratch = tempfile::tempdir().expect("a scratch tree");
    let root = tree(
        scratch.path(),
        [
            "routes/orders.ts",
            "routes/users.ts",
            "routes/README.md",
            "services/index.ts",
            "services/mail.ts",
            "services/mail.test.ts",
            "services/mail.spec.ts",
            "services/types.d.ts",
            "node_modules/left-pad/index.js",
            "dist/bundle.js",
            "tests/orders.e2e.ts",
            ".git/HEAD",
        ],
    );

    let input = SourceInput::workspace(KEY, root);
    let seams = survey(&model, &ctx(&input), DOCS).await.expect("the second inventory is accepted");

    let surfaces: Vec<_> = notes(&seams).into_iter().map(surface).collect();
    assert_eq!(surfaces, [("POST /orders", "routes/orders.ts")]);
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 2, "one rejection, one acceptance");
    let correction = exchanges[0].outcome.as_ref().expect_err("no entry is production source");
    for refused in [
        "services/mail.test.ts",
        "services/mail.spec.ts",
        "services/types.d.ts",
        "node_modules/left-pad/index.js",
        "dist/bundle.js",
        "tests/orders.e2e.ts",
        ".git/HEAD",
        "routes/README.md",
    ] {
        assert!(
            correction.contains(&format!("`{refused}` is not a module this adapter mines")),
            "{refused}: {correction}"
        );
    }
    model.assert_exhausted();
}

// An inventory the SDK's check refuses — here a surface entered at a module
// the tree does not hold — goes back to the model, and the adapter's notes
// are cut from the inventory finally accepted, never the refused one.
#[tokio::test]
async fn corrected_inventory() {
    let model = Scripted::answering([
        r#"{"surfaces":[{"name":"POST /orders","entry":"routes/ghost.ts"}]}"#,
        r#"{"surfaces":[{"name":"POST /orders","entry":"routes/orders.ts"}]}"#,
    ]);
    let scratch = tempfile::tempdir().expect("a scratch tree");
    let root = tree(scratch.path(), ["routes/orders.ts", "routes/users.ts", "services/orders.ts"]);

    let input = SourceInput::workspace(KEY, root);
    let seams = survey(&model, &ctx(&input), DOCS).await.expect("the second inventory is accepted");

    let surfaces: Vec<_> = notes(&seams).into_iter().map(surface).collect();
    assert_eq!(surfaces, [("POST /orders", "routes/orders.ts")]);
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 2, "one rejection, one acceptance");
    let correction = exchanges[0].outcome.as_ref().expect_err("the first inventory is refused");
    assert!(correction.contains("`routes/ghost.ts`"), "{correction}");
    model.assert_exhausted();
}

// A tree the model finds no surface in is refused, not mined: a source no
// caller reaches is incomplete input or a failed discovery, and mining it
// whole would raise what no caller observes into requirements. A tree with
// no production module at all — documentation, build output, tests alone —
// ends the same way, one turn later.
#[tokio::test]
async fn no_surface() {
    let model = Scripted::answering([r#"{"surfaces":[]}"#, r#"{"surfaces":[]}"#]);
    let internals = tempfile::tempdir().expect("a scratch tree");
    let root = tree(internals.path(), ["src/lib/db.ts", "src/lib/logger.ts", "src/lib/format.ts"]);
    let input = SourceInput::workspace(KEY, root);
    let error = survey(&model, &ctx(&input), DOCS).await.expect_err("no surface, nothing to mine");
    assert!(matches!(error, Error::BadRequest { .. }), "{error}");
    assert!(error.description().contains("exposes no surface"), "{error}");

    let unproductive = tempfile::tempdir().expect("a scratch tree");
    let root = tree(
        unproductive.path(),
        ["README.md", "dist/bundle.js", "tests/orders.e2e.ts", "src/types.d.ts"],
    );
    let input = SourceInput::workspace(KEY, root);
    let error = survey(&model, &ctx(&input), DOCS).await.expect_err("no module to enter at");
    assert!(matches!(error, Error::BadRequest { .. }), "{error}");
    assert!(error.description().contains("exposes no surface"), "{error}");
    model.assert_exhausted();
}

// An inline value has no tree to survey: the bound value, whole, and no
// survey turn spent on it.
#[tokio::test]
async fn inline_value() {
    let model = Scripted::default();
    let input = SourceInput::value(KEY, "export const port = 8080;");

    let seams = survey(&model, &ctx(&input), DOCS).await.expect("a value is surveyed");

    assert_eq!(seams, [Seam::Whole]);
    assert!(model.seen().is_empty(), "no turn was spent");
}
