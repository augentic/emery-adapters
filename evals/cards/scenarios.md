# The scenario oracle

A programme of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

## The express-orders scenario oracle

Until this round, one case had a behavioural measure of an `omnia` build: `train`'s hand-written reference guest and its 22 recorded fixtures ([evals/README.md](../README.md#black-box-fixture-oracle)). Every `express-orders` build card was graded on what the build reported of itself (`covered`, the verify verdicts) and on the host's checks over the tree (clippy, the tests the build wrote, the guest compiling), none of which asks whether the service built answers what the source service answers. `express-orders` is the case the slicing programme's build arms run over ([slicing.md](slicing.md)), so its builds needed the measure most.

A reference guest would have meant porting the Express service by hand. The source is an HTTP API over a store, so its behaviour is recorded instead: 57 scenarios under [`cases/express-orders/oracle/`](../cases/express-orders/oracle/), each a sequence of requests with the status, headers, and body fields the Express service answers, driven through the built guest's `router(provider)` over a fresh in-memory store apiece ([evals/README.md](../README.md#black-box-scenario-oracle-oracletoml)). The scenarios are the source's, read from the fixture:

- **`orders` (40).** Placement under each pricing path (standard, trade, vip over bulk, bulk, free shipping, repeated SKUs merged, half-up rounding), every total recomputed from `src/services/pricing.ts` by a script rather than by hand; the validation refusals (unknown and discontinued SKU, quantity, postcode, SKU pattern, 51 lines, a 201-character note); the upstream failures (an unzoned postcode, a 503, an empty quote list) as 502; the read, with the cache-aside read still answering `pending` after the row is set `paid` behind it; the amend on a pending and a paid order; every transition of the state machine and every refused one, with the transition log; the list by state, by customer, and by page with `x-total-count`, and the refused page sizes, pages, states, and customer ids.
- **`customers` (10).** Creation with the email lower-cased and the name trimmed, a `Location` header, a tier given, the duplicate differing in case as a conflict, the refused email and name; the read; the tier set and read back; the unknown id.
- **`start` (4).** The bearer gate: an `/api` request with no bearer and with the wrong bearer as 401, `/health` ungated, an unknown path as 404.
- **`health` (3).** Live, ready, and ready with the store unavailable as 503 `degraded`.

The assertions are strict: the source's camelCase field names, the `Location` and `x-total-count` headers, and of an error body the `error` code and the `state`, never the message. The built guest is held to its own specification, which carries each of these (`start.error-invalid-request`, `start.error-not-found`, `start.error-unauthorized`, `start.error-upstream-unavailable`, the `Location` header on both creations, `x-total-count` on the list), so a finding is a divergence from what the build was asked for and not from the Express service's accidents. One judgement call is recorded here because it costs nine scenarios: the source's `POST /orders/:id/pay` and `/ship` read no body, so the scenarios send none, as a client of the source would.

The store is the one piece of the harness that is not a double. The guest writes Postgres (`$N` parameters, `count(*)::text`, `lower(email) = lower($1)`, JSONB columns), and omnia ships no Postgres host, so the harness runs each scenario over an in-memory SQLite database under the Postgres contract: `$N` bound by number (`UPDATE customers SET tier = $2 WHERE id = $1` binds as written), `expr::type` as a `CAST`, `ILIKE` as `LIKE`, `now()` registered, and every column read back as the type `schema.sql` declares it, so an `INTEGER` column answers `Int64` as a Postgres host would and a guest reading it as `f64` fails here as it would there. Every statement that fails is kept with its SQL error and quoted beside the scenario's finding, so a gap in the shim is told from a fault in the guest.

### The bring-up

The oracle was first run over the build of `2026-11-07T06-05-54Z` (`CURSOR_MODEL` `auto`, 9 slices in 6 waves, `emery/ad28590a…` at `a6e9d0e6`), the card whose self-reported measures were: host checks ok 102/102, scenarios named 98/98, covered 61/76, with the 15 uncovered being `start`'s REQ-001–REQ-015, whose two slices were answered "nothing to merge" (0 files). The prediction written before the run was 32–38 of 57 with the shim, under 25 with the shim's rewrites off.

