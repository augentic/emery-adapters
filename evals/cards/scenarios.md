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

The arm is the baseline's revision rebuilt three times, each run a card of its own:

```bash
CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T06-05-54Z --target omnia express-orders
```

The control was built under `CURSOR_MODEL` `auto` and the arm runs under `composer-2.5`, so the model moves with the component; the rebuild holds the plan and the revision still, and the classes below are read against the baseline's table rather than its total. The component carries three of the four levers, `emery-omnia` alone changed:

- **The `start` rules reach the slices they guard.** No stub is laid: `start` has no crate, its two slices build in the first waves beside `customers`, and which stem owns the `/api` gate is nothing code can decide, so a `pub fn` laid by `start`'s slice would be called by no slice already built. `Plan::is_empty` and the `start` note stand. Instead the scaffold reads, for a slice laying handlers, the design's paragraphs on the rules every route obeys (`authenticate`, `unauthorized`, an error mapping over two or more statuses) and the part's plan brief where it names `start`, and the brief states the layout's rule for them: the rule is this part's to write, a gate as a `pub fn` of the first stem that needs it, called at the top of each handler it guards and depended on by a later stem through `<stem> = { path = "../<stem>" }`; an error's wire code as the design's mapping spells it, `Error::bad_request("<code>", ..)` over the macro's default; a `401` the host's `Error` lacks as the part's own error type; a refusal the specification codes checked by the handler over optional fields, since axum's own rejection of a query or a body answers a plain `400` or a `422` under no code. The selected design paragraphs are quoted under the statement, and the `pub fn`s other stems already export are listed, so the second stem calls the first's gate rather than writing another.
- **The body extractor only where the route states a body.** `Json<..Input>` is laid on a `POST`, `PUT`, or `PATCH` whose requirements or scenarios spell `body` or `payload`; `/pay` and `/ship` spell neither and take no extractor, and their scaffolded test acts send no body.
- **The number from what the design spells.** A `number` field is laid as `i64` or `f64` from what the design says of the same field elsewhere, a `Type:` fence declaring it `int` or `float`, a SQL column declaration (`INTEGER`, `NUMERIC`), the word `integer` beside its name, before the last-word heuristic decides; and the heuristic counts `grams`, `days`, `ms`, `bytes`, and `units` with the words it had. The `Catalogue` design type spells `weight_grams: number` and nothing else of it, so here the heuristic alone carries the lever.

