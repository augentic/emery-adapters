//! Path-resolver and `validate all` integration tests.

use serde_json::Value;
use specify_vectis::validate::{ValidateArgs as Args, ValidateMode, run};

use crate::engine_support::{
    write_assets_project, write_project_yaml, write_specify_project, write_specs_composition,
};

/// Run `validate all` against `root` and return the combined envelope.
fn run_all(root: &std::path::Path) -> Value {
    run(&Args {
        mode: ValidateMode::All,
        path: Some(root.to_path_buf()),
    })
    .expect("run all succeeds")
}

/// Fetch the per-mode `report` object for `mode` from an `all` envelope.
fn report_for<'a>(envelope: &'a Value, mode: &str) -> &'a Value {
    let results = envelope["results"].as_array().expect("results array");
    let entry = results
        .iter()
        .find(|entry| entry["mode"] == mode)
        .unwrap_or_else(|| panic!("missing {mode} sub-report: {envelope}"));
    &entry["report"]
}

/// `validate all` resolves the change-local `layout.yaml` ahead of the
/// project-shape `design-system/layout.yaml` when both exist. The
/// resolved input rides each sub-report's `path`, so the change-local
/// precedence is observable through the public surface without reaching
/// into the resolver.
#[test]
fn all_prefers_change_local_layout() {
    let tmp = write_specify_project();
    let change_dir = tmp.path().join(".specify/slices/active");
    std::fs::create_dir_all(&change_dir).expect("mkdir change");
    std::fs::write(change_dir.join("layout.yaml"), "version: 1\nscreens: {}\n")
        .expect("write change-local layout.yaml");
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(design.join("layout.yaml"), "version: 1\nscreens: {}\n")
        .expect("write design-system/layout.yaml");

    let envelope = run_all(tmp.path());
    let layout = report_for(&envelope, "layout");
    assert!(layout.get("skipped").is_none(), "change-local layout MUST resolve: {layout}");
    let resolved = layout["path"].as_str().expect("layout path string");
    assert!(
        resolved.ends_with(".specify/slices/active/layout.yaml"),
        "expected change-local resolution, got: {resolved}"
    );
}

/// When no change-local file exists, `validate all` falls back to the
/// project-shape `design-system/tokens.yaml`, observed via the tokens
/// sub-report's resolved `path`.
#[test]
fn all_falls_back_to_design_system_tokens() {
    let tmp = write_specify_project();
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(design.join("tokens.yaml"), "version: 1\n").expect("write tokens.yaml");

    let envelope = run_all(tmp.path());
    let tokens = report_for(&envelope, "tokens");
    assert!(tokens.get("skipped").is_none(), "design-system tokens MUST resolve: {tokens}");
    let resolved = tokens["path"].as_str().expect("tokens path string");
    assert!(
        resolved.ends_with("design-system/tokens.yaml"),
        "expected project-shape resolution, got: {resolved}"
    );
}

/// The `<name>` slice template expands against every directory under
/// `.specify/slices/` in alphabetical order, and the first existing
/// file wins. With both `alpha` and `zeta` carrying a `tokens.yaml`,
/// `validate all` resolves the `alpha` copy.
#[test]
fn all_resolves_alphabetically_first_slice() {
    let tmp = write_specify_project();
    let slices = tmp.path().join(".specify/slices");
    for name in ["zeta", "alpha"] {
        let dir = slices.join(name);
        std::fs::create_dir_all(&dir).expect("mkdir slice");
        std::fs::write(dir.join("tokens.yaml"), "version: 1\n").expect("write slice tokens.yaml");
    }

    let envelope = run_all(tmp.path());
    let tokens = report_for(&envelope, "tokens");
    let resolved = tokens["path"].as_str().expect("tokens path string");
    assert!(
        resolved.ends_with(".specify/slices/alpha/tokens.yaml"),
        "expected alphabetically-first slice resolution, got: {resolved}"
    );
}

/// The combined-run envelope MUST carry `mode: "all"`, the project
/// root in `path`, and a `results` array with exactly four sub-reports
/// in the canonical order layout → composition → tokens → assets.
/// Each sub-report has its own per-mode envelope under `report`.
#[test]
fn all_envelope_runs_every_mode_in_canonical_order() {
    let tmp = write_specify_project();

    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(design.join("layout.yaml"), "version: 1\nscreens: {}\n")
        .expect("write layout.yaml");
    std::fs::write(design.join("tokens.yaml"), "version: 1\n").expect("write tokens.yaml");
    std::fs::write(design.join("assets.yaml"), "version: 1\nassets: {}\n")
        .expect("write assets.yaml");
    let specs = tmp.path().join(".specify/specs");
    std::fs::create_dir_all(&specs).expect("mkdir specs");
    std::fs::write(specs.join("composition.yaml"), "version: 1\nscreens: {}\n")
        .expect("write composition.yaml");

    let envelope = run(&Args {
        mode: ValidateMode::All,
        path: Some(tmp.path().to_path_buf()),
    })
    .expect("run all succeeds");

    assert_eq!(envelope["mode"], "all");
    assert_eq!(envelope["path"].as_str().expect("path string"), tmp.path().display().to_string());
    let results = envelope["results"].as_array().expect("results array");
    assert_eq!(results.len(), 4, "expected four sub-reports: {envelope}");
    assert_eq!(results[0]["mode"], "layout");
    assert_eq!(results[1]["mode"], "composition");
    assert_eq!(results[2]["mode"], "tokens");
    assert_eq!(results[3]["mode"], "assets");

    for entry in results {
        let report = &entry["report"];
        assert!(report.get("skipped").is_none(), "unexpected skipped: {entry}");
        assert_eq!(
            report["errors"].as_array().map(Vec::len),
            Some(0),
            "{}: unexpected errors: {entry}",
            entry["mode"]
        );
    }
}

