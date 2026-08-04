//! Cross-artifact validation scenarios over the public
//! [`vectis::validate::run`] entry: conventional committed-export
//! coverage (`validate assets`) and the structural-identity rule
//! (`validate composition`).

use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;
use vectis::validate::{ValidateMode, run};

fn errors_array(envelope: &Value) -> &[Value] {
    envelope.get("errors").and_then(Value::as_array).expect("errors array").as_slice()
}

fn error_messages(envelope: &Value) -> Vec<String> {
    errors_array(envelope)
        .iter()
        .map(|e| e["message"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn write_rel(root: &Path, rel: &str, body: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
    std::fs::write(&path, body).expect("write file");
}

fn assets_project(assets_yaml: &str, composition_yaml: &str) -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    let assets_path = design.join("assets.yaml");
    std::fs::write(&assets_path, assets_yaml).expect("write assets.yaml");
    write_rel(tmp.path(), ".emery/specs/composition.yaml", composition_yaml.as_bytes());
    (tmp, assets_path)
}

const ASSETS_YAML: &str = r"version: 1
assets:
  chevron-right:
    alt: Chevron
    kind: vector
    role: icon
    sources:
      ios: assets/ios/chevron-right.pdf
  check:
    alt: Check
    kind: vector
    role: icon
    sources:
      android: assets/android/check.xml
  hero:
    alt: Hero
    kind: raster
    role: illustration
    sources:
      ios:
        2x: assets/hero@2x.png
        3x: assets/hero@3x.png
";

const COMPOSITION_YAML: &str = r"version: 1
screens:
  home:
    name: Home
    body:
      - group:
          direction: column
          items:
            - icon:
                name: chevron-right
            - icon:
                name: check
            - image:
                name: hero
";

// Conventional committed-export coverage through `validate assets`:
// a composition-referenced asset with no `sources.<platform>` pin
// needs a conventional export on disk — the Android drawable XML, an
// iOS imageset artifact with real PDF magic (Contents.json and junk
// bytes do not count), or a per-density Android raster PNG.
#[test]
fn conventional_export_matrix() {
    let (tmp, assets_path) = assets_project(ASSETS_YAML, COMPOSITION_YAML);
    let design = tmp.path().join("design-system");

    // Declared pin files exist; conventional exports do not (yet).
    write_rel(&design, "assets/ios/chevron-right.pdf", b"%PDF-1.4\nstub\n");
    write_rel(&design, "assets/android/check.xml", b"<vector/>");
    write_rel(&design, "assets/hero@2x.png", b"PNGSTUB");
    write_rel(&design, "assets/hero@3x.png", b"PNGSTUB");
    // A Contents.json-only imageset plus a magic-less pdf must not
    // count as a materialized iOS export.
    write_rel(&design, "assets/exports/ios/check.imageset/Contents.json", b"{\"images\":[]}");
    write_rel(&design, "assets/exports/ios/check.imageset/check.pdf", b"1 0 obj\nendobj\n");

    let envelope = run(ValidateMode::Assets, Some(&assets_path)).expect("run succeeds");
    let messages = error_messages(&envelope);
    let missing: Vec<&String> =
        messages.iter().filter(|m| m.contains("assets-materialization-missing")).collect();
    assert_eq!(missing.len(), 3, "expected three coverage gaps: {messages:?}");
    assert!(
        missing.iter().any(|m| m.contains("`chevron-right`") && m.contains("android")),
        "{messages:?}"
    );
    assert!(missing.iter().any(|m| m.contains("`check`") && m.contains("ios")), "{messages:?}");
    assert!(missing.iter().any(|m| m.contains("`hero`") && m.contains("android")), "{messages:?}");

    // Commit the conventional exports: drawable XML, a real-magic PDF
    // inside the imageset, and one Android density PNG.
    write_rel(&design, "assets/exports/android/drawable/chevron_right.xml", b"<vector/>");
    write_rel(&design, "assets/exports/ios/check.imageset/check.pdf", b"%PDF-1.4\n1 0 obj\n");
    write_rel(&design, "assets/exports/android/drawable-mdpi/hero.png", b"PNGSTUB");

    let envelope = run(ValidateMode::Assets, Some(&assets_path)).expect("run succeeds");
    assert!(
        errors_array(&envelope).is_empty(),
        "committed exports should satisfy coverage: {envelope}"
    );
}

const REWIRED_COMPOSITION: &str = r"version: 1
screens:
  home:
    name: Home
    body:
      - group:
          component: nav-row
          direction: row
          items:
            - icon-button:
                icon: home
                label: Home
                event: NavigateHome
            - icon-button:
                icon: search
                label: Search
                event: NavigateSearch
  settings:
    name: Settings
    body:
      - group:
          component: nav-row
          direction: row
          items:
            - icon-button:
                icon: person
                label: Profile
                event: NavigateProfile
            - icon-button:
                icon: inbox
                label: Inbox
                event: NavigateInbox
";

const DIVERGENT_COMPOSITION: &str = r"version: 1
screens:
  home:
    name: Home
    body:
      - group:
          component: nav-row
          direction: row
          items:
            - icon-button:
                icon: home
                label: Home
            - icon-button:
                icon: search
                label: Search
  settings:
    name: Settings
    body:
      - group:
          component: nav-row
          direction: row
          items:
            - icon-button:
                icon: person
                label: Profile
            - icon-button:
                icon: inbox
                label: Inbox
            - icon-button:
                icon: gear
                label: Settings
";

fn composition_file(yaml: &str) -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("composition.yaml");
    std::fs::write(&path, yaml).expect("write composition.yaml");
    (tmp, path)
}

// The structural-identity rule through `validate composition`: base
// instances of one `component:` slug may differ in wiring and text
// but their group skeleton must match; a diverging cardinality is a
// finding naming the slug and both paths.
#[test]
fn structural_identity_matrix() {
    let (_tmp, path) = composition_file(REWIRED_COMPOSITION);
    let envelope = run(ValidateMode::Composition, Some(&path)).expect("run succeeds");
    assert!(errors_array(&envelope).is_empty(), "rewired-only instances must validate: {envelope}");

    let (_tmp, path) = composition_file(DIVERGENT_COMPOSITION);
    let envelope = run(ValidateMode::Composition, Some(&path)).expect("run succeeds");
    let messages = error_messages(&envelope);
    assert_eq!(messages.len(), 1, "one identity finding expected: {messages:?}");
    assert!(messages[0].contains("structural-identity"), "{messages:?}");
    assert!(messages[0].contains("`nav-row`"), "{messages:?}");
}

const UNIQUE_TEST_IDS_COMPOSITION: &str = r"version: 1
screens:
  splash:
    name: Splash
    body:
      - button:
          label: Go
          test_id: splash-cta
";

const DUPLICATE_TEST_IDS_COMPOSITION: &str = r"version: 1
screens:
  a:
    name: A
    body:
      - button:
          test_id: same-id
  b:
    name: B
    body:
      - button:
          test_id: same-id
";

const NON_KEBAB_TEST_ID_COMPOSITION: &str = r"version: 1
screens:
  splash:
    name: Splash
    body:
      - button:
          test_id: Splash_CTA
";

// `test_id` format and uniqueness through `validate composition`.
#[test]
fn composition_test_id_matrix() {
    let (_tmp, path) = composition_file(UNIQUE_TEST_IDS_COMPOSITION);
    let envelope = run(ValidateMode::Composition, Some(&path)).expect("run succeeds");
    assert!(errors_array(&envelope).is_empty(), "unique kebab test ids must validate: {envelope}");

    let (_tmp, path) = composition_file(DUPLICATE_TEST_IDS_COMPOSITION);
    let envelope = run(ValidateMode::Composition, Some(&path)).expect("run succeeds");
    let messages = error_messages(&envelope);
    assert_eq!(messages.len(), 1, "one duplicate finding expected: {messages:?}");
    assert!(messages[0].contains("duplicate"), "{messages:?}");
    assert!(messages[0].contains("same-id"), "{messages:?}");

    let (_tmp, path) = composition_file(NON_KEBAB_TEST_ID_COMPOSITION);
    let envelope = run(ValidateMode::Composition, Some(&path)).expect("run succeeds");
    let messages = error_messages(&envelope);
    assert!(
        messages.iter().any(|m| m.contains("must match") && m.contains("Splash_CTA")),
        "format finding expected: {messages:?}"
    );
}