**A mask stands between the levers and the oracle.** The baseline's second local rung showed what the first lever's repair uncovers: every successful placement fetches `GET /rates?zone=<zone>&weight=<grams>` under a zone the guest's own table names, since the design carries none of the source's sixty-three rows (the second lever of the baseline's four, a specify-side gap this round does not touch), and the harness's upstream double answers the seeded URL alone, so an unseeded fetch is a scenario failed as `fetched GET http://api.test/rates?zone=…, which the scenario does not seed`. The 23 `orders` scenarios that place an order to go on (`place-standard`, `place-trade`, `place-bulk`, `place-vip-beats-bulk`, `place-free-shipping`, `place-merged-skus`, `place-rounding-half-up`, `get`, `get-cache-aside`, `list-all`, `list-by-state`, `list-by-customer`, `list-page-size`, `amend-pending`, `amend-paid`, `pay-pending`, `pay-paid`, `pay-cancelled`, `ship-paid`, `ship-pending`, `cancel-pending-with-reason`, `cancel-shipped`, `cancel-without-reason`) are out of the oracle's reach whatever the build does, and the three that reach the rates service or the zone table (`place-rates-503`, `place-rates-empty`, `place-unzoned-postcode`) hang on the invented table agreeing with the source. The ceiling is 31 of 57 (`start` 4, `health` 3, `customers` 10, `orders` 14), and the bodiless `POST` lever is read in the exported tree (`run-1.oracle/crates/orders/src/orders.rs`) rather than by the oracle, since every `/pay` and `/ship` scenario is behind the mask.

#### Predictions

- **P1, by group, on every run.** `start` 4/4 (the bearer gate refuses both unauthenticated `/api` requests with `401 unauthorized`, `/health` stays ungated, the unknown path stays `404`); `health` 3/3; `customers` at least 8/10, and 10/10 on at least two of the three runs; `orders` 11–14 of 40, the 14 being the six refusals that place nothing (`amend-unknown`, `get-unknown`, `list-bad-customer`, `list-bad-state`, `list-page-0`, `list-page-size-101`) and the eight placement refusals decided before the rates fetch (`place-unknown-customer`, `place-unknown-sku`, `place-discontinued-sku`, `place-invalid-postcode`, `place-invalid-quantity`, `place-invalid-sku-pattern`, `place-note-too-long`, `place-too-many-lines`). Total 26–31 against the control's 10.
- **P2, by class.** The `weight_grams` read: `i64` in `Catalogue` on three of three trees and no `is not a float` answer on any run. The 23 masked scenarios fail as the unseeded `/rates` fetch on every run, none as a `500`. The error code's spelling: at most 2 scenarios per run outside the mask answer omnia's default code (`bad_request`, `not_found`, `bad_gateway`) where the specification names `invalid-request`, `not-found`, or `upstream-unavailable`, against 20 on the control. The bearer gate: present on three of three, as one `pub fn` under `crates/customers/` or `crates/orders/` called from the other stem's handlers, or one under each. The bodiless `POST`: `pay` and `ship` take no `Json` extractor on three of three trees, while `place`, `amend`, `cancel`, the customer create, and the tier update keep theirs. The framework's rejection on `list-bad-state`: a `400 invalid-request` on at least two of three runs.
- **P3, the build itself.** No new rule fires on this case: `refused_skips` reads consumer parts and this plan has none. The rejections per run (`candidate rejected` in `run-1.build.stderr`) stay within the baseline's rules, and no slice takes more than two correction rounds. The two `start` slices are answered with no turn, as before.

#### Decision rule

- **The arm holds** when P1 and P2 hold on all three runs; the three levers stand as built and the mask is the next specify-side lever.
- **`start` under 4 on any run:** the rules paragraph was not acted on; `crates/*/src/*.rs` of that run is read for a gate function, and whether the brief quoted the design's `authenticate` paragraph is read from `omnia_start_rules_in_brief`'s fixture against that run's `run-1.design.json`, since the brief is not written to the run's files. A gate present but uncalled from one stem is the exports listing's failure; absent on two of three runs, the brief is the lever and a check over `/api` handlers is proposed.
- **The error-code class above 2 on two of three runs:** a check is proposed, over a handler's `bad_request!`, `not_found!`, or `bad_gateway!` where the design's error mapping spells a code; one run alone is variance.
- **A `500` with `is not a float` on any run:** the heuristic's word list is read against the field, and the spelled-type reader against the design.
- **A `Json` extractor on `pay` or `ship` in any tree:** the body-word read is widened or the model wrote the extractor itself, told apart by the scaffolded test act (`Body::empty()` or `Body::from("{}")`).
- **Any failure outside the mask and the classes above recurring on two of three runs** is counted and a check proposed; one occurrence is recorded, not built. Totals alone decide nothing.

#### Results

Three runs, each the `06-05-54Z` revision rebuilt under `composer-2.5` on the component carrying the three levers and `refused_skips`: the cards `2026-11-08T02-11-02Z`, `02-40-14Z`, and `03-06-21Z`. The oracle is read from each card's `run-1.fidelity.md` (`run-1.oracle.md` and `run-1.oracle/` beside it are the baseline's copies, which a rebuild carries over), and the tree from the label branch of its `.emery/vcs/repos/run-1`. The brief is not written to a run's files, so whether it quoted the design's rules was read by projecting the run's revision with `emery show design` over a copy of its staged project and applying `Rules::read`'s block and word rules to the text: eight paragraphs qualify, among them `Bearer authentication gates /api: missing or unknown tokens yield 401 (REQ-006, REQ-007) …`, `authenticate reads the authorization header, requires BEARER_PREFIX "Bearer " … Mounted only under /api.`, and the `errorHandler maps ZodError to 400 { error: "invalid-request" } … UnauthorizedError to 401 … NotFoundError to 404 { error: "not-found" } …` mapping, so every `customers` and `orders` slice was handed the gate and the mapping.

