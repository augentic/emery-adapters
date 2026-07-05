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
