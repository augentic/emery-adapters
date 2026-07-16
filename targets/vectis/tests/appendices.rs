//! Appendix C / D / E worked-example pins against the validation engine.
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::{NamedTempFile, TempDir};
use vectis::validate::{ValidateMode, run};

fn errors_array(envelope: &Value) -> &[Value] {
    envelope.get("errors").and_then(Value::as_array).expect("errors array").as_slice()
}

fn warnings_array(envelope: &Value) -> &[Value] {
    envelope.get("warnings").and_then(Value::as_array).expect("warnings array").as_slice()
}

fn write_named(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("tempfile");
    file.write_all(content.as_bytes()).expect("write fixture");
    file
}

fn write_assets_project(yaml: &str, files: &[&str]) -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let design = tmp.path().join("design-system");
    let assets_path = design.join("assets.yaml");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(&assets_path, yaml).expect("write assets.yaml");
    for rel in files {
        let path = design.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
        std::fs::write(&path, b"PNGSTUB").expect("write fixture file");
    }
    (tmp, assets_path)
}

fn write_specs_composition(project: &Path, yaml: &str) {
    let dir = project.join(".specify").join("specs");
    std::fs::create_dir_all(&dir).expect("mkdir .specify/specs");
    std::fs::write(dir.join("composition.yaml"), yaml).expect("write composition.yaml");
}

// Appendix C verbatim — pinned happy-path layout fixture.
const APPENDIX_C_LAYOUT_YAML: &str = r#"version: 1

provenance:
  sources:
    - kind: screenshots
      captured_at: "2026-04-12T10:30:00Z"
    - kind: manual

screens:
  task-list:
    name: Task list
    description: Primary screen showing all open tasks for the signed-in user.
    header:
      title: My tasks
      trailing:
        - icon-button:
            icon: settings
            label: Open settings
    body:
      list:
        each: tasks
        style: plain
        item:
          - group:
              component: task-row
              direction: row
              gap: md
              padding: md
              align: center
              items:
                - checkbox:
                    label: Mark task complete
                - group:
                    direction: column
                    gap: xs
                    size:
                      width: fill
                    items:
                      - text:
                          role: heading
                          style: body
                      - text:
                          style: caption
                          color: on-surface-variant
                - icon:
                    name: chevron-right
                    color: on-surface-variant
    fab:
      icon: plus
      label: Add task
    states:
      empty:
        when: tasks.is_empty
        replaces: body
        body:
          - group:
              direction: column
              gap: md
              padding: lg
              align: center
              justify: center
              items:
                - image:
                    name: empty-tasks-hero
                - text:
                    content: No tasks yet
                    style: title
                - text:
                    content: Tap the + button to add your first task.
                    style: body
                    color: on-surface-variant
      loading:
        when: tasks.is_loading
        replaces: body
        body:
          - progress-indicator:
              style: circular
    overlays:
      delete-confirm:
        kind: dialog
        title: Delete task?
        content:
          - text:
              content: This task will be removed permanently.
          - group:
              direction: row
              gap: sm
              justify: end
              items:
                - button:
                    label: Cancel
                    style: text
                - button:
                    label: Delete
                    style: text
                    color: error

  settings:
    name: Settings
    header:
      title: Settings
      leading:
        - icon-button:
            icon: chevron-left
            label: Back
    body:
      form:
        - group:
            direction: column
            gap: lg
            padding: md
            items:
              - text:
                  content: Appearance
                  role: heading
                  style: title
              - segmented-control:
                  options:
                    - System
                    - Light
                    - Dark
              - text:
                  content: Account
                  role: heading
                  style: title
              - button:
                  label: Sign out
                  style: outlined
                  color: error
    platforms:
      ios:
        header:
          title: Settings
      android:
        header:
          title: Settings
"#;

// Appendix D verbatim — `r##"..."##` keeps embedded `"#0066CC"` literals intact.
const APPENDIX_D_TOKENS_YAML: &str = r##"version: 1

provenance:
  sources:
    - kind: figma-variables
      uri: "https://www.figma.com/file/ABC123/Design-System"
      captured_at: "2026-04-10T09:15:00Z"
    - kind: manual

colors:
  primary:
    light: "#0066CC"
    dark: "#3399FF"
  on-primary:
    light: "#FFFFFF"
    dark: "#001F3F"
  surface:
    light: "#FFFFFF"
    dark: "#121212"
  on-surface:
    light: "#1C1B1F"
    dark: "#E6E1E5"
  on-surface-variant:
    light: "#49454F"
    dark: "#CAC4D0"
  outline:
    light: "#79747E"
    dark: "#938F99"
  error:
    light: "#B3261E"
    dark: "#F2B8B5"

typefaces:
  default:
    family: "Inter"
    fallback: "system-ui, sans-serif"
    source: google-fonts
  mono:
    family: "Roboto Mono"
    source: google-fonts

typography:
  caption:
    typeface: default
    size: 12
    weight: regular
    lineHeight: 16
  body:
    size: 16
    weight: regular
    lineHeight: 24
  title:
    typeface: default
    size: 22
    weight: semibold
    lineHeight: 28
  display:
    size: 32
    weight: bold
    lineHeight: 40
    letterSpacing: -0.5
  code-inline:
    typeface: mono
    size: 14
    weight: regular
    lineHeight: 20

