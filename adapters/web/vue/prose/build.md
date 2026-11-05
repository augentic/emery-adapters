# target.build — extract

Experiment / non-production. Write one static Vue 3 single-file component, `src/Page.vue`, that renders the page the facts in `layout/facts/` describe, at every viewport width of their `range` and every height of their `height_range`; capture it, and report what you covered and wrote. A second turn refactors what this one proves; this one is about rendering the page right.

## Inputs

- `$WORKSPACE` — the page's tree, lent writable, in the [tree](references/tree.md)'s layout. Write `src/` through this call's two tools alone: `write_files` writes a file whole at a `/`-separated path relative to `$WORKSPACE`, and `edit_files` changes within a file the tree holds, each edit the one place its `find` matches. The shell installs, serves, and captures, and nothing else: a source file the shell writes, through a redirect, a heredoc, `cp`, or a script, is not yours. A script that measures or compares is fine.
- `layout/facts/` — the original page's facts, in the [facts](references/facts.md) format: every element with its tag, attributes, and text, every box and line, every computed style, across the range.
- **The slice** — its plan entry, the specification cut to it, and the design: the `ui-layout` section outlines the same page as regions, containers, and leaves, and names the facts by their digest. The facts are the authority; the outline is a reading aid.

Nothing outside `$WORKSPACE` is reachable, and nothing in it but `layout/facts/` and `public/facts/assets/` describes the page. Do not look for the original page or its source.

## Rules

- **No invented content.** Keep every element's order, tag, attributes, and text as the facts hold them. Render the children of the original `<body>`; the host is the [tree](references/tree.md)'s `index.html` and `src/main.js`, unchanged.
- **Flex and grid.** Lay the page out with flex or grid, `gap`, margins and padding, and the content's own size. No `position: absolute` and no `position: fixed`, anywhere under `src/`: a source line that holds one is refused. `position: relative` and `sticky` are fine where the original has them.
- **Sizes that follow the viewport.** A size or position that depends on the viewport (a non-zero `a` in the facts) is written with `%`, `vw`, `vh`, `fr`, or `calc()`; a fixed size may be px.
- **Styles.** Styles for `html` and `body` go in a non-scoped `<style>` block of `src/Page.vue`; every `@font-face` the page uses is declared there with the `faces` descriptors as written, its `src` under `/facts/assets/`.
- **Assets.** Fonts and images are under `public/facts/assets/`; refer to each as `/facts/assets/<file>`, the file `index.json` `assets` names for its url. The page reads nothing under `layout/`.
- **The network.** Only `npm install --ignore-scripts`, from the npm registry, and the page at `http://127.0.0.1:5173/`. Open no other URL.

## The loop

1. Read `layout/facts/index.json`, then the parts with `jq`, from the root down.
2. Write `src/Page.vue` through `write_files`.
3. Install, serve, and capture as [capture](references/capture.md) says, into `layout/capture/` with screenshots in `layout/capture-shots/`, stamped over `src`.
4. Read the screenshots and the capture's counts. Fix what differs from the facts through `edit_files`, and capture again.

Answer once the capture of the code as it stands raises no page error and looks like the page. The check assembles that capture and scores it against the original facts; a score under the pass mark comes back with its findings and the overlays in `layout/review/`, and you fix and capture again. Fix the cause the overlay shows, a box laid out wrong or a font not loaded, rather than the one width a finding names.

## Report

Answer with one JSON object:

- `covered` — each requirement id of the specification above that the page now renders, once each. Leave an id out rather than claim what the page does not show.
- `written` — the files the two tools wrote, once each, as `/`-separated paths relative to `$WORKSPACE`: `src/Page.vue`, and any other file under `src/` you split out.

## Worked example

A slice `farm-page` (`SLICE-001`) whose specification holds `REQ-001`, the page rendered as recorded; the extraction wrote one component, its capture scored above the pass mark at every width:

```json
{
  "covered": ["REQ-001"],
  "written": ["src/Page.vue"]
}
```
