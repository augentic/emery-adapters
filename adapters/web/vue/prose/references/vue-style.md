# Vue style

Curated rules for this adapter's refactor turn. Written for the model; not a
copy of the upstream guide.

**Attribution.** Drawn from the [Vue.js Style Guide](https://vuejs.org/style-guide/)
(CC BY 4.0; Copyright (c) 2019-present, Yuxi (Evan) You and Vue documentation
contributors). Upstream source pinned at
https://github.com/vuejs/docs/tree/40aa88af0094f7bab4aaf786e55c748a6a251d88/src/style-guide.
This turn reaches no network, so those URLs are names only — do not open them.

This document is the adapter's recipe under `prose/references/`, the same place
node and omnia keep layout recipes and python/typescript keep worked examples:
short, linked from the prompt, embedded in `PROSE`. It is not a vendored reprint
of the upstream pages.

## Priority A — essential

Follow each rule. `npm run lint` (eslint-plugin-vue `flat/recommended`) enforces
several; the check also refuses an unscored `<style>` and absolute/fixed
positioning under `src/`.

- **Multi-word component names.** Every component except the root is
  multi-word (`SiteHeader`, `ProduceCard`). The root is `Page` (the guide's
  `App`); the seed allows that single-word name.
- **Keyed `v-for`.** Every `v-for` carries a stable `:key`.
- **No `v-if` with `v-for` on the same element.** Filter in a computed or wrap
  the conditional in a nested element.
- **Scoped styling.** Every component's `<style>` is `scoped`. Only `html`,
  `body`, `:root`, and `@font-face` may sit in a non-scoped `<style>` on
  `src/Page.vue`.
- **Detailed props when props appear.** Prefer typed props (`defineProps` with
  types or a runtime declaration) over undeclared ones.

## Priority B — strongly recommended (this tree)

- **One component per file** under `src/components/`, PascalCase filenames
  (`ProduceCard.vue`).
- **`src/Page.vue` is the root**; `src/main.js` mounts it and is not rewritten.
- **Use `<script setup>`** in every SFC.
- **Self-close** components with no slot content: `<ProduceCard />`.
- **PascalCase** in templates for component tags; **camelCase** prop names in
  script, kebab-case in templates when passed as attributes.
- **Keep template expressions simple**; push logic into script (`computed`,
  methods, data).

## Priority C and D

Prefer the Priority C defaults. Reach for a Priority D feature
(`v-if` / `v-for` on the same element is already forbidden above; `v-html`,
scoped slot misuse, and similar) only when the page's facts require it and no
simpler shape works.

## How this fits the prompt

The prompt's Code section and the seed's lint config are the gate. Where this
recipe and looking like the reference conflict, the prompt's rules win.