/// Sub-modes whose default-resolved input does not exist on disk MUST
/// surface as a synthetic `{ skipped: true }` sub-report rather than a
/// hard `InvalidProject` failure -- so `validate all` keeps running
/// through the rest of the modes.
#[test]
fn all_envelope_skips_missing_inputs_without_failing() {
    let tmp = write_specify_project();
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(design.join("tokens.yaml"), "version: 1\n").expect("write tokens.yaml");

    let envelope = run(&Args {
        mode: ValidateMode::All,
        path: Some(tmp.path().to_path_buf()),
    })
    .expect("run all does not fail on missing inputs");

    let results = envelope["results"].as_array().expect("results array");
    let by_mode: std::collections::BTreeMap<&str, &Value> =
        results.iter().map(|e| (e["mode"].as_str().expect("mode str"), e)).collect();

    for skipped_mode in ["layout", "composition", "assets"] {
        let report = &by_mode[skipped_mode]["report"];
        assert_eq!(
            report["skipped"],
            Value::Bool(true),
            "[{skipped_mode}] expected skipped: {report}",
        );
        assert_eq!(
            report["errors"].as_array().map(Vec::len),
            Some(0),
            "[{skipped_mode}] errors must stay empty: {report}"
        );
    }

    // Skipped sub-reports still name the last-candidate fallback path
    // (the project / baseline shape), so the operator-facing "not
    // readable" location stays the friendliest one.
    let layout_path = by_mode["layout"]["report"]["path"].as_str().expect("layout path string");
    assert!(
        layout_path.ends_with("design-system/layout.yaml"),
        "expected design-system/layout.yaml fallback, got: {layout_path}"
    );
    let composition_path =
        by_mode["composition"]["report"]["path"].as_str().expect("composition path string");
    assert!(
        composition_path.ends_with(".specify/specs/composition.yaml"),
        "expected baseline composition fallback, got: {composition_path}"
    );

    let tokens_report = &by_mode["tokens"]["report"];
    assert!(
        tokens_report.get("skipped").is_none(),
        "tokens.yaml IS on disk; skipped MUST be absent: {tokens_report}",
    );
}

/// A sub-mode's findings MUST surface inside `results[*].report` so
/// the dispatcher's recursion-aware `validate_exit_code` helper picks
/// them up. This test feeds a deliberately-broken tokens.yaml and
/// asserts the broken-hex error rides the nested sub-report.
#[test]
fn all_envelope_propagates_sub_errors() {
    let tmp = write_specify_project();
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(
        design.join("tokens.yaml"),
        "version: 1\ncolors:\n  primary:\n    light: \"#xyz\"\n    dark: \"#000000\"\n",
    )
    .expect("write tokens.yaml");

    let envelope = run(&Args {
        mode: ValidateMode::All,
        path: Some(tmp.path().to_path_buf()),
    })
    .expect("run all succeeds");
    let results = envelope["results"].as_array().expect("results array");
    let tokens_entry =
        results.iter().find(|e| e["mode"] == "tokens").expect("tokens sub-report present");
    let tokens_errors = tokens_entry["report"]["errors"].as_array().expect("tokens errors array");
    assert!(
        !tokens_errors.is_empty(),
        "broken hex MUST surface in nested tokens report: {envelope}"
    );
}

/// `validate all` fans out `assets-materialization-missing` through the
/// assets sub-report.
#[test]
fn all_envelope_propagates_materialization_missing() {
    let yaml = r"version: 1
assets:
  hero:
    kind: raster
    role: illustration
    sources:
      ios:
        1x: assets/hero.png
";
    let (tmp, assets_path) = write_assets_project(yaml, &["assets/hero.png"]);
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    write_specs_composition(
        tmp.path(),
        r"version: 1
screens:
  s:
    name: S
    body:
      list:
        item:
          - image:
              name: hero
",
    );
    let design = tmp.path().join("design-system");
    std::fs::write(design.join("layout.yaml"), "version: 1\nscreens: {}\n").expect("layout");
    std::fs::write(design.join("tokens.yaml"), "version: 1\n").expect("tokens");
    drop(assets_path);

    let envelope = run(&Args {
        mode: ValidateMode::All,
        path: Some(tmp.path().to_path_buf()),
    })
    .expect("run all succeeds");
    let results = envelope["results"].as_array().expect("results array");
    let assets_entry =
        results.iter().find(|e| e["mode"] == "assets").expect("assets sub-report present");
    let errors = assets_entry["report"]["errors"].as_array().expect("assets errors");
    assert!(
        errors.iter().any(|e| e["message"]
            .as_str()
            .unwrap_or("")
            .contains("assets-materialization-missing")),
        "materialization-missing MUST surface in nested assets report: {envelope}"
    );
}
