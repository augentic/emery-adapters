# target.build — refactor

Experiment / non-production. Build the page shown in `layout/refactor/ref/` as a new, maintainable Vue 3 app, written the way the Vue Style Guide and common practice say an app should be: `src/Page.vue` plus components in `src/components/`. It must look the same as the reference at every viewport width of `layout/refactor/facts/index.json` `range` and every height of its `height_range`. Capture it, and report what you covered and wrote.

## Inputs

- `$WORKSPACE` — the page's tree, lent writable, in the [tree](references/tree.md)'s layout. Write `src/` through this call's two tools alone: `write_files` writes a file whole at a `/`-separated path relative to `$WORKSPACE`, and `edit_files` changes within a file the tree holds, each edit the one place its `find` matches. The shell installs, serves, lints, and captures, and nothing else: a source file the shell writes, through a redirect, a heredoc, `cp`, or a script, is not yours. A script that measures or compares is fine.
- `layout/refactor/ref/<width>.png` — full-page screenshots of the reference page at the widths named, at the viewport height `index.json` `height`. They show how the page looks, not how it is built.
- `layout/refactor/facts/` — the reference page's measurement at every width, in the [facts](references/facts.md) format, without the document structure: boxes, styles, text and its lines, images, and fonts, and no tag, nesting, or class name. Use it to recover intent, as its last section says.
- [Vue style](references/vue-style.md) — curated recipe for this adapter (CC BY 4.0 attribution inside; not a vendored copy of the upstream guide). Follow it with the Code rules below and pass `pnpm run lint`.
- **The slice** — its plan entry, the specification cut to it, and the design: its `ui-layout` outline is a reading aid, and the facts and the screenshots are the authority.

Nothing outside `$WORKSPACE` is reachable. Decide the markup, the components, and the CSS yourself from what the page shows; do not look for the original page or its source.

## Rules

- **No invented content.** Keep every visible text and its order, every image, every link target (`attrs.href`), and every image description (`attrs.alt`).
- **The host.** `src/main.js` mounts `src/Page.vue` into `<div id="app">`, the only element of an otherwise empty `<body>`, with no other CSS; both stay as the seed wrote them.
- **Assets.** Fonts and images are under `public/facts/assets/`; refer to each as `/facts/assets/<file>`. The page reads nothing under `layout/`.
- **The network.** Only `XDG_RUNTIME_DIR="$TMPDIR" pnpm install --ignore-scripts`, from the npm registry, and the page at `http://127.0.0.1:5173/`. Open no other URL, the Vue documentation included.

## Code

- Follow [Vue style](references/vue-style.md). `src/Page.vue` is the root component (the guide's `App`). Use `<script setup>`.
- `pnpm run lint` passes: eslint-plugin-vue `flat/recommended` (Priority A essentials: multi-word component names, keyed `v-for`, no `v-if` with `v-for`, and related), the name `Page` allowed as a single-word component name, and stylelint `stylelint-config-standard-vue`. The check does not run it; you do, and answer only once it passes.
- Semantic HTML for what each part of the page is — headings, lists, links, buttons, images, landmarks — chosen from what the page shows.
- Components in `src/components/`: one per section, and one for every structure that repeats with the same shape, rendered with `v-for` over data declared in `<script setup>`. Repeated text lives in that data, not in markup.
- `<style scoped>` in every component; a component whose `<style>` is not scoped is refused. Only `html`, `body`, `:root`, and `@font-face` go in a non-scoped `<style>` block of `src/Page.vue`.
- No `position: absolute` and no `position: fixed` under `src/`; a source line that holds one is refused.
- CSS states intent, not measurements:
  - Lay out with flex or grid, `fr`, `%`, `max-width`, `gap`, `auto` margins, and the content's own size. Give an element a px width or height only when it is a fixed design size: an icon, an avatar, a dot.
  - Breakpoints are shared by the whole page.
  - Where a measured value is fractional because it comes from a ratio, write the ratio — a unitless `line-height`, `em`, `fr`, `%` — instead of the px it computes to.
  - Each declaration once: no physical and logical duplicates, nothing that only restates an inherited or initial value.
  - Class names say what the element is.
  - Colours, font families, radii, and spacing used more than once are custom properties on `:root`.
  - One family name per `@font-face`. No static `style` attributes and no `!important`.

## The loop

1. Read the screenshots, then `index.json` and the parts you need with `jq`.
2. Write `src/Page.vue` and the components through `write_files`.
3. Install, serve, and capture as [capture](references/capture.md) says, into `layout/capture/` with screenshots in `layout/capture-shots/`, stamped over `src`; run `pnpm run lint`.
4. Compare `layout/capture-shots/<width>.png` with `layout/refactor/ref/<width>.png` at the same widths, across the whole range and on both sides of each of your breakpoints. A box you give a background, border, or shadow the reference does not paint counts against the match even where it looks the same.

Answer once the capture of the code as it stands raises no page error, the linters pass, and the page looks like the reference. The check scores the capture against the original page's facts. It is one attempt; the turn has at most two. An attempt under the pass mark comes back with `layout/feedback/attempt-<n>.json`, what differs, its ids those of `layout/refactor/facts/`, and the overlays in `layout/review/`: fix the causes in `src/` within the rules below, capture again, and answer again. The check writes `layout/feedback/`; never edit it. The turn ends on its best attempt.

## Fixing differences

- Every revision keeps the code within the Style Guide and every rule under "Code". Where looking like the reference and these rules conflict, the rules win.
- Fix the cause, not the place it shows: change the shared layout, spacing, breakpoint, or component that produces the difference. Do not add a per-element override, a breakpoint, a px size, or an offset that exists only to match the reference at the widths you looked at.
- Take design values — a colour, a font size, a spacing or radius step — from the facts; never measured sizes or positions.
- After each revision, capture and compare again across the whole range, not only at the width you fixed.

## Report

Answer with one JSON object:

- `covered` — each requirement id of the specification above that the page now renders, once each. Leave an id out rather than claim what the page does not show.
- `written` — the files the two tools wrote, once each, as `/`-separated paths relative to `$WORKSPACE`.

## Worked example

A slice `farm-page` (`SLICE-001`) whose specification holds `REQ-001`, the page rendered as recorded; the refactor wrote the root component, one component per section, and one for a repeated card:

```json
{
  "covered": ["REQ-001"],
  "written": [
    "src/Page.vue",
    "src/components/SiteHeader.vue",
    "src/components/ProduceSection.vue",
    "src/components/ProduceCard.vue",
    "src/components/SiteFooter.vue"
  ]
}
```
