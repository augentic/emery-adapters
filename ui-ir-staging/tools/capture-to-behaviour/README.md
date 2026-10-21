# capture-to-behaviour

Runs scripted Playwright sessions against a built app and attaches what the
runs showed to a `behaviour.json` from `code-to-behaviour`. Deterministic
and model-free: two runs of the same scenario give the same recording bytes.

```
node bin/capture-to-behaviour.mjs check   --scenario <file> [--scenario <file>]...
node bin/capture-to-behaviour.mjs record  --scenario <file> --out <recording.json> [--chromium <path>]
node bin/capture-to-behaviour.mjs apply   --behaviour <behaviour.json> --recording <file>... --out <behaviour.json>
node bin/capture-to-behaviour.mjs compare --recording <left.json> --recording <right.json> [--mode digest|behaviour]
node bin/run-samples.mjs <out-dir>
```

`run-samples` builds `code-to-behaviour` and `ui-ir-behaviour`, then for
each entry of `samples.json`: static extract, `record` of every
`scenarios/<sample>/*.json`, `apply`, `ui-ir-behaviour check`, and
`coverage` when a ui-ir capture exists; it writes `<out-dir>/summary.json`.

`compare --mode digest` (default) compares `replay_digest`, so any DOM or
accessibility-tree change is `changed`. `--mode behaviour` is for a port to
another framework: per step it compares `url_after`, API requests (requests
with an origin; page and asset loads ignored), request body shape (content
type and field names, not the hash), `texts_appeared` (as a set) and
`page_errors`, and answers `same` / `differs` / `unknown`. Once the left run
is off `start_path`, texts and GET requests are not compared (`off_screen`),
nor are texts on the step that returns to it (`off_screen_baseline`), since
`texts_appeared` is a delta against a page the port need not reproduce. Exit
0 means `unchanged` / `same`.
`run-samples` needs `$EXPERIMENT`; apps are found under `$SITES` (default
`$EXPERIMENT/behaviour-sites`), captures under `$CAPTURES` (default
`$EXPERIMENT/ui-ir/captures`).

Dependencies: `playwright-core` 1.58.2 (exact), installed with
`npm install --ignore-scripts`. No browser download: an installed Chromium
is found under the Playwright cache, or passed with `--chromium` /
`CAPTURE_CHROMIUM`. Tests: `node --test test/`.

## Taken from rea-agents

The design follows rea-agents 6.7.0 by morluto (MIT License, Copyright (c)
2026 morluto, https://github.com/morluto/rea). No REA code is copied; the
ideas were re-implemented. References are to the REA repository.

| REA idea | REA | here |
|---|---|---|
| declarative scenario, fixed action vocabulary, strict objects, unique step ids, refused whole before a browser starts | `docs/browser-scenario-contract.md` L3-28, L189-205; `src/domain/browserScenarioValues.ts` L137-223 | `lib/scenario.mjs` (`check`) |
| pinned environment: fresh context, viewport, scale, locale, timezone, colour scheme, reduced motion, service workers blocked | contract L3-28; `browserScenarioValues.ts` L99-129 | `lib/record.mjs` |
| readiness is explicit (`wait_for`), not inferred | contract L110-113 | `wait_for` action; `apply` merges it into the preceding action's observation |
| step 0 is the start navigation; url before / after, DOM and accessibility digests per step; later steps `cancelled` after a failure | contract L207-225 | `lib/record.mjs` |
| requests, request bodies and page errors attributed per step | contract L207-225, L277-349 | `Events` in `lib/record.mjs` |
| body kept as shape and sha256, values never written; secrets redacted | contract L177-187, L277-349 | `bodyShape`: content type, sorted field names, `body_sha256` |
| canonical digest, RFC 8785 | `src/domain/canonicalDigest.ts` | `lib/canonical.mjs`; `replay_digest` per step, `run_digest` |
| comparison aligned by step id: `unchanged` / `changed` / `unknown` | contract L371-418; `browserScenarioDiffValues.ts` | `lib/compare.mjs` (`compare`; `compareBehaviour` for `--mode behaviour`) |
| code vs runtime: match / mismatch / unknown; not seen is not absent | `docs/javascript-runtime-reconciliation.md` L87-134 | `apply`: `confirms`, `contradicts`, `unexplained`; nothing is written for what a run did not reach |
| canonical ordering for determinism | reconciliation L156-169 | sorted keys and lists everywhere |

Deliberate differences: other origins are blocked and the API is stubbed per
scenario (REA records the real network, contract L12-14); viewport
1280x800 to match ui-ir captures (REA 1280x720); a fixed clock; no typed
`not_requested` list; code is matched to elements by the ui-ir tuple, not
by initiator stacks.

Additions: a static server with SPA fallback on 127.0.0.1, preset
localStorage, and an element inventory per step (`lib/inventory.mjs`) whose
paths follow ui-ir's `tools/capture/src/collect.ts#L191-L199` so elements
carry ui-ir node ids. An element keeps the path it had when first seen, so a
portal opening later does not rename it. The inventory keeps only steps on
the screen's own route.

## apply

1. Match each trigger's tuple (and each submitter's) to inventory elements.
   `null` matches anything; `text_in` matches any listed text. An element
   that fits code elements from more than one place in the code is claimed
   by none of them: they become `ambiguous`. Otherwise `none` / `unique` /
   `multiple`.
2. Each action on a matched node (with its following `wait_for` steps) adds
   an observation:
   - `confirms`: the `does` paths the step showed (url, hash, overlay opened
     or closed, request seen, toast or message text appeared); nested
     effects as `0.on_success.1`.
   - `contradicts`: a certain `navigate` whose destination was not reached
     while no guard, validation message or other navigate explains it.
   - `unexplained`: requests the code does not explain (a GET of the landed
     page is not counted).
   - `bodies`, `page_errors`: as recorded.
3. Guards, routes and validation rules are observed the same way.
