//! The typed view of `tokens.yaml` the platform renderers consume.
//!
//! Parsing is schema-shaped but deliberately lenient. `vx spec` is the authority on whether a
//! document is valid and has already said so; anything this model cannot interpret is dropped
//! rather than guessed at, so a token that reaches a renderer is always one the renderer can
//! spell.

use serde_json::Value;

use crate::Error;

/// One `colors.<name>` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color {
    /// The kebab-case token name.
    pub name: String,

    /// The `#RRGGBB` light-scheme value.
    pub light: String,

    /// The `#RRGGBB` dark-scheme value.
    pub dark: String,
}

/// One `typefaces.<name>` entry.
///
/// Extraction has always written these — `figma-compile` emits one per distinct family the text
/// styles name — but nothing read them, so every entry in the type scale rendered in whatever the
/// platform's system font happened to be. A design system that states Inter and renders in San
/// Francisco is wrong in the same quiet way a line height thrown away in a comment was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typeface {
    /// The kebab-case token name, as `typography.<style>.typeface` refers to it.
    pub name: String,

    /// The family name to ask the platform for.
    pub family: String,

    /// The font stack to fall back to, as the design system states it, for the shells that take
    /// one. Only CSS does; Swift and Kotlin fall back to the system face whatever this says.
    pub fallback: Option<String>,

    /// How the family is obtained, as `typefaces.<name>.source` states it: `system`,
    /// `google-fonts` or `bundled`. A `system` family is the platform's to supply, so nothing is
    /// planned for download — which is how a web extract's CSS system stack (`-apple-system`,
    /// `system-ui`) reaches a CSS shell as itself rather than as a font to fetch.
    pub source: Option<String>,
}

/// One `typography.<name>` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Typography {
    /// The kebab-case token name.
    pub name: String,

    /// A reference to a `typefaces` token. `None` leaves the platform default.
    pub typeface: Option<String>,

    /// The point or sp size.
    pub size: f64,

    /// A numeric weight on the 100–900 ladder. `None` leaves the platform default.
    pub weight: Option<u16>,

    /// The line height, in the same unit as [`Self::size`].
    pub line_height: Option<f64>,

    /// The tracking.
    pub letter_spacing: Option<f64>,
}

/// One entry of a scalar category: `spacing`, `cornerRadius`, `elevation` or `opacity`.
#[derive(Debug, Clone, PartialEq)]
pub struct Scalar {
    /// The kebab-case token name.
    pub name: String,

    /// The value.
    pub value: f64,
}

/// One `border.<name>` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Border {
    /// The kebab-case token name.
    pub name: String,

    /// The stroke width.
    pub width: f64,

    /// A reference to a `colors` token, when the design system states a default for this width.
    ///
    /// Usually absent. Figma keeps a border's width and its colour in different places, so
    /// extraction reads the colour per layer into `composition.yaml` and leaves this for a border
    /// somebody authored by hand. `vx emit` refuses a composition border that states no colour and
    /// names a width that has none either, so nothing renders an unstated one.
    pub color: Option<String>,

    /// A reference to a `cornerRadius` token.
    pub radius: Option<String>,
}

/// A drop shadow (`tokens.shadow`): an offset, a CSS blur radius and a colour.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// The token key.
    pub name: String,

    /// Offset toward the end edge.
    pub x: f64,

    /// Offset downward.
    pub y: f64,

    /// The CSS blur radius (`SwiftUI`'s `radius` is half of it).
    pub blur: f64,

    /// `#RRGGBB` or `#RRGGBBAA`.
    pub color: String,
}

/// Everything one `tokens.yaml` can be rendered from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Theme {
    /// `colors` entries.
    pub colors: Vec<Color>,

    /// `typography` entries.
    pub typography: Vec<Typography>,

    /// `typefaces` entries.
    pub typefaces: Vec<Typeface>,

    /// `spacing` entries.
    pub spacing: Vec<Scalar>,

    /// `cornerRadius` entries.
    pub corner_radius: Vec<Scalar>,

    /// `elevation` entries.
    pub elevation: Vec<Scalar>,

    /// `opacity` entries.
    pub opacity: Vec<Scalar>,

    /// `border` entries.
    pub border: Vec<Border>,

    /// `offset` entries: signed lengths a stack layer is moved by.
    pub offset: Vec<Scalar>,

    /// `shadow` entries: drop shadows a node's `shadow` names.
    pub shadow: Vec<Shadow>,
}

