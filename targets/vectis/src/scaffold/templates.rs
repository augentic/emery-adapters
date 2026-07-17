//! Template placeholder substitution and capability-conditional sections.

pub(super) mod registry;

use super::PlannedFile;

pub(super) struct TemplateEntry {
    pub target: &'static str,
    pub contents: &'static str,
    pub path_mode: PathMode,
    pub include_when: IncludeWhen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathMode {
    ContentOnly,
    AppName,
    AndroidPackage,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum IncludeWhen {
    Always,
    AnyOf(&'static [Capability]),
}

impl IncludeWhen {
    pub(super) fn should_include(self, caps: &[Capability]) -> bool {
        match self {
            Self::Always => true,
            Self::AnyOf(needed) => needed.iter().any(|c| caps.contains(c)),
        }
    }
}

pub(super) fn plan_assembly(
    entries: &[TemplateEntry], params: &Params, caps: &[Capability],
) -> Vec<PlannedFile> {
    let android_package_path = params.android_package.replace('.', "/");
    entries
        .iter()
        .filter(|entry| entry.include_when.should_include(caps))
        .map(|entry| {
            let relative_path = match entry.path_mode {
                PathMode::ContentOnly => entry.target.to_string(),
                PathMode::AppName => substitute_path(entry.target, params),
                PathMode::AndroidPackage => {
                    substitute_path_with(entry.target, params, Some(&android_package_path))
                }
            };
            PlannedFile {
                relative_path,
                contents: render(entry.contents, params, caps),
            }
        })
        .collect()
}

/// Capability enabled via `--caps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// HTTP effects.
    Http,
    /// Key-value storage effects.
    Kv,
    /// Time effects.
    Time,
    /// Platform effects.
    Platform,
    /// Server-sent events support.
    Sse,
}

impl Capability {
    /// Marker tag in templates and on `--caps`.
    #[must_use]
    pub const fn marker_tag(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Kv => "kv",
            Self::Time => "time",
            Self::Platform => "platform",
            Self::Sse => "sse",
        }
    }

    /// Parse a user-facing tag.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "http" => Some(Self::Http),
            "kv" => Some(Self::Kv),
            "time" => Some(Self::Time),
            "platform" => Some(Self::Platform),
            "sse" => Some(Self::Sse),
            _ => None,
        }
    }
}

pub(super) struct Params {
    pub app_name: String,
    pub app_struct: String,
    pub app_name_lower: String,
    pub android_package: String,
    pub crux_core_version: String,
    pub crux_http_version: String,
    pub crux_kv_version: String,
    pub crux_time_version: String,
    pub crux_platform_version: String,
    pub facet_version: String,
    pub serde_version: String,
    pub uniffi_version: String,
    pub agp_version: String,
    pub kotlin_version: String,
    pub compose_bom_version: String,
    pub ktor_version: String,
    pub koin_version: String,
    pub android_ndk_version: String,
}

pub(super) fn render(template: &str, params: &Params, caps: &[Capability]) -> String {
    let stripped = process_caps(template, caps);
    substitute_placeholders(&stripped, params)
}

pub(super) fn substitute_path(target: &str, params: &Params) -> String {
    substitute_path_with(target, params, None)
}

pub(super) fn substitute_path_with(
    target: &str, params: &Params, android_package_path: Option<&str>,
) -> String {
    let mut out = target.to_string();
    if let Some(pkg_path) = android_package_path {
        out = out.replace("__ANDROID_PACKAGE_PATH__", pkg_path);
    }
    out.replace("__APP_NAME_LOWER__", &params.app_name_lower)
        .replace("__APP_NAME__", &params.app_name)
}

fn substitute_placeholders(input: &str, params: &Params) -> String {
    input
        .replace("__APP_NAME_LOWER__", &params.app_name_lower)
        .replace("__APP_NAME__", &params.app_name)
        .replace("__APP_STRUCT__", &params.app_struct)
        .replace("__ANDROID_PACKAGE__", &params.android_package)
        .replace("__CRUX_CORE_VERSION__", &params.crux_core_version)
        .replace("__CRUX_HTTP_VERSION__", &params.crux_http_version)
        .replace("__CRUX_KV_VERSION__", &params.crux_kv_version)
        .replace("__CRUX_TIME_VERSION__", &params.crux_time_version)
        .replace("__CRUX_PLATFORM_VERSION__", &params.crux_platform_version)
        .replace("__FACET_VERSION__", &params.facet_version)
        .replace("__SERDE_VERSION__", &params.serde_version)
        .replace("__UNIFFI_VERSION__", &params.uniffi_version)
        .replace("__AGP_VERSION__", &params.agp_version)
        .replace("__KOTLIN_VERSION__", &params.kotlin_version)
        .replace("__COMPOSE_BOM_VERSION__", &params.compose_bom_version)
        .replace("__KTOR_VERSION__", &params.ktor_version)
        .replace("__KOIN_VERSION__", &params.koin_version)
        .replace("__ANDROID_NDK_VERSION__", &params.android_ndk_version)
}

fn process_caps(input: &str, caps: &[Capability]) -> String {
    let newline = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = input.ends_with('\n') || input.ends_with("\r\n");
    let mut out = String::with_capacity(input.len());
    let mut active: Option<&str> = None;
    let mut wrote_first = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if let Some(tag) = trimmed.strip_prefix("<<<CAP:") {
            if tag.is_empty() || tag.contains(char::is_whitespace) || active.is_some() {
                push_line(&mut out, &mut wrote_first, newline, line);
                continue;
            }
            active = Some(static_tag(tag));
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("CAP:")
            && let Some(tag) = rest.strip_suffix(">>>")
            && let Some(open_tag) = active
            && tag == open_tag
        {
            active = None;
            continue;
        }

        if let Some(open_tag) = active {
            if cap_selected(open_tag, caps) {
                push_line(&mut out, &mut wrote_first, newline, line);
            }
        } else {
            push_line(&mut out, &mut wrote_first, newline, line);
        }
    }

    if trailing_newline {
        out.push_str(newline);
    }
    out
}

fn push_line(out: &mut String, wrote_first: &mut bool, newline: &str, line: &str) {
    if *wrote_first {
        out.push_str(newline);
    }
    out.push_str(line);
    *wrote_first = true;
}

fn cap_selected(tag: &str, caps: &[Capability]) -> bool {
    caps.iter().any(|c| c.marker_tag() == tag)
}

fn static_tag(tag: &str) -> &'static str {
    match tag {
        "http" => "http",
        "kv" => "kv",
        "time" => "time",
        "platform" => "platform",
        "sse" => "sse",
        _ => "__unknown__",
    }
}