spacing:
  xs: 4
  sm: 8
  md: 16
  lg: 24
  xl: 32

cornerRadius:
  sm: 4
  md: 8
  lg: 16

elevation:
  card: 2
  modal: 8

border:
  subtle:
    width: 1
    color: outline
  emphasis:
    width: 2
    color: primary
    radius: md

opacity:
  disabled: 0.38
  scrim: 0.4
"##;

// Appendix E verbatim — pinned happy-path assets fixture.
const APPENDIX_E_ASSETS_YAML: &str = r#"version: 1

provenance:
  sources:
    - kind: manual

assets:
  empty-tasks-hero:
    kind: raster
    role: illustration
    alt: "Empty clipboard with a relaxed character beside it"
    sources:
      ios:
        1x: assets/empty-tasks-hero.png
        2x: assets/empty-tasks-hero@2x.png
        3x: assets/empty-tasks-hero@3x.png
      android:
        mdpi: assets/android/empty-tasks-hero-mdpi.png
        hdpi: assets/android/empty-tasks-hero-hdpi.png
        xhdpi: assets/android/empty-tasks-hero-xhdpi.png
        xxhdpi: assets/android/empty-tasks-hero-xxhdpi.png

  brand-logo:
    kind: vector
    role: illustration
    alt: "Acme logo"
    source: assets/brand-logo.svg
    sources:
      ios: assets/ios/brand-logo.pdf
      android: assets/android/brand-logo.xml

  settings:
    kind: symbol
    role: icon
    symbols:
      ios: gearshape
      android: settings
    tint: on-surface

  chevron-left:
    kind: symbol
    role: icon
    symbols:
      ios: chevron.left
      android: arrow_back
    tint: on-surface

  chevron-right:
    kind: symbol
    role: icon
    symbols:
      ios: chevron.right
      android: chevron_right
    tint: on-surface-variant

  plus:
    kind: symbol
    role: icon
    symbols:
      ios: plus
      android: add
    tint: on-primary
"#;

const APPENDIX_E_FILES: &[&str] = &[
    "assets/empty-tasks-hero.png",
    "assets/empty-tasks-hero@2x.png",
    "assets/empty-tasks-hero@3x.png",
    "assets/android/empty-tasks-hero-mdpi.png",
    "assets/android/empty-tasks-hero-hdpi.png",
    "assets/android/empty-tasks-hero-xhdpi.png",
    "assets/android/empty-tasks-hero-xxhdpi.png",
    "assets/brand-logo.svg",
    "assets/ios/brand-logo.pdf",
    "assets/android/brand-logo.xml",
];

#[test]
fn appendix_c_validates() {
    let file = write_named(APPENDIX_C_LAYOUT_YAML);
    let envelope = run(ValidateMode::Layout, Some(file.path())).expect("run succeeds");
    assert_eq!(envelope["mode"], "layout");
    assert!(errors_array(&envelope).is_empty(), "Appendix C unexpectedly errored: {envelope}");
    assert!(
        warnings_array(&envelope).is_empty(),
        "no warnings expected for Appendix C: {envelope}"
    );
}

#[test]
fn appendix_d_validates() {
    let file = write_named(APPENDIX_D_TOKENS_YAML);
    let envelope = run(ValidateMode::Tokens, Some(file.path())).expect("run succeeds");
    assert_eq!(envelope["mode"], "tokens");
    assert!(errors_array(&envelope).is_empty(), "Appendix D unexpectedly errored: {envelope}");
    assert!(warnings_array(&envelope).is_empty(), "no warnings expected: {envelope}");
}

#[test]
fn appendix_e_validates() {
    let (tmp, assets_path) = write_assets_project(APPENDIX_E_ASSETS_YAML, APPENDIX_E_FILES);
    write_specs_composition(
        tmp.path(),
        // A trimmed composition referencing the same asset ids as
        // Appendix C (icon-button, fab, image, icon items).
        r"version: 1
screens:
  task-list:
    name: Task list
    header:
      title: My tasks
      trailing:
        - icon-button:
            icon: settings
            label: Open settings
    body:
      list:
        each: tasks
        item:
          - group:
              direction: row
              items:
                - icon:
                    name: chevron-right
    fab:
      icon: plus
      label: Add task
    states:
      empty:
        when: tasks.is_empty
        replaces: body
        body:
          - group:
              direction: column
              items:
                - image:
                    name: empty-tasks-hero
  settings:
    name: Settings
    header:
      title: Settings
      leading:
        - icon-button:
            icon: chevron-left
            label: Back
    body:
      form: []
",
    );

    let envelope = run(ValidateMode::Assets, Some(&assets_path)).expect("run succeeds");
    assert_eq!(envelope["mode"], "assets");
    assert!(
        errors_array(&envelope).is_empty(),
        "Appendix E + composition pairing unexpectedly errored: {envelope}"
    );
    let warnings = warnings_array(&envelope);
    assert!(
        warnings
            .iter()
            .any(|w| w["message"].as_str().unwrap_or("").contains("missing optional `xxxhdpi`")),
        "expected a missing-density warning for xxxhdpi: {warnings:?}"
    );
}