/// The named weights on the shared 100–900 ladder.
///
/// Both platforms render the same rung under their own spelling: Swift's `.ultraLight` and
/// Kotlin's `FontWeight.Thin` are both 100, so the ladder is the thing the two agree about and
/// the names are what each language calls its rungs.
const LADDER: [(&str, u16); 9] = [
    ("ultra-light", 100),
    ("thin", 200),
    ("light", 300),
    ("regular", 400),
    ("medium", 500),
    ("semibold", 600),
    ("bold", 700),
    ("heavy", 800),
    ("black", 900),
];

impl Theme {
    /// Parses a `tokens.json` document.
    ///
    /// # Errors
    ///
    /// When the source is not a JSON object.
    pub fn parse(source: &str) -> Result<Self, Error> {
        let document: Value = serde_json::from_str(source)
            .map_err(|error| Error::Malformed(format!("tokens.json is not valid JSON: {error}")))?;

        if document.as_object().is_none() {
            return Err(Error::Malformed("tokens.json is not an object".to_owned()));
        }

        Ok(Self {
            colors: colors(&document),
            typography: typography(&document),
            typefaces: typefaces(&document),
            spacing: scalars(&document, "spacing"),
            corner_radius: scalars(&document, "cornerRadius"),
            elevation: scalars(&document, "elevation"),
            opacity: scalars(&document, "opacity"),
            border: borders(&document),
            offset: scalars(&document, "offset"),
            shadow: shadows(&document),
        })
    }

    /// Whether every category is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.colors.is_empty()
            && self.typography.is_empty()
            && self.typefaces.is_empty()
            && self.spacing.is_empty()
            && self.corner_radius.is_empty()
            && self.elevation.is_empty()
            && self.opacity.is_empty()
            && self.border.is_empty()
            && self.offset.is_empty()
            && self.shadow.is_empty()
    }

    /// A colour token by name.
    #[must_use]
    pub fn color(&self, name: &str) -> Option<&Color> {
        self.colors.iter().find(|token| token.name == name)
    }

    /// A typeface token by name.
    #[must_use]
    pub fn typeface(&self, name: &str) -> Option<&Typeface> {
        self.typefaces.iter().find(|token| token.name == name)
    }
}

fn colors(document: &Value) -> Vec<Color> {
    entries(document, "colors")
        .filter_map(|(name, entry)| {
            Some(Color {
                name,
                light: entry.get("light").and_then(Value::as_str)?.to_owned(),
                dark: entry.get("dark").and_then(Value::as_str)?.to_owned(),
            })
        })
        .collect()
}

fn typography(document: &Value) -> Vec<Typography> {
    entries(document, "typography")
        .filter_map(|(name, entry)| {
            Some(Typography {
                name,
                typeface: entry.get("typeface").and_then(Value::as_str).map(str::to_owned),
                size: entry.get("size").and_then(Value::as_f64)?,
                weight: entry.get("weight").and_then(rung),
                line_height: entry.get("lineHeight").and_then(Value::as_f64),
                letter_spacing: entry.get("letterSpacing").and_then(Value::as_f64),
            })
        })
        .collect()
}

