//! Conventional export paths for `vectis materialize assets` (RFC-46 §2, Resolved §7).
//!
//! Paths are relative to the directory containing `assets.yaml` (typically
//! `design-system/`) and use the `assets/exports/<platform>/…` prefix.

use std::path::{Path, PathBuf};

/// Android drawable density buckets for rasterized exports.
pub const ANDROID_DENSITIES: &[&str] = &["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"];

/// iOS raster scales for vector illustration materialize (`@2x` / `@3x` only).
pub const IOS_ILLUSTRATION_SCALES: &[&str] = &["2x", "3x"];

/// iOS raster scales accepted when copying per-density photo masters.
pub const IOS_RASTER_SCALES: &[&str] = &["1x", "2x", "3x"];

/// Android density scale factors relative to the SVG 1× logical canvas (mdpi baseline).
#[must_use]
pub fn android_density_factor(density: &str) -> Option<f32> {
    match density {
        "mdpi" => Some(1.0),
        "hdpi" => Some(1.5),
        "xhdpi" => Some(2.0),
        "xxhdpi" => Some(3.0),
        "xxxhdpi" => Some(4.0),
        _ => None,
    }
}

/// iOS imageset scale factor relative to the SVG 1× logical canvas.
#[must_use]
pub fn ios_scale_factor(scale: &str) -> Option<f32> {
    match scale {
        "1x" => Some(1.0),
        "2x" => Some(2.0),
        "3x" => Some(3.0),
        _ => None,
    }
}

/// iOS imageset PNG filename for a raster export (`1x` omits the `@` suffix).
#[must_use]
pub fn ios_raster_filename(asset_id: &str, scale: &str) -> String {
    if scale == "1x" { format!("{asset_id}.png") } else { format!("{asset_id}@{scale}.png") }
}

/// Design-system-relative path for one iOS raster artifact inside an imageset.
#[must_use]
pub fn ios_raster_artifact_rel(asset_id: &str, scale: &str) -> String {
    format!("{}/{}", ios_imageset_dir(asset_id), ios_raster_filename(asset_id, scale))
}

/// Design-system-relative path for one Android raster drawable PNG.
#[must_use]
pub fn android_raster_artifact_rel(asset_id: &str, density: &str) -> String {
    let snake = kebab_to_snake(asset_id);
    format!("{}/drawable-{density}/{snake}.png", exports_root(Platform::Android))
}

/// Target platform for export path computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// iOS export tree (`Assets.xcassets`, PDF imagesets).
    Ios,
    /// Android export tree (`res/drawable-*`, vector drawables).
    Android,
}

impl Platform {
    /// Lowercase wire token for this platform (`"ios"` / `"android"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Android => "android",
        }
    }

    /// Parse a platform from its lowercase wire token; `None` if unrecognised.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "ios" => Some(Self::Ios),
            "android" => Some(Self::Android),
            _ => None,
        }
    }
}

/// Resolved export layout for one asset platform slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportLayout {
    /// Value recorded in `sources.<platform>` after auto-write (Resolved §7).
    pub pin: String,
    /// Artifact paths materialize will create under `design-system/`.
    pub artifacts: Vec<String>,
}

/// Translate a kebab-case asset id to `snake_case` for Android `R.drawable` names.
#[must_use]
pub fn kebab_to_snake(id: &str) -> String {
    id.replace('-', "_")
}

/// Compute the conventional export layout for auto-materialize from `source:`.
///
/// Returns `None` for roles/kinds that do not auto-convert from a canonical
/// master (`symbol`, `photo`, raster UI icons without `source:`, etc.).
#[must_use]
pub fn export_layout(
    role: &str, kind: &str, platform: Platform, asset_id: &str,
) -> Option<ExportLayout> {
    let materialize_role = resolve_materialize_role(role, kind)?;
    Some(match materialize_role {
        MaterializeRole::IconVector => icon_vector_layout(platform, asset_id),
        MaterializeRole::IllustrationVector => illustration_vector_layout(platform, asset_id),
        MaterializeRole::AppIcon => app_icon_layout(platform),
    })
}

/// Join `assets/exports/<platform>/…` under the design-system root.
#[must_use]
pub fn exports_root(platform: Platform) -> String {
    format!("assets/exports/{}", platform.as_str())
}

/// iOS imageset directory for a kebab-case asset id.
#[must_use]
pub fn ios_imageset_dir(asset_id: &str) -> String {
    format!("{}/{}.imageset", exports_root(Platform::Ios), asset_id)
}

