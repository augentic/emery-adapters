//! Shell catalog completeness probes.

use std::path::Path;

use tempfile::tempdir;
use vectis::verify::catalog_findings;

fn write_yaml(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir parents");
    }
    std::fs::write(path, content).expect("write yaml");
}

fn scaffold_project(root: &Path) {
    let emery = root.join(".emery");
    std::fs::create_dir_all(emery.join("specs")).expect("mkdir specs");
    write_yaml(
        &emery.join("project.yaml"),
        "name: test-app\nadapter: vectis\nemery_version: '2.0'\nplatforms:\n  - core\n  - ios\n  - android\n",
    );
    let shared = root.join("shared/src");
    std::fs::create_dir_all(&shared).expect("mkdir shared");
    std::fs::write(shared.join("app.rs"), "pub struct App;").expect("write app.rs");
    let ios = root.join("iOS/TodoApp");
    std::fs::create_dir_all(ios.join("Assets.xcassets")).expect("mkdir ios");
    std::fs::write(ios.join("ContentView.swift"), "struct ContentView {}").expect("write swift");
    let android = root.join("Android/app/src/main/kotlin/com/test");
    std::fs::create_dir_all(&android).expect("mkdir android");
    std::fs::write(android.join("MainActivity.kt"), "class MainActivity").expect("write kt");
    std::fs::create_dir_all(root.join("Android/app/src/main/res")).expect("mkdir res");
}

fn write_inventory(root: &Path) {
    write_yaml(
        &root.join("design-system/assets.yaml"),
        r"
version: 1
assets:
  empty-tasks-hero:
    kind: vector
    role: illustration
    source: assets/empty-tasks-hero.svg
    sources:
      ios: assets/exports/ios/empty-tasks-hero.imageset/empty-tasks-hero@3x.png
      android: assets/exports/android/drawable-xxxhdpi/empty_tasks_hero.png
  chevron-right:
    kind: symbol
    role: icon
    symbols:
      ios: chevron.right
      android: chevron_right
",
    );
    write_yaml(
        &root.join(".emery/specs/composition.yaml"),
        r"
version: 1
screens:
  empty:
    body:
      - image:
          name: empty-tasks-hero
",
    );
}

// iOS imageset branches covered in `verify.rs`; matrix below hits catalog-only cases.
#[test]
fn contents_json_only_imageset() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_inventory(tmp.path());

    let imageset = tmp.path().join("iOS/TodoApp/Assets.xcassets/empty-tasks-hero.imageset");
    std::fs::create_dir_all(&imageset).expect("mkdir imageset");
    std::fs::write(imageset.join("Contents.json"), "{\"images\":[]}").expect("write json");

    let findings = catalog_findings(tmp.path(), &["ios".to_string()]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["id"], "shell-catalog-entry-missing");
}

#[test]
fn exemplar_layout_pdf_imageset_present() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_inventory(tmp.path());

    let imageset = tmp.path().join("iOS/TodoApp/Assets.xcassets/empty-tasks-hero.imageset");
    std::fs::create_dir_all(&imageset).expect("mkdir imageset");
    std::fs::write(imageset.join("Contents.json"), "{\"images\":[]}").expect("write json");
    std::fs::write(imageset.join("empty-tasks-hero.pdf"), b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\n")
        .expect("write pdf");

    let findings = catalog_findings(tmp.path(), &["ios".to_string()]);
    assert!(findings.is_empty(), "exemplar layout with PDF magic must satisfy catalog verify");
}

#[test]
fn resources_prefix_not_accepted() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_inventory(tmp.path());

    // Hard cut: only `iOS/<App>/Assets.xcassets/` counts — a lone
    // `Resources/Assets.xcassets/` tree elsewhere is ignored.
    let imageset =
        tmp.path().join("iOS/TodoApp/Resources/Assets.xcassets/empty-tasks-hero.imageset");
    std::fs::create_dir_all(&imageset).expect("mkdir imageset");
    std::fs::write(imageset.join("Contents.json"), "{\"images\":[]}").expect("write json");
    std::fs::write(imageset.join("empty-tasks-hero.pdf"), b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\n")
        .expect("write pdf");

    let findings = catalog_findings(tmp.path(), &["ios".to_string()]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["id"], "shell-catalog-entry-missing");
}

#[test]
fn symbols_skipped() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_yaml(
        &tmp.path().join("design-system/assets.yaml"),
        r"
version: 1
assets:
  chevron-right:
    kind: symbol
    role: icon
    symbols:
      ios: chevron.right
      android: chevron_right
",
    );
    write_yaml(
        &tmp.path().join(".emery/specs/composition.yaml"),
        r"
version: 1
screens:
  list:
    body:
      - icon-button:
          icon: chevron-right
",
    );

    let findings = catalog_findings(tmp.path(), &["ios".to_string()]);
    assert!(findings.is_empty());
}

#[test]
fn android_vector_icon_missing() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_yaml(
        &tmp.path().join("design-system/assets.yaml"),
        r"
version: 1
assets:
  settings:
    kind: vector
    role: icon
    source: assets/settings.svg
",
    );
    write_yaml(
        &tmp.path().join(".emery/specs/composition.yaml"),
        r"
version: 1
screens:
  home:
    body:
      - icon-button:
          icon: settings
",
    );

    let findings = catalog_findings(tmp.path(), &["android".to_string()]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["id"], "shell-catalog-entry-missing");
    assert!(findings[0]["message"].as_str().unwrap().contains("drawable/settings.xml"));
}

#[test]
fn android_density_raster_missing() {
    let tmp = tempdir().unwrap();
    scaffold_project(tmp.path());
    write_inventory(tmp.path());

    // `empty-tasks-hero` is a `role: illustration` vector, so the android
    // catalog probe falls through the vector-drawable arm to the density-raster
    // search; with no `res/drawable-<density>/empty_tasks_hero.png` on disk it
    // exhausts every density and reports the entry missing (the
    // `android_shell_has_density_raster` no-match return).
    let findings = catalog_findings(tmp.path(), &["android".to_string()]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["id"], "shell-catalog-entry-missing");
    let message = findings[0]["message"].as_str().unwrap();
    assert!(message.contains("empty-tasks-hero"));
    assert!(message.contains("drawable-<density>"));
}
