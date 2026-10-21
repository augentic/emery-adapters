# ui-ir behaviour layer

The behaviour layer records what a screen does (routes, triggers, guards,
state, validation) next to what it looks like (composition, tokens, assets).
It holds facts only: every entry is anchored to source code (`file#L`) or to
a recorded run (`replay_digest`). Extraction is deterministic and model-free;
the same inputs give the same bytes.

Path names used below:

- `ea/`: this repository (emery-adapters).
- `ui-ir/`: the ui-ir contract repository (`augentic/ui-ir-experiment`).
- `vx/`: the vx repository (`augentic/vx`).
- `fastapi/`: the FastAPI full-stack template frontend used as sample 1
  (`fastapi-html-capture/fixtures/frontend`).
- `shadcn/`: the shadcn landing page used as sample 2
  (`react-html-capture/fixtures/shadcn-landing-page`).

## 1. Principles

- Behaviour is a separate layer from appearance. Output adapters that only
  draw keep working without reading it.
- The IR holds facts, not summaries or names. "A opens B" is a trigger with
  its own evidence, not a property of A.
- Conversion rules are a hand-written table, the behaviour counterpart of
  `ui-ir/vocab/mappings.json`. Nothing is guessed: a source key without a
  row is reported in `behaviour.unmapped.json`.
- Adapters are independent. The only shared interface is the IR directory.

## 2. Files

| Path | Contents | Schema |
|---|---|---|
| `<ir>/behaviour.json` | routes, triggers, guards, state, validation, transitions | `schema/core/behaviour.schema.json` (`urn:ui-ir:core:behaviour:1`) |
| `x-web` / `x-mobile` / `x-ios` / `x-android` keys inside it | platform-only facts (`target=_blank`, `localStorage` keys, hover) | same schema |
| beside the IR: `behaviour.unmapped.json` | source keys with no table row (`unmapped`) and rows whose value is not a literal (`unconverted`) | same shape as `unmapped.json` |

The schema, the mapping table (`vocab/behaviour-mappings.json`), the node id
rule (`docs/node-id.md`) and the `ui-ir-behaviour` crate (`node_id`,
`validate`, CLI `check` / `coverage`) live in `ea/ui-ir-staging/` until they
move into ui-ir; `ea/ui-ir-staging/README.md` has the move list.

## 3. Shape of `behaviour.json`

- `routes`: screen paths, from the router definitions.
- `triggers[]`: "this gesture on this element causes these effects".
  - `id`: `t-` + the first 12 hex digits of sha256 over the element's
    `file#L`, the `.map` index path and the gesture.
  - `on.gesture`: `tap`, `submit`, `change`, `long_press`, `swipe`.
  - `does[].kind`: `navigate`, `back`, `open_url`, `scroll_to`, `open` /
    `close` / `toggle` (overlays), `set` (a literal into state), `validate`,
    `call` (API request, with `operation.method` / `operation.url` when the
    client definition has a literal URL), `show` (toast or snackbar).
  - `on_success` / `on_failure` exist only inside `call`.
  - `guard`: an early `if (...) return` in the handler.
- `guards[]`: conditions checked before a route is entered, and where they go.
- `state`: overlay open flags, field values, flags. Initial values only when
  they are literals; derived state is not recorded.
- `validation[]`: per-field rules (`required`, `email`, `min_length`,
  `max_length`, `pattern`, `min`, `max`, `url`) with the literal message.
- `transitions`: reserved (`motion.*` in the catalog); always empty for now.
- `evidence` (code, `file#L` relative to the source root) and `observed`
  (recorded runs, with `replay_digest`). Every entry has at least one.

Conditions are not parsed into an expression language. A condition is kept
as the literal source expression (`expr`) with its evidence; anything that
needs interpretation is reported as unconverted.

## 4. Node ids

Behaviour refers to composition nodes by id:
`node_id(screen, path) = screen + "/n-" + hex16(sha256(path))`, where `path`
is the element path that ui-ir `tools/capture/src/collect.ts#L191-L199`
writes to `computed.json`. The rule and test vectors are in
`ea/ui-ir-staging/docs/node-id.md`; the Rust and JavaScript implementations
both test against them.

Composition nodes do not carry this id yet. `ui-ir/sources/web-to-ir`
writes `"event": "TODO(event)"` holes (`sources/web-to-ir/src/extract.rs#L1092`,
`#L1125`) that the behaviour layer is meant to fill once nodes carry ids.

