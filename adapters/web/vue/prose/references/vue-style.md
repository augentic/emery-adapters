# Vue style

**Attribution.** Drawn from the [Vue.js Style Guide](https://vuejs.org/style-guide/)
(CC BY 4.0; Copyright (c) 2019-present, Yuxi (Evan) You and Vue documentation
contributors). Upstream source pinned at
https://github.com/vuejs/docs/tree/40aa88af0094f7bab4aaf786e55c748a6a251d88/src/style-guide.
This turn reaches no network, so those URLs are names only: do not open them.

## Priority A: essential

- **Multi-word component names.** Every component except the root is
  multi-word (`SiteHeader`, `ProduceCard`).
- **Detailed prop definitions.** Declare props with types (`defineProps` with
  types or a runtime declaration), never as a bare list of names.
- **Keyed `v-for`.** Every `v-for` carries a stable `:key`.
- **No `v-if` with `v-for` on the same element.** Filter in a computed, or move
  the condition to a wrapping element.
- **Component-scoped styling.** Scope each component's styles.

## Priority B: strongly recommended

- **One component per file**, with PascalCase filenames (`ProduceCard.vue`).
- **Self-close** components that have no content: `<ProduceCard />`.
- **PascalCase** component tags in templates.
- **camelCase** prop names in script, kebab-case when passed in templates.
- **Simple template expressions.** Move logic into `computed` or functions in
  script.

## Priority C: recommended

- **Component options order.** Keep the parts of a component in a consistent
  order.
- **Element attribute order.** Keep attributes in a consistent order
  (definition, list rendering, conditionals, identifiers, other attributes,
  events, content).
- **Empty lines** between multi-line component options where they help
  reading.
- **SFC top-level order.** `<script>`, then `<template>`, then `<style>`, in
  every file.

## Priority D: use with caution

- **Element selectors with `scoped`.** Prefer class selectors; element
  selectors in scoped styles are slower.
- **Implicit parent-child communication.** Use props and events, not
  `this.$parent` or mutating props.

## This adapter's rules

These come from this adapter's Code rules and check, not from the upstream
guide.

- The root component is `src/Page.vue` (the guide's `App`), mounted by
  `src/main.js`, which is not rewritten. It may keep its single-word name.
- Components live one per file under `src/components/` and use
  `<script setup>`.
- Every component's `<style>` is `scoped`. Only `html`, `body`, `:root`, and
  `@font-face` may sit in a non-scoped `<style>`, and only in `src/Page.vue`.
- Nothing under `src/` uses `position: absolute` or `position: fixed`.
- `npm run lint` (eslint-plugin-vue `flat/recommended`) passes.

Where this document and the prompt's Code rules disagree, the Code rules win.
