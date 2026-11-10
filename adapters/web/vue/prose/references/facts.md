# Facts

The facts are what a browser rendered of the original page, losslessly, across a range of viewport widths and heights: every element's box, every text node's line boxes, computed styles, fonts, and assets. Read `index.json` first, then the parts you need with `jq`; some parts are large.

## The index

`index.json` holds:

- `range` — the widths swept, `[lo, hi]`; `height` — the viewport height the width facts were taken at; `height_range` — the heights swept.
- `defaults` — the computed value per style property, used where a node carries none.
- `faces` — the page's `@font-face` rules, every descriptor as written, each `src` url pointing at `assets/<file>`.
- `fonts` — font tokens: the computed `family`, `weight`, and `style`, and `platform`, the fonts the browser drew them with.
- `assets` — url to file under `assets/`. The tree serves the same files at `/facts/assets/<file>`; refer to them by that path.
- `parts` — the part files, each `file` a path in the facts folder.
- `counts`, `search`, `version` — how the capture ran; nothing on the page.

Only in the full facts, the extraction's:

- `ax` — the accessibility tree, a file in `ax/` per width range `[from, to, file]`; `ax_files` the files it names, and `ax_h` the same along the height where it changes.
- `root` — the root node; `containers` — the nodes split into parts, with `tag`, `path`, `parent`, and `children`.

## A node

A part file maps node id to node. A node has `kind` (`box`, `text`, `svg`, `br`, …) and:

- `box` — pieces `[lo, hi, ax, ay, aw, ah, bx, by, bw, bh, shown]`: for viewport width W in `[lo, hi]`, the border box is x = ax·W + bx, y = ay·W + by, width = aw·W + bw, height = ah·W + bh, in page coordinates at height `height`; `shown` 0 means not rendered.
- `style` — the computed style per property: a value; or runs `[[from, to, value], …]` by width; or `{"px": [[from, to, [[a, b], …]], …]}`, each px number a·W + b.
- `sizing` — `computedStyleMap()` values, which keep `%`, `auto`, and `calc()`; `vw`, `vh`, and `em` arrive as px.
- `attrs` — attributes, as runs where they change with width. `font` — the node's font token.
- `pseudo` — `::before`, `::after`, and `::marker`, each with `box` and `style`.
- `h` — `{"<width>": {box | style | sizing | lines …}}`: the same facts along the viewport height H at that width (a·H + b), only where they change with height.
- `extra` — content facts: an image's `file`, inline SVG markup, form values.
- A text node (`kind` `text`) has `text`, `parent` (the node whose style the text has), and `lines`: pieces `{lo, hi, lines, breaks, rects}` with one `[[a, b] ×4]` rect (x, y, width, height = a·W + b) per line and the character offsets where lines break.

In the full facts an element also has `tag`, `path` (its CSS path in the original), `parent`, and `children` (node ids in document order, `" "` for whitespace between them), and `attrs` holds every attribute.

## Without the document structure

The refactor reads the facts stripped of how the original was built. Every node loses `tag`, `path`, and `children`, and an element its `parent`; `attrs` keeps `href` and `alt` alone; the index loses `ax`, `ax_files`, `ax_h`, `root`, and `containers`, and each part is its `file` alone. Boxes, text and its lines, styles, sizing, fonts, faces, and assets stay. They show where everything lands and what it looks like, not which element it was.

## Reading intent

Boxes side by side that each grow by 1/n px per px of viewport are n equal columns. A width that stops growing at some viewport width is a `max-width`. A value that jumps at one width is a breakpoint. A non-zero `a` is a size that depends on the viewport.