| card | oracle | customers · health · orders · start | the `/api` gate | `pay` / `ship` | `weight_grams` | default codes outside the mask | the masked scenarios | rejections | build |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `02-11-02Z` | 29/57 | 10 · 3 · 14 · 2 | none: no gate function, the one `Bearer` the rates client's outbound header, `/api/customers/{id}` answered `404 not-found` unauthenticated | no `Json`, the tests' acts `Body::empty()` | `i64` | 0 | 26 fail as `fetched GET http://api.test/rates?zone=akl-metro&weight=…, which the scenario does not seed` | 0 | 1694s |
| `02-40-14Z` | 28/57 | 10 · 3 · 13 · 2 | none, the same | the same | `i64` | 0 | 27 fail as `500 column active is not an integer: Boolean(Some(true))`: the laid `Catalogue { active: bool }` stands, and the turn's own `#[derive(FromRow)] struct CatalogueRow` beside it reads the `BOOLEAN` column as `active: i64`, so the catalogue read fails before the rates fetch and `place-discontinued-sku` with it | 2: SLICE-004 `broad_bounds`, SLICE-006 `unused_dependencies` | 1512s |
| `03-06-21Z` | 29/57 | 10 · 3 · 14 · 2 | none, the same | the same | `i64` | 0 | 26 fail as the unseeded fetch under `zone=metro` | 1: SLICE-004 `broad_bounds` | 1459s |

- **P1 fails on `start` alone.** `start` 2/4 on three of three: `health-without-token` and `unknown-path` pass, `api-without-token` and `api-wrong-token` reach the customer lookup and answer `404` where `401 unauthorized` is expected. `health` 3/3 and `customers` 10/10 on three of three; `orders` 14, 13, 14 of 40, the 14 the predicted fourteen exactly (the six refusals that place nothing and the eight placement refusals decided before the rates fetch), the 13 those less `place-discontinued-sku`, which `02-40-14Z`'s catalogue read fails before the discontinued check. Totals 29, 28, 29 against the control's 10: inside the 26–31 band, whose ceiling of 31 assumed the gate and stands at 29 without it, so two runs sit at the ceiling the three levers alone allow.
- **P2 holds on three of its five classes.** The `weight_grams` read is `i64` in `Catalogue` on three of three trees and no run answers `is not a float`. The error code's spelling: 0 scenarios outside the mask on every run against 20 on the control; every refusal is `Error::bad_request("invalid-request", ..)` or `Error::not_found("not-found", ..)`, `list-bad-state` is a `400 invalid-request` on three of three over an `Option<String>` the handler checks, and `pay` and `ship` answer the `409 conflict` through a module error type (`TransitionError`), as the brief's "a status of its own" rule says. The bodiless `POST`: `pay` and `ship` take no `Json` extractor on three of three trees, their scaffolded acts send `Body::empty()`, and `place`, `amend`, `cancel`, the customer create, and the tier update keep theirs. The bearer gate fails: absent on three of three. The masked class fails on one: `02-40-14Z` answers the 27 as a `500`, the `active` column read as an integer.
- **P3 holds.** `refused_skips` fired on no run. Rejections 0, 2, 1, each within the baseline's rules (`broad_bounds` twice at `health`'s SLICE-004, `unused_dependencies` once at SLICE-006), no slice past one correction round. The two `start` slices were answered with no turn on every run.