fn typefaces(document: &Value) -> Vec<Typeface> {
    entries(document, "typefaces")
        .filter_map(|(name, entry)| {
            Some(Typeface {
                name,
                family: entry.get("family").and_then(Value::as_str)?.to_owned(),
                fallback: entry.get("fallback").and_then(Value::as_str).map(str::to_owned),
                source: entry.get("source").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

fn scalars(document: &Value, category: &str) -> Vec<Scalar> {
    entries(document, category)
        .filter_map(|(name, entry)| {
            Some(Scalar {
                name,
                value: entry.as_f64()?,
            })
        })
        .collect()
}

fn borders(document: &Value) -> Vec<Border> {
    entries(document, "border")
        .filter_map(|(name, entry)| {
            Some(Border {
                name,
                width: entry.get("width").and_then(Value::as_f64)?,
                color: entry.get("color").and_then(Value::as_str).map(str::to_owned),
                radius: entry.get("radius").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

/// One category's entries.
///
/// The maps are ordered, so the emitted order is a function of the token names rather than of
/// where they happen to sit in the YAML — reordering the file changes nothing downstream.
fn shadows(document: &Value) -> Vec<Shadow> {
    entries(document, "shadow")
        .filter_map(|(name, entry)| {
            Some(Shadow {
                name,
                x: entry.get("x").and_then(Value::as_f64)?,
                y: entry.get("y").and_then(Value::as_f64)?,
                blur: entry.get("blur").and_then(Value::as_f64)?,
                color: entry.get("color").and_then(Value::as_str)?.to_owned(),
            })
        })
        .collect()
}

fn entries<'a>(document: &'a Value, category: &str) -> impl Iterator<Item = (String, &'a Value)> {
    document
        .get(category)
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, entry)| (name.clone(), entry))
}

/// Normalises a `weight`, named or numeric, onto the ladder.
fn rung(value: &Value) -> Option<u16> {
    if let Some(name) = value.as_str() {
        return LADDER.iter().find(|(candidate, _)| *candidate == name).map(|(_, rung)| *rung);
    }
    u16::try_from(value.as_u64()?).ok().filter(|rung| (100..=900).contains(rung))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
  "version": 1,
  "colors": {
    "primary": {
      "light": "#0066CC",
      "dark": "#3399FF"
    },
    "outline": {
      "light": "#79747E",
      "dark": "#938F99"
    }
  },
  "typography": {
    "body": {
      "typeface": "inter",
      "size": 16,
      "weight": "regular",
      "lineHeight": 24
    },
    "display": {
      "size": 32,
      "weight": 700
    }
  },
  "typefaces": {
    "inter": {
      "family": "Inter"
    }
  },
  "spacing": {
    "md": 16,
    "xs": 4
  },
  "cornerRadius": {
    "md": 8
  },
  "border": {
    "subtle": {
      "width": 1,
      "color": "outline"
    }
  },
  "opacity": {
    "disabled": 0.38
  }
}
"##;

    #[test]
    fn every_category_parses() {
        let theme = Theme::parse(SAMPLE).expect("the sample parses");

        assert_eq!(theme.colors.len(), 2);
        assert_eq!(theme.color("primary").expect("primary").light, "#0066CC");
        assert_eq!(theme.typography.len(), 2);
        assert_eq!(theme.spacing.len(), 2);
        assert_eq!(theme.border[0].color.as_deref(), Some("outline"));
        assert!(!theme.is_empty());
    }

    #[test]
    fn a_style_keeps_the_typeface_it_names() {
        let theme = Theme::parse(SAMPLE).expect("the sample parses");
        let of = |name: &str| {
            theme.typography.iter().find(|token| token.name == name).expect("a token").clone()
        };

        assert_eq!(of("body").typeface.as_deref(), Some("inter"));
        assert_eq!(theme.typeface("inter").expect("inter").family, "Inter");
        assert_eq!(of("display").typeface, None, "a style may state none");
        assert_eq!(theme.typeface("helvetica"), None);
    }

    #[test]
    fn a_weight_normalises_however_it_was_written() {
        let theme = Theme::parse(SAMPLE).expect("the sample parses");
        let of = |name: &str| {
            theme.typography.iter().find(|token| token.name == name).expect("a token").weight
        };

        assert_eq!(of("body"), Some(400), "named");
        assert_eq!(of("display"), Some(700), "numeric");
    }

    #[test]
    fn the_order_is_the_token_names_rather_than_the_file() {
        let theme = Theme::parse(SAMPLE).expect("the sample parses");
        let names: Vec<&str> = theme.spacing.iter().map(|token| token.name.as_str()).collect();

        assert_eq!(names, ["md", "xs"], "authored xs first");
    }

    #[test]
    fn a_document_with_no_tokens_in_it_is_empty_rather_than_an_error() {
        assert!(Theme::parse(r#"{"version": 1}"#).expect("it parses").is_empty());
    }

    #[test]
    fn a_document_that_is_not_a_mapping_is_refused() {
        assert!(matches!(Theme::parse(r#"["one", "two"]"#), Err(Error::Malformed(_))));
    }
}
