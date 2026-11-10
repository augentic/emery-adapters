use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use serde_json::{Map, Value};

use crate::{Error, FactsFolder, image};

const VERSION: u64 = 2;

/// A viewport width in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Width(pub u32);

impl Display for Width {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

/// A viewport: a width, and a height where it is not the facts' own height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// The viewport width.
    pub width: Width,
    /// The viewport height in CSS pixels; `None` is the height the widths were searched at.
    pub height: Option<u32>,
}

impl Viewport {
    /// The viewport `width` wide at the facts' own height.
    #[must_use]
    pub const fn at(width: Width) -> Self {
        Self { width, height: None }
    }

    /// The viewport as a score report names it: `1280`, or `1280x600`.
    #[must_use]
    pub fn label(&self) -> String {
        self.height
            .map_or_else(|| self.width.0.to_string(), |height| format!("{}x{height}", self.width.0))
    }
}

/// The id of a node in the facts: `n<k>` for an element, `t<k>` for a text node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// The id as the facts write it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A box in page coordinates: x, y, width, height.
pub type Rect = [f64; 4];

/// The facts of one page, read from a [`FactsFolder`], answering what was rendered at a [`Viewport`].
#[derive(Debug, Clone)]
pub struct Facts {
    index: Value,
    nodes: BTreeMap<String, Value>,
    // The part file each node is held in.
    files: BTreeMap<String, String>,
    order: Vec<NodeId>,
    // Each asset file's image identity.
    images: BTreeMap<String, String>,
}

