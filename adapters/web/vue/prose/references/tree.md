# The tree

Experiment / non-production. One page, rebuilt as a Vue 3 app served by Vite.

## Layout

- `index.html`, `src/main.js` — the host: `src/main.js` mounts `src/Page.vue` with `createApp(Page).mount('#app')` into `<div id="app">`, the only element of an otherwise empty `<body>`, with no other CSS.
- `src/Page.vue` — the page's root component, and in the refactor `src/components/*.vue` beside it.
- `public/facts/assets/` — the page's fonts and images, served at `/facts/assets/<file>`. The adapter writes them before the first turn; never change them.
- `layout/capture.mjs` — the capture script; [capture](capture.md) says how to run it.
- `layout/` — everything else the adapter and the captures put there: the facts a turn reads, the captures, the scores and their overlays. The page never reads it: Vite refuses to serve it, and a source file that names `layout/` is refused.
- `package.json`, `vite.config.js`, `eslint.config.js`, `stylelint.config.js`, `.gitignore` — scaffolded when the repository was initialised.

`layout/capture.mjs` and every file below but `.gitignore` are held byte for byte to what the seed wrote: a turn whose tree changed one is refused, naming it. Nothing is added to `package.json`; `npm install --ignore-scripts` installs what it names.

`package.json`:

```json
{
  "name": "page",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1 --port 5173 --strictPort",
    "lint": "eslint src && stylelint \"src/**/*.vue\""
  },
  "dependencies": {
    "vue": "3.5.44"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "6.0.10",
    "eslint": "10.14.0",
    "eslint-plugin-vue": "10.12.0",
    "playwright": "1.63.0",
    "stylelint": "17.16.0",
    "stylelint-config-standard-vue": "2.0.0",
    "vite": "8.3.5",
    "vue-eslint-parser": "10.4.1"
  }
}
```

`vite.config.js`:

```js
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue()],
  server: { fs: { deny: [".env", ".env.*", "*.{crt,pem}", "**/.git/**", "**/layout/**"] }, watch: { ignored: ["**/layout/**"] } },
});
```

`index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Page</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.js"></script>
  </body>
</html>
```

`src/main.js`:

```js
import { createApp } from "vue";

import Page from "./Page.vue";

createApp(Page).mount("#app");
```

`eslint.config.js`:

```js
import pluginVue from "eslint-plugin-vue";

export default [
  ...pluginVue.configs["flat/recommended"],
  { files: ["src/Page.vue"], rules: { "vue/multi-word-component-names": "off" } },
];
```

`stylelint.config.js`:

```js
export default { extends: ["stylelint-config-standard-vue"] };
```

`.gitignore`:

```text
node_modules/
dist/
.emery/
layout/facts/
layout/refactor/
layout/capture/
layout/capture-shots/
layout/review/
layout/verify/capture/
```

## Around each turn

- Before the extraction, the adapter clears `src/` but `src/main.js` and writes the page's facts to `layout/facts/`.
- Before the refactor, it removes `layout/facts/`, the extracted code, and the extraction's captures, and writes the facts without their document structure to `layout/refactor/facts/` and the extracted page's screenshots to `layout/refactor/ref/`.
- After the refactor, it writes the original facts to `layout/original/`, which the verification scores against.