## 5. Source adapters

### 5.1 `code-to-behaviour` (static)

`ui-ir-staging/crates/code-to-behaviour`: reads a React + TypeScript source tree with
oxc and writes `behaviour.json` plus `behaviour.unmapped.json`. See its README
for the reading rules (modules, render tree, evaluator, effects, match tuple).

Code elements are matched to captured elements by the tuple (role, visible
text, href, form field name, inside an overlay). A tuple that does not pick
exactly one element gets `match_status` `none`, `multiple` or `ambiguous`
instead of a guess.

### 5.2 `capture-to-behaviour` (runtime)

`ui-ir-staging/tools/capture-to-behaviour`: runs hand-written scenarios (`fill`,
`click`, ...) with Playwright against a built app served on 127.0.0.1, with
the API stubbed per scenario and every other origin blocked. Each step
records the URL before and after, the requests, and a canonical digest
(RFC 8785) of the step. `apply` attaches the recordings to the static
`behaviour.json` as `observed` entries. The recording model is adapted from
rea-agents (MIT).

Browsers do not run inside an Emery source adapter, so recording stays a
separate tool that writes files into the IR directory.

## 6. Facts and inferred meaning

| Layer | Where | Produces | Model |
|---|---|---|---|
| Facts | ui-ir and its adapters (web-to-ir, `code-to-behaviour`, `capture-to-behaviour`) | `composition.json`, `behaviour.json`, ... with `file#L` or `observed` evidence | none; deterministic |
| Meaning | a separate emery-adapters layer (not implemented) | Emery claims: trigger and flow intent, screen and component purpose, proposals for ambiguous matches | yes; every output is `inferred` |

- The meaning layer reads the fact layer and never rewrites it. The fact
  layer does not know about it.
- `intent` is not part of `behaviour.json`. It belongs to a meaning-layer
  claim that points at node ids, trigger ids and `file#L` evidence.
- The vx behaviour schema (`vx/crates/vx-spec/schemas/behaviour.schema.json`)
  requires `intent`; from the fact layer it is unfilled. vx is not changed.

Claim shape (draft, not implemented):

```json
{
  "version": 1,
  "claims": [
    {
      "id": "c-3f9a1b2c4d5e",
      "kind": "trigger_intent",
      "subject": { "screen": "login", "trigger": "t-0a1b2c3d4e5f", "nodes": ["login/n-cf2331bab5423b71"] },
      "value": "Sign the user in with email and password, then go to the home screen.",
      "status": "inferred",
      "evidence": [
        { "path": "src/routes/login.tsx#L65-L68" },
        { "behaviour": "login/behaviour.json", "pointer": "/triggers/3" }
      ],
      "producer": { "name": "emery-behaviour-semantics", "model": "<model id>", "prompt_digest": "sha256:..." }
    }
  ]
}
```

- `kind`: `trigger_intent`, `flow_intent`, `screen_purpose`,
  `component_purpose`, `match_resolution`.
- `status` is `inferred` for model output; `confirmed` only after a named
  person checks it.
- A claim without evidence is dropped. `match_resolution` never rewrites
  `behaviour.json`.
- `producer` records the model and prompt digest; claims are outside the
  fact layer's determinism check.

## 7. Milestone 1 (fastapi login, shadcn landing)

Run `node ui-ir-staging/tools/capture-to-behaviour/bin/run-samples.mjs <out-dir>`.
It builds both binaries, extracts static facts for every sample in
`samples.json`, records every scenario in `scenarios/<sample>/`, applies the
recordings, validates each `behaviour.json` and writes a coverage report
against the ui-ir web capture when one exists.

Done when:

- The facts listed for both samples appear in `behaviour.json`; the rest is in
  `behaviour.unmapped.json`.
- Two runs give byte-identical output.
- `behaviour.json` validates with zero violations.
- Recorded triggers agree with the code on method, URL and destination.

Not done yet:

- Moving `ui-ir-staging/` into ui-ir (`ui-ir check` covering `behaviour.json`).
- web-to-ir writing node ids and filling `TODO(event)` from triggers.
- vx reading `behaviour.json`.
- An Emery `behaviour` source adapter that joins `code_to_behaviour::evidence`
  claims after its model turns (see `ui-ir-staging/README.md`, step 10).
- The meaning layer (section 6).