impl Facts {
    /// Reads and checks the facts in `folder`: the index, every part it lists, and the tree from its root.
    ///
    /// # Errors
    ///
    /// When the index is missing or of another version, a listed part is
    /// missing or not a JSON object, or a node's child is not in any part.
    pub fn read(folder: &FactsFolder) -> Result<Self, Error> {
        let index = folder.json("index.json")?;
        if index["version"].as_u64() != Some(VERSION) {
            return Err(Error::new(format!(
                "`index.json` is facts version {}, not {VERSION}",
                index["version"]
            )));
        }
        for key in ["range", "height_range"] {
            let pair = index[key]
                .as_array()
                .filter(|pair| pair.len() == 2 && pair.iter().all(Value::is_number));
            if pair.is_none() {
                return Err(Error::new(format!("`index.json`'s `{key}` is not two numbers")));
            }
        }
        let parts =
            index["parts"].as_array().ok_or_else(|| Error::new("`index.json` lists no `parts`"))?;
        let mut nodes = BTreeMap::new();
        let mut files = BTreeMap::new();
        for part in parts {
            let file = part["file"].as_str().ok_or_else(|| {
                Error::new(format!("a part in `index.json` names no file: {part}"))
            })?;
            let Value::Object(held) = folder.json(file)? else {
                return Err(Error::new(format!("`{file}` is not a JSON object of nodes")));
            };
            files.extend(held.keys().map(|id| (id.clone(), file.to_owned())));
            nodes.extend(held);
        }

        let root = index["root"].as_str().unwrap_or("n0").to_owned();
        if !nodes.contains_key(&root) {
            return Err(Error::new(format!("the root `{root}` is in no part")));
        }
        let mut order = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let node = nodes
                .get(&id)
                .ok_or_else(|| Error::new(format!("node `{id}` is a child but in no part")))?;
            let children =
                node["children"].as_array().into_iter().flatten().filter_map(Value::as_str);
            stack
                .extend(children.filter(|child| !child.trim().is_empty()).map(str::to_owned).rev());
            order.push(NodeId(id));
        }
        let images = folder
            .paths()
            .filter(|path| path.starts_with("assets/"))
            .map(|path| (path.to_owned(), image::identity(folder.get(path).unwrap_or_default())))
            .collect();
        Ok(Self {
            index,
            nodes,
            files,
            order,
            images,
        })
    }

    /// The widths the facts cover, low and high.
    #[must_use]
    pub fn width_range(&self) -> (f64, f64) {
        pair(&self.index["range"])
    }

    /// The heights the height sweeps cover, low and high.
    #[must_use]
    pub fn height_range(&self) -> (f64, f64) {
        pair(&self.index["height_range"])
    }

    /// The viewport height the widths were searched at.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.index["height"].as_f64().unwrap_or(0.0)
    }

    /// The browser the facts were captured with, as the capture recorded it:
    /// its `channel` and `version`. `None` for facts that predate the record.
    #[must_use]
    pub fn browser(&self) -> Option<&Value> {
        self.index["search"].get("browser").filter(|browser| !browser.is_null())
    }

    /// The widths the heights were swept at, in order.
    #[must_use]
    pub fn swept_widths(&self) -> Vec<Width> {
        let mut widths: Vec<Width> = self.index["search"]["height_sweeps"]
            .as_object()
            .into_iter()
            .flat_map(Map::keys)
            .filter_map(|key| key.parse().ok().map(Width))
            .collect();
        widths.sort_unstable();
        widths
    }

    /// The node ids depth-first from the root, as the document nests them.
    #[must_use]
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    /// The root element's id.
    #[must_use]
    pub fn root(&self) -> &NodeId {
        &self.order[0]
    }

    pub(crate) fn node(&self, id: &NodeId) -> &Value {
        &self.nodes[&id.0]
    }

    pub(crate) fn file_of(&self, id: &NodeId) -> &str {
        &self.files[&id.0]
    }

    pub(crate) fn node_by_key(&self, id: &str) -> Option<&Value> {
        self.nodes.get(id)
    }

    pub(crate) fn asset(&self, url: &str) -> Option<&str> {
        self.index["assets"][url].as_str()
    }

    // What the asset `file` shows, by its content: its image identity, or
    // its name where the facts do not hold it.
    pub(crate) fn image<'a>(&'a self, file: &'a str) -> &'a str {
        self.images
            .get(file)
            .map_or_else(|| file.rsplit('/').next().unwrap_or(file), String::as_str)
    }

    pub(crate) fn platform_fonts(&self, token: &str) -> &[Value] {
        self.index["fonts"][token]["platform"].as_array().map_or(&[], Vec::as_slice)
    }

    fn default_style(&self, property: &str) -> String {
        self.index["defaults"][property].as_str().map_or_else(String::new, str::to_owned)
    }

    // Whether element `id`'s items can change along the height at `width`:
    // the facts hold it, a pseudo-element of it, or a text node in it apart
    // along the height there.
    pub(crate) fn varies_with_height(&self, id: &NodeId, width: Width) -> bool {
        let key = width.0.to_string();
        let held = |node: &Value| node.get("h").is_some_and(|h| h.get(&key).is_some());
        let node = self.node(id);
        held(node)
            || node["pseudo"].as_object().into_iter().flatten().any(|(_, pseudo)| held(pseudo))
            || node["children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|child| self.node_by_key(child))
                .any(|child| child["tag"] == "#text" && held(child))
    }

    // The height facts of `node` at the viewport's width, where the viewport
    // sets a height: they replace the width facts field by field.
    fn heightwise(node: &Value, viewport: Viewport) -> Option<&Value> {
        viewport.height?;
        node.get("h")?.get(viewport.width.0.to_string())
    }

    // The coordinate a piece is read at: the height where the height facts
    // hold the field, else the width.
    fn coordinate(viewport: Viewport, heightwise: bool) -> f64 {
        match (heightwise, viewport.height) {
            (true, Some(height)) => f64::from(height),
            _ => f64::from(viewport.width.0),
        }
    }

    // The box of `node` at `viewport`, where it is shown with an area.
    pub(crate) fn box_at(node: &Value, viewport: Viewport) -> Option<Rect> {
        let height_box = Self::heightwise(node, viewport).and_then(|h| h.get("box"));
        let x = Self::coordinate(viewport, height_box.is_some());
        let pieces = height_box.or_else(|| node.get("box"))?.as_array()?;
        let piece = pieces
            .iter()
            .filter_map(Value::as_array)
            .find(|piece| num(&piece[0]) <= x && x <= num(&piece[1]))?;
        if num(&piece[10]) == 0.0 {
            return None;
        }
        let rect = [
            linear(num(&piece[2]), num(&piece[6]), x),
            linear(num(&piece[3]), num(&piece[7]), x),
            linear(num(&piece[4]), num(&piece[8]), x),
            linear(num(&piece[5]), num(&piece[9]), x),
        ];
        (rect[2] > 0.0 && rect[3] > 0.0).then_some(rect)
    }

    // The computed `property` of `node` at `viewport`, or the facts' default for it.
    pub(crate) fn style_at(&self, node: &Value, property: &str, viewport: Viewport) -> String {
        let height_value = Self::heightwise(node, viewport).and_then(|h| h["style"].get(property));
        let x = Self::coordinate(viewport, height_value.is_some());
        let Some(value) = height_value.or_else(|| node.get("style").and_then(|s| s.get(property)))
        else {
            return self.default_style(property);
        };
        match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => js_number(number.as_f64().unwrap_or(0.0)),
            Value::Array(runs) => runs
                .iter()
                .filter_map(Value::as_array)
                .find(|run| num(&run[0]) <= x && x <= num(&run[1]))
                .map_or_else(|| self.default_style(property), |run| js_string(&run[2])),
            Value::Object(fitted) => fitted
                .get("px")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_array)
                .find(|piece| num(&piece[0]) <= x && x <= num(&piece[1]))
                .map_or_else(
                    || self.default_style(property),
                    |piece| {
                        piece[2]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|line| {
                                let px = linear(num(&line[0]), num(&line[1]), x);
                                format!("{}px", js_number(js_round(px * 100.0) / 100.0))
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    },
                ),
            Value::Null | Value::Bool(_) => js_string(value),
        }
    }

    // The line boxes of text `node` at `viewport`.
    pub(crate) fn line_rects(node: &Value, viewport: Viewport) -> Vec<Rect> {
        let height_lines = Self::heightwise(node, viewport).and_then(|h| h.get("lines"));
        let x = Self::coordinate(viewport, height_lines.is_some());
        let Some(pieces) = height_lines.or_else(|| node.get("lines")).and_then(Value::as_array)
        else {
            return Vec::new();
        };
        pieces
            .iter()
            .find(|piece| num(&piece["lo"]) <= x && x <= num(&piece["hi"]))
            .and_then(|piece| piece["rects"].as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_array)
            .map(|rect| {
                let at = |k: usize| linear(num(&rect[k][0]), num(&rect[k][1]), x);
                [at(0), at(1), at(2), at(3)]
            })
            .collect()
    }

    // The line breaks of text `node` at `viewport`: offsets into its text, in UTF-16 units.
    pub(crate) fn line_breaks(node: &Value, viewport: Viewport) -> Vec<usize> {
        let height_lines = Self::heightwise(node, viewport).and_then(|h| h.get("lines"));
        let x = Self::coordinate(viewport, height_lines.is_some());
        height_lines
            .or_else(|| node.get("lines"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|piece| num(&piece["lo"]) <= x && x <= num(&piece["hi"]))
            .and_then(|piece| piece["breaks"].as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .filter_map(|offset| usize::try_from(offset).ok())
            .collect()
    }
}

fn pair(value: &Value) -> (f64, f64) {
    (num(&value[0]), num(&value[1]))
}

pub fn num(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

// `a*x+b` rounded twice, as JavaScript computes it, so scores match its float for float.
#[expect(
    clippy::suboptimal_flops,
    reason = "a fused multiply-add rounds once, unlike the scorer it ports"
)]
pub fn linear(a: f64, b: f64, x: f64) -> f64 {
    a * x + b
}

// `Math.round`: halves round up, toward positive infinity.
pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

// A number as JavaScript's `String(n)` writes it, for the values the facts hold.
pub fn js_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if value.is_nan() {
        "NaN".to_owned()
    } else {
        value.to_string()
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => js_number(number.as_f64().unwrap_or(0.0)),
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