fn resolve_materialize_role(role: &str, kind: &str) -> Option<MaterializeRole> {
    match (role, kind) {
        ("app-icon", _) => Some(MaterializeRole::AppIcon),
        ("icon" | "decorative", "vector") => Some(MaterializeRole::IconVector),
        ("illustration", "vector") => Some(MaterializeRole::IllustrationVector),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializeRole {
    IconVector,
    IllustrationVector,
    AppIcon,
}

fn icon_vector_layout(platform: Platform, asset_id: &str) -> ExportLayout {
    match platform {
        Platform::Ios => {
            let imageset = ios_imageset_dir(asset_id);
            let pdf = format!("{imageset}/{asset_id}.pdf");
            ExportLayout {
                pin: pdf.clone(),
                artifacts: vec![pdf, format!("{imageset}/Contents.json")],
            }
        }
        Platform::Android => {
            let snake = kebab_to_snake(asset_id);
            let xml = format!("{}/drawable/{snake}.xml", exports_root(platform));
            ExportLayout {
                pin: xml.clone(),
                artifacts: vec![xml],
            }
        }
    }
}

fn illustration_vector_layout(platform: Platform, asset_id: &str) -> ExportLayout {
    match platform {
        Platform::Ios => {
            let imageset = ios_imageset_dir(asset_id);
            let mut artifacts = IOS_ILLUSTRATION_SCALES
                .iter()
                .map(|scale| ios_raster_artifact_rel(asset_id, scale))
                .collect::<Vec<_>>();
            let pin = artifacts.last().expect("illustration scales non-empty").clone();
            artifacts.push(format!("{imageset}/Contents.json"));
            ExportLayout { pin, artifacts }
        }
        Platform::Android => {
            let artifacts = ANDROID_DENSITIES
                .iter()
                .map(|density| android_raster_artifact_rel(asset_id, density))
                .collect::<Vec<_>>();
            let pin = artifacts.last().expect("android densities non-empty").clone();
            ExportLayout { pin, artifacts }
        }
    }
}

fn app_icon_layout(platform: Platform) -> ExportLayout {
    match platform {
        Platform::Ios => {
            let root = format!("{}/app-icon/AppIcon.appiconset", exports_root(platform));
            ExportLayout {
                pin: root.clone(),
                artifacts: vec![format!("{root}/Contents.json"), format!("{root}/AppIcon.png")],
            }
        }
        Platform::Android => {
            let root = format!("{}/app-icon", exports_root(platform));
            let mut artifacts = vec![
                format!("{root}/mipmap-anydpi-v26/ic_launcher.xml"),
                format!("{root}/mipmap-anydpi-v26/ic_launcher_round.xml"),
                format!("{root}/values/ic_launcher_background.xml"),
            ];
            for density in ANDROID_DENSITIES {
                artifacts.push(format!("{root}/drawable-{density}/ic_launcher_foreground.png"));
                artifacts.push(format!("{root}/mipmap-{density}/ic_launcher.png"));
            }
            ExportLayout { pin: root, artifacts }
        }
    }
}

/// Resolve a design-system-relative pin path to an absolute path under `assets_dir`.
#[must_use]
pub fn resolve_under_assets_dir(assets_dir: &Path, pin_rel: &str) -> PathBuf {
    assets_dir.join(pin_rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    // `kebab_to_snake`, `ios_scale_factor`, `android_density_factor`, and
    // `ios_raster_filename` are the scalar path helpers; one matrix pins the
    // kebab→snake translation, every iOS/Android scale factor, and the
    // imageset filename suffix convention (`1x` omits `@`).
    #[test]
    fn scale_and_filename_conventions() {
        assert_eq!(kebab_to_snake("onboarding-hero"), "onboarding_hero");
        assert_eq!(kebab_to_snake("settings"), "settings");

        assert_eq!(ios_scale_factor("2x"), Some(2.0_f32));
        assert_eq!(ios_scale_factor("3x"), Some(3.0_f32));
        for (density, factor) in
            [("mdpi", 1.0_f32), ("hdpi", 1.5), ("xhdpi", 2.0), ("xxhdpi", 3.0), ("xxxhdpi", 4.0)]
        {
            assert_eq!(android_density_factor(density), Some(factor), "{density}");
        }

        assert_eq!(ios_raster_filename("hero", "1x"), "hero.png");
        assert_eq!(ios_raster_filename("hero", "2x"), "hero@2x.png");
    }

    // `export_layout` resolves each (role, kind, platform) to its conventional
    // pin + ordered artifact list. The deterministic full-list cases collapse
    // into one table; `decorative/vector` aliases `icon/vector`, and the
    // 13-artifact android app-icon tree is asserted by shape.
    #[test]
    fn export_layout_matrix() {
        struct Case {
            role: &'static str,
            kind: &'static str,
            platform: Platform,
            asset_id: &'static str,
            pin: &'static str,
            artifacts: &'static [&'static str],
        }

        let cases = [
            Case {
                role: "icon",
                kind: "vector",
                platform: Platform::Ios,
                asset_id: "settings",
                pin: "assets/exports/ios/settings.imageset/settings.pdf",
                artifacts: &[
                    "assets/exports/ios/settings.imageset/settings.pdf",
                    "assets/exports/ios/settings.imageset/Contents.json",
                ],
            },
            Case {
                role: "icon",
                kind: "vector",
                platform: Platform::Android,
                asset_id: "chevron-right",
                pin: "assets/exports/android/drawable/chevron_right.xml",
                artifacts: &["assets/exports/android/drawable/chevron_right.xml"],
            },
            Case {
                role: "illustration",
                kind: "vector",
                platform: Platform::Ios,
                asset_id: "onboarding-hero",
                pin: "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png",
                artifacts: &[
                    "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@2x.png",
                    "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png",
                    "assets/exports/ios/onboarding-hero.imageset/Contents.json",
                ],
            },
            Case {
                role: "illustration",
                kind: "vector",
                platform: Platform::Android,
                asset_id: "onboarding-hero",
                pin: "assets/exports/android/drawable-xxxhdpi/onboarding_hero.png",
                artifacts: &[
                    "assets/exports/android/drawable-mdpi/onboarding_hero.png",
                    "assets/exports/android/drawable-hdpi/onboarding_hero.png",
                    "assets/exports/android/drawable-xhdpi/onboarding_hero.png",
                    "assets/exports/android/drawable-xxhdpi/onboarding_hero.png",
                    "assets/exports/android/drawable-xxxhdpi/onboarding_hero.png",
                ],
            },
            Case {
                role: "app-icon",
                kind: "vector",
                platform: Platform::Ios,
                asset_id: "app-icon",
                pin: "assets/exports/ios/app-icon/AppIcon.appiconset",
                artifacts: &[
                    "assets/exports/ios/app-icon/AppIcon.appiconset/Contents.json",
                    "assets/exports/ios/app-icon/AppIcon.appiconset/AppIcon.png",
                ],
            },
        ];

        for case in cases {
            let layout = export_layout(case.role, case.kind, case.platform, case.asset_id)
                .unwrap_or_else(|| panic!("{}/{} {:?}", case.role, case.kind, case.platform));
            assert_eq!(layout.pin, case.pin, "{}/{}", case.role, case.kind);
            assert_eq!(
                layout.artifacts,
                case.artifacts.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "{}/{}",
                case.role,
                case.kind
            );
        }

        // `decorative/vector` aliases `icon/vector` exactly.
        assert_eq!(
            export_layout("decorative", "vector", Platform::Ios, "sparkle"),
            export_layout("icon", "vector", Platform::Ios, "sparkle"),
        );

        // The android app-icon tree fans out to 13 artifacts (anydpi xml +
        // background + per-density foreground/launcher pngs).
        let android = export_layout("app-icon", "raster", Platform::Android, "app-icon")
            .expect("app-icon android");
        assert_eq!(android.pin, "assets/exports/android/app-icon");
        assert_eq!(android.artifacts.len(), 13);
        assert!(android.artifacts.contains(
            &"assets/exports/android/app-icon/mipmap-anydpi-v26/ic_launcher.xml".to_string()
        ));
        assert!(android.artifacts.iter().any(|path| path.contains("drawable-mdpi")));
        assert!(android.artifacts.iter().any(|path| path.contains("mipmap-xxxhdpi")));
    }

    // Roles/kinds without a canonical master (`photo`, `symbol`, raster UI
    // icons) do not auto-convert.
    #[test]
    fn unsupported_roles_return_none() {
        assert!(export_layout("photo", "raster", Platform::Ios, "hero").is_none());
        assert!(export_layout("icon", "symbol", Platform::Ios, "close").is_none());
        assert!(export_layout("icon", "raster", Platform::Android, "badge").is_none());
    }
}
