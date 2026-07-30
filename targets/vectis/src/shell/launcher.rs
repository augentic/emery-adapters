//! Shell-resident launcher icon probes.

use std::path::Path;

/// Returns whether the on-disk shell for `platform` already carries a
/// satisfiable launcher icon (shell-resident escape hatch).
#[must_use]
pub fn shell_resident_app_icon(project_dir: &Path, platform: &str) -> bool {
    match platform {
        "ios" => ios_shell_resident_app_icon(project_dir),
        "android" => android_shell_resident_app_icon(project_dir),
        _ => false,
    }
}

fn ios_shell_resident_app_icon(project_dir: &Path) -> bool {
    super::ios_xcassets_roots(project_dir)
        .into_iter()
        .any(|xcassets| ios_appiconset_satisfied(&xcassets.join("AppIcon.appiconset")))
}

fn ios_appiconset_satisfied(appiconset_dir: &Path) -> bool {
    let contents_path = appiconset_dir.join("Contents.json");
    if !contents_path.is_file() {
        return false;
    }
    let Ok(contents) = std::fs::read_to_string(&contents_path) else {
        return false;
    };
    referenced_png_filenames(&contents).into_iter().any(|name| appiconset_dir.join(name).is_file())
}

fn referenced_png_filenames(contents_json: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = contents_json[search_from..].find("\"filename\"") {
        let rest = &contents_json[search_from + rel..];
        if let Some(filename) = parse_json_string_value_after_key(rest, "filename")
            && Path::new(&filename).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            out.push(filename);
        }
        search_from += rel + 1;
    }
    out
}

fn parse_json_string_value_after_key(fragment: &str, key: &str) -> Option<String> {
    let key_pattern = format!("\"{key}\"");
    let key_start = fragment.find(&key_pattern)?;
    let after_key = &fragment[key_start + key_pattern.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let open = after_colon.find('"')?;
    let value_start = &after_colon[open + 1..];
    let close = value_start.find('"')?;
    let value = &value_start[..close];
    if value.is_empty() { None } else { Some(value.to_owned()) }
}

fn android_shell_resident_app_icon(project_dir: &Path) -> bool {
    let res = project_dir.join("Android/app/src/main/res");
    if res.join("mipmap-anydpi-v26/ic_launcher.xml").is_file() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(&res) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("mipmap-") && entry.path().join("ic_launcher.png").is_file() {
            return true;
        }
    }
    false
}
