# target.build — extract

Experiment / non-production. Make one static Vue 3 single-file component, `src/Page.vue`, that renders the page the facts in `layout/facts/` describe, at every viewport width of their `range` and every height of their `height_range`; capture it, and report what you covered and wrote. A second turn refactors the best of this turn's attempts; this one is about rendering the page right.

How you build and check `src/Page.vue` is up to you: write it through `write_files` and `edit_files`, or generate it from the facts with tooling of your own. Keep any tooling you add under `tools/`, and run it from the shell; it is gone before the refactor.

## Inputs

- `$WORKSPACE` — the page's tree, lent writable, in the [tree](references/tree.md)'s layout. `write_files` writes a file whole at a `/`-separated path relative to `$WORKSPACE`, and `edit_files` changes within a file the tree holds, each edit the one place its `find` matches. The shell installs, serves, captures, and runs your tooling.
   10|- `layout/facts/` — the original page's facts, in the [facts](references/facts.md) format: every element with its tag, attributes, and text, every box and line, every computed style, across the range.
- **The slice** — its plan entry, the specification cut to it, and the design: the `ui-layout` section outlines the same page as regions, containers, and leaves, and names the facts by their digest. The facts are the authority; the outline is a reading aid.

Nothing outside `$WORKSPACE` is reachable, and nothing in it but `layout/facts/` and `public/facts/assets/` describes the page. Do not look for the original page or its source.

## Rules

- **No invented content.** Keep every element's order, tag, attributes, and text as the facts hold them.
- **Sizes that follow the viewport.** A size or position that depends on the viewport (a non-zero `a` in the facts) is written with `%`, `vw`, `vh`, or `calc()`; a fixed size may be px.
- **The host.** `src/main.js` mounts `src/Page.vue` with `createApp(Page).mount('#app')` into `<div id="app">`, the only element of an otherwise empty `<body>`, with no other CSS; it and `index.html` stay as the seed wrote them. Render the children of the original `<body>`. Styles for `html` and `body` go in a non-scoped `<style>` block.
   20|- **Assets.** Fonts and images are under `public/facts/assets/`; refer to each as `/facts/assets/<file>`. The page reads nothing under `layout/`.
- **The facts.** Read `layout/facts/index.json` first, then the parts you need with `jq`; some are large.
- **The network.** Only `npm install --ignore-scripts`, from the npm registry, and the page at `http://127.0.0.1:5173/`. Open no other URL.

## Attempts

You are done only when you have seen `src/Page.vue` render in a browser under the host above without compile or runtime errors, and it shows the page. Capture it then as [capture](references/capture.md) says, into `layout/capture/` with screenshots in `layout/capture-shots/`, stamped over `src`, and answer.

The check scores that capture against the original facts. It is one attempt; the turn has at most two. An attempt under the pass mark comes back with `layout/feedback/attempt-<n>.json`, what differs from the facts, and the overlays in `layout/review/`: fix the causes in `src/`, capture again, and answer again. Fix the cause an entry shows, a box laid out wrong or a font not loaded, rather than the one width it names. The check writes `layout/feedback/`; never edit it. The turn ends on its best attempt.

## Report
   30|
Answer with one JSON object:

- `covered` — each requirement id of the specification above that the page now renders, once each. Leave an id out rather than claim what the page does not show.
- `written` — every file under `src/` you wrote or generated, and any file under `tools/`, once each, as `/`-separated paths relative to `$WORKSPACE`.

## Worked example

A slice `farm-page` (`SLICE-001`) whose specification holds `REQ-001`, the page rendered as recorded; the extraction generated one component from the facts with a script of its own:

   40|```json
{
  "covered": ["REQ-001"],
  "written": ["src/Page.vue", "tools/generate.mjs"]
}
```