**Reading under the rule.** `start` is under 4 on every run, so the trees were read for a gate: none has one, and none calls one, so this is not the exports listing's failure but the brief's. The brief quoted the design's `authenticate` paragraph and the mapping paragraph together, and the turns acted on the mapping (the code class moved from 20 to 0) and not on the gate: the gate's requirements (`REQ-006`, `REQ-007`) belong to `start`'s slices, which no turn builds, and nothing in the slice's own requirements or scenarios states a `401`, so a turn reporting `covered` over its own ids has nothing to cover by writing it. Absent on three of three, a check is proposed: `ungated_routes`, at a build, where the design's rule paragraphs spell a gate (`Rules::read`'s gate words) and name the prefix it mounts under (`/api`): a filled handler whose route falls under that prefix and whose body neither reads the `authorization` header (`HeaderMap`, `TypedHeader<Authorization<Bearer>>`) nor calls a function that takes one is one finding at its line, the header naming the layout's rule, one `pub fn` of the first stem under the prefix, called at the top of each handler it guards and depended on by the later stem through `<stem> = { path = "../<stem>" }`. The alternative is the scaffold laying the gate's `pub fn` stub in the first stem whose routes fall under the prefix, by schedule order (`customers` here), with its call at the top of each scaffolded handler, which decides by code what the brief leaves to the turn; recorded beside the check, not built in this round. The error-code class stays at 0, so no check there. No `is not a float`. No `Json` on `pay` or `ship`.

- **The `active` column read as an integer: 1 of 3.** The design type is laid right (`active: bool`) and the turn's own row struct for the store read diverges from it (`active: i64`), where `03-06-21Z`'s row struct reads the column as `bool` and `02-11-02Z` tries `bool` first and `i64` second. The slice's tests pass because they seed the column as the guest reads it. Recorded, not built: a second occurrence would propose a check over a `FromRow` struct whose field shares a laid design type's field name and differs in scalar kind.
- **The zone table: 3 of 3**, a different invented table per run (`akl-metro`, `metro`), as predicted, the specify-side lever outside this round.

Follow-ups, in this order: the `ungated_routes` check or the scaffolded gate; the data table and the `excerpt` at specify; the row-read struct against the laid type if it recurs.

### The gate by code