**Result: 10/57** (`run-1.oracle.md`: 5/10 customers, 3/3 health, 0/40 orders, 2/4 start). The prediction missed because one fault was not foreseen as blocking: every `orders` scenario placing an order passes through the catalogue read, and the guest reads `weight_grams` as `f64` from the `INTEGER` column (`Catalogue { weight_grams: f64 }`, `row.get("weight_grams")?`, answered `500 column weight_grams is not a float: Int64(Some(300))`). The guest's own tests pass because they seed the column as a `Double` in `ScriptedTables`.

To read past the one fault, the kept export was edited locally twice, under `--oracle`'s edit-and-rerun loop, and the edits were not shipped:

1. The weight read as `i64` cast to `f64`: still **10/57**. Every placement then fetched `GET /rates?zone=auckland&weight=…`, which no scenario seeds, since the guest holds a four-row zone table of its own (`auckland`, `waikato`, `wellington`, `south-island` over postcode ranges) where the source's `src/data/postcodes.ts` holds sixty-three rows (`1010` is `auckland-central-otahuhu`). The design carries none of the source's table, so the guest invented one.
2. The one zone renamed to the source's: **24/57** (14/40 orders). Every pricing scenario passed with the totals recomputed from `pricing.ts`, as did the cache-aside read, the amend, the cancel with a reason, and the three lists with `x-total-count`, so the shim, the pagination, the `count(*)::text` cast, and the JSONB round-trips hold.

The 33 failures that remain at the second rung classify as:

| class | scenarios | what the guest answers |
| --- | --- | --- |
| The error code's spelling | 20 | `bad_request`, `not_found`, `bad_gateway`: omnia's default codes (`bad_request!`) where the specification says `invalid-request`, `not-found`, `upstream-unavailable` |
| A bodiless `POST` refused | 9 | 415 `Expected request with Content-Type: application/json` on `/pay` and `/ship`, since the scaffold lays `Json<Input>` on every `POST` |
| The framework's rejections | 2 | 422 from axum's JSON rejection on a missing `reason`, and a plain-text 400 from its query rejection on an unknown `state`, where the specification says 400 `invalid-request` |
| The bearer gate absent | 2 | 404 `not_found` for the customer, so an unauthenticated `/api` request is served |
| An empty quote list | 1 | 400 `no quote for zone` where the source answers 502 |
| The zone table | 1 | postcode `1000` zoned where the source has it unzoned |

### What the baseline says

The build's self-reported 102/102 and 98/98 and the oracle's 10/57 measure different things, and the gap between them is the finding of the round. The checks and the tests the build wrote hold the tree to the layout and to the guest's own reading of the contract; the oracle holds it to the source's behaviour. The failures point at four levers, none of them the model's judgement in a slice:

- **The `start` requirements are built nowhere.** The bearer gate and the error mapping are `start`'s and carry no route, so `Plan::is_empty` answers both slices `Built` with no turn, and no other slice is told to answer `invalid-request` or to refuse a missing bearer. A cross-cutting rule is the stem's exported `pub fn` called at the top of the handlers it guards, by the layout's own rule, and nothing lays or asks for it.
- **The design loses the data table.** The `orders` seam read the table's module and answered one `excerpt` at `src/data/postcodes.ts#L16-L22`, which describes `zoneFor` and says that "ZONES at L24–L346 lists every delivery range with zone, courier, minDays, and rural" without carrying a row, and the design's `## Technical logic` repeats the description. A data table in a code source is summarised by an `excerpt` and carried by nothing, so the guest invents one.
- **The scaffold's `Json<Input>` on every `POST`.** A route whose scenarios state no body is laid with a body extractor, and the handler then refuses the source's clients.
- **The number heuristic.** `COUNTED` decides `i64` over `f64` by the field's last word, and `grams` is not counted.

### The next arm

The baseline is the control for whatever arm follows, and this card is the place its predictions are written before it is built, under the protocol: three runs under a pinned `CURSOR_MODEL`, the oracle read from `run-N.fidelity.md`'s `## Reference comparison` on a `--build --target omnia express-orders` card and from `run-1.oracle.md` on a `--rebuild`. A rule on the oracle reads the group counts (`orders`, `customers`, `start`, `health`) and the class a failure falls in, since one fault on the placement path moves forty scenarios at once and the total alone cannot tell a lever from a mask.
