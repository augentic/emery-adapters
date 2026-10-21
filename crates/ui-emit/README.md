# ui-emit

A ui-ir design system (`composition.json`, `tokens.json`, `assets.json`) to a
Vue app, deterministically. Experimental: the first piece of a UI target, on
`experiment/behaviour-layer`.

`ui_emit::design::vue(dir, allow_holes)` validates the directory, derives the
copy and event contracts, and renders the views (`vue/src/views/*Screen.vue`),
`App.vue`, `ui-strings.ts`, `contract.ts`, `assets.ts`, the theme sheet and the
Vite scaffold. `ui_emit::write` and `ui_emit::drift` reconcile those files
with a workspace; a file without the generated header is never overwritten.

## Where it came from

Copied from vx (`augentic/vx`, branch `feat/ui-ir-integration`), `crates/vx-emit`
and `crates/vx-spec`, and trimmed to the deterministic Vue path. For the
fastapi-login fixture the output is byte-identical to
`vx emit --shell vue --allow-holes` (`tests/golden.rs`).

Kept: the composition model, naming, the display-format vocabulary, the copy
contract, the event contract's derivation (`contract::derive`), tokens, the
CSS theme and the Vue views; from vx-spec, holes and the composition's
cross-document checks.

Left behind: the SwiftUI, Compose and Dioxus emitters and themes; the Crux
contract's Rust output (`contract::files`) and the format helpers per shell;
capabilities, services, permissions, wiring and custom elements; vx-spec's
JSON Schema validation (ui-ir owns the schemas: `ui-ir check`), behaviour
vocabulary and `check_agreement` (the contract reports the same disagreement as
a `Clash`), and the `components.yaml` catalogue; asset download, scaffold and
every model stage. Input is JSON only.

## Dependencies

`ui-ir` is a path dependency on the sibling checkout
(`../ui-ir/crates/ui-ir`, default features off), so only the emery-ui layout
resolves it until ui-ir is published or tagged. `serde_json` needs
`preserve_order`: the composition's screen order is the app's (the first screen
is the start route).