The first follow-up of the arm above, built in the second build-quality round ([decoders.md](decoders.md#the-registration-before-the-turn) carries that round's `train` arm). The `/api` bearer gate was absent on three of three trees though every `customers` and `orders` slice was handed the design's `authenticate` and mapping paragraphs: the turns acted on the mapping (the code class 20 to 0) and not on the gate, whose requirements (REQ-006, REQ-007) are `start`'s and so no slice's to cover. The arm above proposed either `ungated_routes` or the gate laid by the scaffold. This arm lays it and proposes the check only on a recurrence, for a reason the rounds so far give: every lever that moved the oracle was the scaffold laying by code (the typed route, the laid types, the three levers above, 10 to 29), while the checks of the last two rounds found nothing (`default_decoders` 0, `refused_skips` 0) or spent the one correction round a turn has (`unconsumed_streams`, 9 of 9). A check corrects after the turn at the price of its round; a laid stub decides before it at none.

What the scaffold lays, in `adapters/omnia/` alone:

- **The reading.** `reference::gate(slice)` reads the gate paragraphs `Rules::read` already selects: the prefix is the `/`-led path token they spell most often (`gates /api:`, `under /api.` give `/api`), the settings the keys they spell (`API_TOKENS`, `BEARER_PREFIX`) through `Settings::spelled`, and the wire code the word quoted beside `401` (`unauthorized`). A gate paragraph spelling no prefix lays nothing, as today.
- **The owner, from the tree.** The slice carries no schedule, so who lays the gate is decided from the working copy: where no existing stem crate exports a `pub fn` taking a `HeaderMap`, the slice's first stem with a route under the prefix lays it; where one does, it is imported and called, with `<stem> = { path = "../<stem>" }` in the manifest; a serial slice of the gating stem calls the one its held module declares. On this plan `customers` builds in wave 1 and lays it, `orders` in waves 2–6 calls it, and `health` is not under `/api`. Known limitation, recorded: two stems under the prefix in one wave each lay a gate.
- **The shape.** In the gating module: `GateSettings` over the keys read; `GateError`, the part's own error type (`Unauthorized`, `Host(#[from] omnia_sdk::Error)`, `From<anyhow::Error>`, and `IntoResponse` answering `401 { "error": "unauthorized" }`), since `omnia_sdk::Error` has no 401 and `Handlers` has no layer; and `pub async fn authenticate<P: Config>(context: &Context<P>, headers: &HeaderMap) -> Result<(), GateError>` over a `todo!` marker the brief lists in fill order. Every scaffolded handler under the prefix takes `headers: HeaderMap`, calls `authenticate(&context, &headers).await?` as its first line above its marker block, and answers `Result<Json<O>, GateError>`; its scaffolded test seeds the gate's keys and sends `AUTHORIZATION` from a `"<authorization>"` placeholder the existing placeholder rule holds to being filled.
- **The backstop.** The gate's marker is no `REQ-`, so `markers_left` gains one clause: the gate left `todo!` while a gated handler of the slice is filled is one finding at the gate's line. No `ungated_routes` this round.

The arm is the baseline's revision rebuilt three times, each a card of its own, on the component carrying the gate and the `train` arm's statement:

```bash
CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T06-05-54Z --target omnia express-orders
```

Its control is the arm above (29, 28, 29 of 57; `start` 2/4 on three of three). The mask stands, so the ceiling with the gate is 31 of 57.

#### Predictions

- **G1, by group, on every run.** `start` 4/4 (`api-without-token` and `api-wrong-token` answer `401 unauthorized`, `health-without-token` and `unknown-path` as before); `health` 3/3; `customers` 10/10; `orders` 14/40, the fourteen of the arm above. Total 31/57 on three of three, the mask's ceiling.
- **G2, the mask.** The 26 masked `orders` scenarios fail on the zone, the unseeded `/rates` fetch under an invented zone or a postcode zoned where the source has it unzoned, and none as a `500`.
- **G3, the gate in the tree.** One `pub async fn authenticate` with `GateError` under `crates/customers/src/customers.rs`, filled, no `todo!` left in it; called at the top of every `/api` handler of `customers` and `orders`; `customers = { path = "../customers" }` in `crates/orders/Cargo.toml`; no second gate; `health`'s handlers take no `HeaderMap`.
- **G4, the three levers above hold.** `weight_grams: i64`; `pay` and `ship` bodiless; 0 default codes outside the mask.
- **G5, the build.** No slice past one correction round; the `start` slices answered with no turn; the `markers_left` gate clause seen on no run.

#### Decision rule

- **The arm holds** when G1–G5 hold on all three runs; the gate stands as laid.
- **`start` under 4 on any run,** the tree read: the gate absent, `reference::gate` missed the paragraphs or the prefix, read against that run's `run-1.design.json`; present and the oracle's bearer refused, the gate body's reading of `API_TOKENS` (split on `,`) and `BEARER_PREFIX`, the brief's gate entry the lever; present and uncalled from a filled handler, `ungated_routes` proposed at 2 of 3 (a filled handler under the prefix whose body neither reads the `authorization` header nor calls a `HeaderMap`-taking fn) and recorded at 1 of 3.
- **The `FromRow` struct against the laid `bool`** (1 of 3 above) on a second sighting: the check proposed.
- **Anything else recurring on two of three runs** is counted and a check proposed; one occurrence is recorded. Totals alone decide nothing.

#### Results

Not yet run.

### The data table

The second lever of the baseline's four, pulled forward in the same round: it is the mask over 26 of the 40 `orders` scenarios, and the ceiling of every express-orders arm until it moves. The loss was traced to one line of the engine's SDK. The typescript survey's walk reads `ZONES` (`src/data/postcodes.ts#L24-L346`, 321 rows over 323 lines, 36 KB) as a module-level table, and `survey::types` elides a table over `TABLE_LINES` (120 lines) from its `type` claim; the `06-05-54Z` design holds `Type:` blocks for `TRANSITIONS` and `TERMINAL_STATES`, the small tables, each fenced with its rows, and none for `ZONES`, which reaches it as the `excerpt`'s sentence alone. The build fills the small tables from their fences (`Transitions::default()` carries the four rows) and invents the large one: `akl-metro` 1000–1999 and `wlg-metro` 6000–6999 on `02-11-02Z`, `metro` on `03-06-21Z`, where the oracle's seeded rates URL names `auckland-central-otahuhu` for postcode `1010`.

Two changes, one in each repository:

- **Engine, the cap.** `TABLE_LINES` becomes a byte budget, `TABLE_BYTES` at 64 KiB, the inline budget a turn already carries, applied to the module-level and the class-member table alike. A table within it reaches the design whole as its `Type:` fence, which the engine renders verbatim and no model copies. Pre-1.0, this repository's `emery-sdk` pin moves with the engine's commit.
- **Adapter, the table laid as data.** The omnia scaffold reads a `const NAME: readonly T[] = [ {..}, .. ]` fence whose rows are object literals of scalars into a table, and, where `T` is a type the slice owns, lays `crates/<stem>/data/<snake>.json` (the rows under their wire keys, one object per line) and `pub fn <snake>() -> Result<Vec<T>>` reading it through `include_str!` and `serde_json`, the file guarded line for line so the turn cuts no row, and the brief naming it finished. A map (`Readonly<Record<..>>`), a table of an unowned element, or a row that is no object of scalars stays today's marker. The alternative, a 321-row marker for the turn to transcribe, asks the model to copy 36 KB faithfully, which the arm above shows it is never asked to and the small tables show it need not be.

The arm is a fresh card over the rebuilt engine and both components, since the cap lives in the typescript component's SDK:

```bash
CURSOR_MODEL=composer-2.5 EVAL_RUNS=3 cargo run -p evals -- --build --target omnia express-orders
```

Its controls are `06-05-54Z`'s design (no `ZONES` fence) and the gate arm's trees above. A `--build` card seals three fresh revisions, so the plan and the specification vary with it, and the gate and the table ride together; the classes are read apart, never the total.

#### Predictions

- **D1, the design.** `Type: ZONES` fenced with its 321 rows in each run's design on three of three, `DeliveryZone` beside it, and no `excerpt` sentence summarising the table in its place.
- **D2, the tree.** `crates/orders/data/zones.json` with 321 rows and `pub fn zones()` in the module that owns `DeliveryZone`, the file byte-identical from the scaffold to the seal (the turn edits it on no run), on three of three; no table of the turn's own (`ZoneRange`, `ZONE_RANGES`) beside it.
- **D3, the zone class gone.** No `place-*` scenario fails on the zone: every placement fetches the seeded `GET /rates?zone=auckland-central-otahuhu&weight=…`, `place-unzoned-postcode` (postcode `1000`) answers `400 invalid-request`, and the three rates scenarios (`place-rates-503`, `place-rates-empty`, `place-unzoned-postcode`) pass on at least two of three.
- **D4, the rest read, not predicted.** `start` 4/4, `customers` 10/10, `health` 3/3 carry from the gate arm where the fresh specification holds them; the `orders` count and the classes behind the lifted mask (the pricing paths, the totals, the cache-aside read, the transitions) are counted here for the first time and are the next round's levers.

#### Decision rule

- **D1 fails:** the SDK side is the lever; the run's evidence is read for the `ZONES` claim, present there and absent from the design pointing at the design-draft turn or its gate, absent pointing at the survey's walk or the cap.
- **D2 fails with D1 holding:** the omnia reader missed the fence's shape; the fence is quoted in the card beside what the reader accepts.
- **D3 fails with D1 and D2 holding:** the placement path is read for where the rows are not consulted; one occurrence recorded, two of three a check proposed.
- **D4:** the classes counted; totals alone decide nothing.

#### Results

Not yet run.
