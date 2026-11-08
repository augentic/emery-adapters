# The inbound wire format

A programme of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

## The wire format from the `call` claims to the decoder

Three `train` builds through `omnia` on 2026-11-07 failed or passed the reference oracle on one fact: whether the built guest decoded the inbound train update as XML.

- `2026-11-07T07-05-51Z` (`CURSOR_MODEL` `auto`): the design's `## Overview` said the program "consumes XML train-update messages"; the model registered the consumer through `topic_with` over a `quick_xml::de::from_reader` decoder; the oracle passed 22/22 once the harness was repaired (`run-1.oracle.md`).
- `2026-11-07T08-06-00Z` and `2026-11-07T08-14-57Z` (`composer-2.5`): the design named XML under `## Overview`, `## Domain model`, and `## Technical logic`, but the inbound-stream paragraph of `## APIs and integrations` named no payload format; the model registered the consumer through `.topic(..)`, whose default decoder reads JSON; every fixture carrying a message was refused `malformed JSON payload`, and the oracle passed 7/22, the seven being fixtures that expect a refusal.

In all three runs every requirement carried `trigger: None`: the `typescript` survey derives `Trigger::Message` from a literal topic at the registration, and this source reads its topic from a setting. So the adapter scaffolded functions, never a consumer, and the model wired the stream itself along the untyped path of `build.md` and the `unreached_publishers` finding, both of which spell `.topic(..)` alone. The `topic_with` alternative stands only on the typed-consumer path, in the brief's consumer entry and `build.md`'s consumer section, which none of the three runs took.

The evidence carried the fact every time: a `call` claim `x2js:X2JS.xml2js` at `src/main.ts#L102` and an `excerpt` of the parse. Nothing held the design to it: `DesignBrief::verify` checks the section plan, blank and reserved paragraphs, bound citations, and type keys, and reads no `callee`.

The arm changes the engine (`emery`) and this repository together:

- **Engine, prompt.** `design-format.md`'s stream sub-bullet names the payload's wire format and the codec the `call` claims spell as reading or writing it, `[unknown]` where the claims are silent, since a target reads the decoder from the inbound bullet.
- **Engine, check.** `DesignBrief::verify` and `DesignAmend::verify` hold the design's text to the packages the `call` claims spell: a package named nowhere in the design is a finding the model corrects. The check reads every section's paragraphs, so a package named in `## Technical logic` passes; it closes the silent loss, not the placement.
- **Adapter, prose.** `build.md`'s untyped-stream paragraph and the `unreached_publishers` header offer `topic_with` over a `decode` of the part and `quick-xml.workspace = true` where the design says the payload is not JSON.
- **Adapter, reader.** `reference::wire(slice)` reads the wire format of the inbound stream from the paragraphs of `## APIs and integrations` and `## Technical logic` that cite one of the slice's ids or none, and from the slice's own words: a sentence spelling an inbound word and a non-JSON format word decides, or, in a paragraph led by an inbound word (the inbound stream's own bullet), a sentence spelling the format alone with no outbound word. The format is held on the plan beside the roles and the idioms.
- **Adapter, check.** `default_decoders`, at a build, reports each `.topic(..)` registration in the slice's parts whose handler is filled while the wire is known to be no JSON, naming `topic_with`, the `decode` signature, and the manifest line.

The reader's paragraph rule was added during the round, before any treatment run completed: the first treatment run's design (set aside under `target/eval/arms/decoders-treatment-aborted-19-16-16Z/`, specify only) rendered the prompt's sub-bullet as ``Wire payload is XML; decoding uses `x2js` (`X2JS.xml2js`) before `ta-json` deserialisation under the configured root element``, two sentences the sentence rule as first written reads neither of, so the run was stopped at its first build and the arm restarted over the component carrying both rules. P1's second clause is read against the reader as restarted.

The round runs under `CURSOR_MODEL` pinned to `composer-2.5` over the `train` case. The control is the engine and component of `2026-11-07T08-14-57Z`, snapshotted under `target/eval/arms/decoders-control/` (`emery`, `emery_omnia.wasm`, `emery_typescript.wasm`) before any rebuild; the two existing `composer-2.5` cards are the control arm's first two runs, the first of them over the component of the morning's build and the second over the component rebuilt with the seed's comment-anchored router array, neither of which touches the decoder.

1. **Control, one run** (completing the arm at three):
   ```bash
   EMERY_BIN=target/eval/arms/decoders-control/emery EMERY_OMNIA_WASM=target/eval/arms/decoders-control/emery_omnia.wasm EMERY_TYPESCRIPT_WASM=target/eval/arms/decoders-control/emery_typescript.wasm \
     CURSOR_MODEL=composer-2.5 EVAL_RUNS=1 cargo run -p evals -- --build --target omnia train
   ```
2. **Treatment, three runs**, over the rebuilt engine and component:
   ```bash
   CURSOR_MODEL=composer-2.5 EVAL_RUNS=3 cargo run -p evals -- --build --target omnia train
   ```
3. **Isolation, three runs**, the treatment component over the revision `2026-11-07T08-14-57Z` sealed, so the adapter's lever is measured over a design whose inbound bullet names no format:
   ```bash
   CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T08-14-57Z --target omnia train
   ```
   run three times, each a card of its own.

The predictions and the decision rule were written before the arms were built.

### Predictions

- **P1, the inbound bullet names the format.** On every treatment run, the inbound-stream paragraph of `## APIs and integrations` in `run-N.design.json` names XML, and `reference::wire` reads it as the slice's wire.
- **P2, the guest decodes XML.** On every treatment run and every isolation run, the consumer is registered through `topic_with` over a decoder reading `quick_xml`, `run-N.fidelity.md` records no `malformed JSON payload` refusal, and `default_decoders` is answered in at most one correction round per slice (`candidate rejected` in `run-N.build.stderr`).
- **P3, the oracle returns to its band.** On at least two of three treatment runs, and at least two of three isolation runs, the oracle passes within 21–22 of 22, the band Round E set and `07-05-51Z` held.
- **P4, the control reproduces.** The third control run passes 7/22 with the `malformed JSON payload` refusal on every fixture carrying a message.
- **P5, specify holds.** On every treatment run the stem set is `{"train-update"}`, leaks are 0, requirement recall is within 11–12 of 14, and the design's `call_unnamed` check costs at most one correction round of the `design-draft` turn.

### Decision rule

- **The arm holds** when P1, P2, P4, and P5 hold on every run and P3 is within the band on both arms; the prompt, the engine check, the reader, and `default_decoders` then stand.
- **P1 fails:** the prompt is the lever; read the inbound paragraph the model wrote and tighten `design-format.md`'s sub-bullet before the engine check is touched.
- **P2 fails with P1 holding:** the reader or the check; read the sentences `wire` was offered in `run-N.design.json` and the `candidate rejected` findings, and fix the one that did not fire.
- **P3 fails with P1 and P2 holding:** the oracle's per-fixture failures are logic defects behind the decoder, recorded against the build turn, not against the arm.
- **P2 holds on the isolation arm while the control fails it:** the adapter's reader and check carry the result alone, and the engine's prompt change is kept for the design's own sake.
- **P5 fails on the correction round alone:** the lenient check is costing a turn on packages the design has no role for; read the findings and narrow `packages` before loosening the check.

### Results

Seven cards, all `composer-2.5` over `train`: the control arm `2026-11-07T08-06-00Z`, `08-14-57Z`, and `18-14-21Z` (the third over `target/eval/arms/decoders-control/`); the treatment card `19-26-01Z` (three runs); the isolation cards `19-53-19Z`, `20-03-34Z`, and `20-13-48Z`, each the revision of `08-14-57Z` rebuilt through the treatment component. The brief a build turn is given is not written to the run's files, so "wire read" below is the reader's rule applied by hand to the paragraphs of `run-N.design.json` that cite one of the slice's ids or none.

| arm | card · run | inbound bullet | wire read | registration | build rejections | oracle |
| --- | --- | --- | --- | --- | --- | --- |
| control | `08-06-00Z` | no format | — (old component) | `.topic(topic::REALTIME_TRAIN, ..)` | 1 | 7/22, `malformed JSON payload` on every message fixture |
| control | `08-14-57Z` | no format | — | `.topic(topic::DEV_REALTIME_TRAIN_V1, ..)` | 1 | 7/22, the same |
| control | `18-14-21Z` | no format | — | none: `Handlers::from([])`, every requirement a `pub fn` | 0 | not run, `no inbound topic consumed` |
| treatment | `19-26-01Z` · 1 | ``Wire payload XML; codec `x2js` (`X2JS.xml2js`)`` | XML, both slices, from the bullet | none: `Handlers::from([])`, no `Publish` spelled | 0 | not run, `no inbound topic consumed` |
| treatment | `19-26-01Z` · 2 | ``Wire format XML; codec `x2js` for `xml2js` `` | XML, from the bullet | `topic_with(topic::REALTIME_TRAIN, on_realtime_train, decode_cco)` over `quick_xml::de::from_reader`, SLICE-001 | 1 (`unnamed_topics`, `unreached_publishers`) | 11/22 |
| treatment | `19-26-01Z` · 3 | `Payload wire format XML; codec x2js for xml2js` | XML, both slices, from the bullet | `topic_with(topic::REALTIME_TRAIN, on_train_update, decode_train_update)` over `quick_xml`, SLICE-002 | 1 (`unreached_publishers`) | 11/22 |
| isolation | `19-53-19Z` | no format (the `08-14-57Z` design) | XML for SLICE-001 from `## Technical logic` (`` Inbound XML → object via `x2js` ``, citing REQ-001–004 and REQ-006); none for SLICE-002 | `topic_with(topic::REALTIME_TRAIN, on_realtime_train, decode_inbound_train)` over `quick_xml`, SLICE-002 | 1 (`unnamed_topics`, `unreached_publishers`) | 13/22 |
| isolation | `20-03-34Z` | the same | the same | `topic_with(topic::REALTIME_TRAIN_UPDATE, on_train_update, decode_train_update)` over `quick_xml`, SLICE-002 | 1 (`unreached_publishers`) | 15/22 |
| isolation | `20-13-48Z` | the same | the same | `topic_with(topic::REALTIME_TRAIN_UPDATE, on_train_update, decode_train_update)` over `quick_xml::de`, SLICE-002 | 1 (`unreached_publishers`) | 13/22 |

- **P1 holds.** All three treatment designs name XML and `x2js` in the inbound bullet, in the sub-bullet's own words (`wire`, `payload`, `codec`), and the reader as restarted reads XML for every slice of every treatment run, through the paragraph rule in each case: the format stands in a sentence of its own after the semicolon, with no inbound word beside it. The sentence rule as first written would have read none of the three, which is what stopped the first treatment run (above). The design-draft turn was corrected once on every treatment run, and the corrected design carries the bullet's format.
- **P2 holds where a consumer exists, and the round found a failure before it.** Every consumer registered in the round, two of three treatment runs and three of three isolation runs, is `topic_with` over a `quick_xml::de::from_reader` decoder under `quick-xml.workspace = true`, and no file of any run records `malformed JSON payload`. `default_decoders` fired on no run: on all five the model's first candidate was rejected by `unnamed_topics` or `unreached_publishers`, and the corrected candidate registered `topic_with` at once, so the `unreached_publishers` header, which now spells `topic_with` for a non-JSON payload, carried the change and the rule stood behind it unused. On the three isolation runs the consumer was registered by SLICE-002 (REQ-013 through REQ-015), the slice whose wire the reader reads as none, since the only paragraph naming XML cites SLICE-001's ids; so `default_decoders` could not have fired there, and the decoder the model chose came from the header's words alone. Treatment run 1 and control run 3 registered no consumer at all: every requirement landed as a `pub fn` and the stem's entry stayed `Handlers::from([])`, no part spelled `Publish`, and the oracle found no topic to drive. Neither `unreached_publishers` (which keys on `Publish`) nor `default_decoders` (which keys on `.topic(`) reads that shape.
- **P3 fails on both arms, with no decoder failure among the causes.** Treatment 11/22 and 11/22 (and one run not driven), isolation 13/22, 15/22, 13/22, against the 21–22 band. The failures are logic defects behind the decoder, recorded against the build turn as the rule reads them: `19-26-01Z` run 2 publishes no `key` header on nine fixtures (the `publish-message-key` requirement), publishes two events where a refusal is recorded (`static/0010`), and rejects one replay on its event time (`replay/4145`); run 3 puts the vehicle label in `remoteData.externalId` where the fixture records the identifier (seven fixtures), publishes other coordinates than the fixture records (`-36.84448` for `-36.12345`), publishes where nothing is recorded (`static/0004`), and publishes one distinct event where two are recorded (`replay/0304`, `replay/4156`); every isolation run rejects the seven replay fixtures as `no_actual_update` where the reference publishes, and two of them publish other coordinates than recorded (`19-53-19Z` truncates them to integers, `-36` for `-36.12345`; `20-13-48Z` `-37.20299` for `-37.20331`).
- **P4 fails in letter.** The third control run did not reproduce 7/22 with the JSON refusal: it registered no consumer, so the oracle ran nothing. The control's two modes are the refusal (two runs) and the missing consumer (one run); the treatment arm met the second of them once in three.
- **P5 fails on recall once.** Stems `{"train-update"}` and leaks 0 on every run; recall 12/14, 10/14, 13/14, the second outside the 11–12 band on 45 accepted claims against 53 and 50, which is the extraction's variance and not the design's. The engine's package check cost exactly one correction round on each treatment run, on `auth-common` and `realtime-common` (runs 1 and 3) and on `moment-timezone` (run 2): internal and utility packages the design has no role sentence for, named after the correction in a clause such as `realtime-common:KafkaCommon.KafkaConsumer`. That is the rule's P5 clause met, and its warning read: the check is buying the inbound bullet's codec at the price of one turn on packages the design would otherwise pass over.

**Reading under the rule.** P1 holds, so the prompt is not the lever. P2 holds on the isolation arm while the control fails it, so the adapter's prose and check carry the decoder result alone, and the engine's prompt change is kept for the design's own sake. P3 fails with P1 and P2 holding, so the oracle's failures are logic defects, listed above and not charged to the arm. Two findings fall outside the rule as written:

- **An unconsumed stream.** In two of the round's seven `composer-2.5` builds (the third control run and the first treatment run), the model answered every requirement with a `pub fn`, registered nothing, and spelled no `Publish`, so no rule fired and the guest is a library. The design's inbound bullet names the stream and the requirements its deliveries set off (`Deliveries drive train-update processing (REQ-001 through REQ-013)`), so a build-time rule can read it: where the design lists an inbound stream for the stem and no part of the stem registers a `.topic(..)` or `.topic_with(..)`, after every slice of the stem, the finding is the registration the `unreached_publishers` header already spells. The rule has no `Publish` to key on, so it keys on the bullet.
- **The wire is read per slice, and the consumer lands in another.** All three isolation runs registered the consumer in SLICE-002 while the reader read the wire for SLICE-001, since the one paragraph naming XML cites SLICE-001's ids. A wire the design names for the stem is the stem's, not a slice's: `wire` should read the paragraphs citing any id of the stem, or none, as `roles` does not.

Follow-ups, in this order: the `unconsumed_streams` rule; the wire read for the stem; the oracle's logic defects as build-turn findings (`publish-message-key`'s header, the external identifier, the departure-versus-arrival coordinates, and the actual-time check), each a worked example or a translation row before any check. The next section takes the first two and re-reads the third.

## The stream consumed and the wire read for the part

The round above left two shapes no rule reads and five driven runs between 11 and 15 of 22. Read again for this round:

- **The unconsumed stream.** In `18-14-21Z` and `19-26-01Z` run 1 the model answered every requirement with a `pub fn`, left the stem's entry at `Handlers::from([])`, spelled no `Publish`, and the oracle found no topic to drive. `unreached_publishers` keys on `Publish` and `default_decoders` on `.topic(`, so neither fired. The design's inbound bullet names the stream and the requirements its deliveries set off (`Deliveries drive deserialization and the train-update behaviour in REQ-001–REQ-016`), which is enough for a rule: at the part's last slice, a stem the bullet's requirements lead with whose crate registers no `.topic(..)` or `.topic_with(..)` is a finding at its entry, carrying the registration recipe `unreached_publishers` already spells.
- **The wire read per slice.** All three isolation runs registered the consumer in SLICE-002 while `wire` read XML for SLICE-001 alone, since the one paragraph naming XML cites SLICE-001's ids. The slice carries the part's entry in the plan, whose `Requirements:` line lists every id of the part, so the reader's citation clause is widened from the slice's ids to the part's: a wire the design names for the part is every slice's.
- **The `no_actual_update` rejections** on all seven replay fixtures of all three isolation runs are the `08-14-57Z` revision's own REQ-002: its scenario says `has departed but no positive actual departure time, or has arrived but no positive actual arrival time`, an `or` of two checks where the source is an `if hasDeparted .. else if hasArrived ..` and one `<= 0` test after it; the built guests read the `or` as written and rejected a departed train whose actual arrival time is `-1`, which every replay fixture carries. Four fresh `composer-2.5` specifications since (`18-14-21Z`, `19-26-01Z` runs 1–3) and `08-06-00Z` phrase it as `neither departed nor arrived with a positive actual time`, which accepts the fixtures; the typescript `extract.md` has carried the `if / else if` precedence rule since October. One specification in six flattened it: variance, no prompt change, and `08-14-57Z` retired as a rebuild base for any logic measure.
- **The rest are build misreads of correct text:** the departure overwrite applied to an arrival on two of five driven runs (`19-26-01Z` run 3 and `20-13-48Z`, `static/0001` and `replay/4156`); the registration number in `remoteData.externalId` on one (`19-26-01Z` run 3); integer coordinates on one (`19-53-19Z`); the `key` header missing on one (`19-26-01Z` run 2, whose specification missed the `publish-message-key` requirement). Counted again this round, no check.

The round changes `emery-omnia` alone; the engine stays as the round above left it, its package check costing the one correction round P5 allowed.

- **Reader.** `reference::wire` filters the design's paragraphs by the part's ids, read from the slice's own ids and the `Requirements:` line of its plan entry. `reference::inbound` reads the inbound stream's paragraph from `## APIs and integrations`: the first whose lead sentence spells an inbound word and no outbound word and which cites an id of the part, answered as that lead sentence, so a status bullet that mentions the consumer and the producer together is passed over. Both are held on the plan; the slice's position (`index == count`) beside them. (As built, the lead must spell a stream word too — `stream`, `topic`, `consumer`, `broker`, `message` — since the suite's `Inbound HTTP surfaces, a REST API over JSON` paragraph read as the stream under the inbound word alone.)
- **Check.** `unconsumed_streams`, at a build, where the design lists an inbound stream for the part and the slice is the part's last: for each stem of the slice the bullet's requirements lead with, every stem of the slice where the bullet cites none of the slice's own ids, no `.topic(..)` or `.topic_with(..)` under `crates/<stem>/src/` is one finding at the entry's `Handlers::from(` line, quoting the bullet's lead.
- **One recipe.** The registration (the topic's constant, the `handlers()` entry over `.topic(..)` or, where the wire is known, `.topic_with(.., decode)` with the format's reader, the `<part>::handlers(),` line) is written once and read by both `unreached_publishers` and `unconsumed_streams`.

Both arms run under `CURSOR_MODEL` `composer-2.5` over `train`, the engine binary of the round above and the component rebuilt with the three changes:

1. **Arm A, three runs**, the revision of `18-14-21Z` rebuilt, each a card of its own:
   ```bash
   CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T18-14-21Z --target omnia train
   ```
   The revision whose build left the stream unconsumed; its REQ-002 reads right, and its design names XML in `## Technical logic` alone, in a paragraph citing nothing, so the wire reads for both slices. Its control is that card's own build: not driven.
2. **Arm B, three runs**, fresh:
   ```bash
   CURSOR_MODEL=composer-2.5 EVAL_RUNS=3 cargo run -p evals -- --build --target omnia train
   ```
   Its control is `19-26-01Z`: one of three not driven, the other two 11 of 22.

The predictions and the decision rule were written before the code was changed or an arm built.

### Predictions

- **R1, the stream is consumed.** Every build of both arms ends with the stem's consumer registered through `.topic_with` over `quick_xml`; `unconsumed_streams` is seen (a `candidate rejected` carrying its header in `run-N.build.stderr`) at most once per run and only at the part's last slice, on the runs whose first candidate registered nothing.
- **R2, the decoder holds.** `default_decoders` fires on no run and no run's files record `malformed JSON payload`; the per-part read is asserted by `omnia_wire_read`, since the brief is not written to the run's files.
- **R3, every run is driven and holds the floor.** The oracle finds a topic on every run of both arms, every run passes at least 11 of 22, the floor the five driven runs of the round above set (11, 11, 13, 15, 13), and no run fails a replay fixture on `no_actual_update`.
- **R4, specify holds.** On arm B the stem set is `{"train-update"}`, leaks are 0, requirement recall is within 10–13 of 14, and the design-draft turn is corrected at most once.

### Decision rule

- **The arm holds** when R1–R4 hold; the rule and the per-part read stand.
- **R1 fails with no rejection logged:** the inbound-paragraph read missed the bullet; read the lead sentences of `## APIs and integrations` in `run-N.design.json` and widen the words before anything else.
- **R1 fails with a rejection the model answered by a route, a `start` crate, or a requirement of its own:** the header is the lever.
- **R3's floor fails on a driven run:** the per-fixture failures are logic defects recorded against the build turn; the departure-overwrite and `externalId` misreads are counted across the six runs, and a check is proposed only for one that recurs on two or more.
- **R4 fails on recall alone:** variance, no action.

### Results

Six runs, all `composer-2.5` over `train` on the rebuilt component: arm A the cards `2026-11-07T23-12-52Z`, `23-21-26Z`, and `23-29-44Z`, each the `18-14-21Z` revision rebuilt; arm B the card `23-41-59Z`, three fresh runs. (A first launch of arm A ran on the component of the round above, since the shell's `CARGO_TARGET_DIR` had sent the rebuild elsewhere; it was stopped at its first slice and parked under `target/eval/arms/stream-aborted-stale-component-23-09-16Z/`, and the component was rebuilt into the tree the eval reads before any run below.)

| arm | card · run | wire read | first SLICE-002 candidate | rejections | registration | oracle |
| --- | --- | --- | --- | --- | --- | --- |
| A | `23-12-52Z` | XML, from `## Technical logic` | every requirement a `pub fn`, `Handlers::from([])` | 1: SLICE-002 `unconsumed_streams` | `topic_with(topic::REALTIME_TRAIN, on_train_update::<P>, decode_train_update)` over `quick_xml::de::from_reader` | 22/22 |
| A | `23-21-26Z` | the same | the same | 1: the same | `topic_with(topic::TRAIN_UPDATE_INBOUND, ..)` over `quick_xml` | 12/22: `locationData.gpsAccuracy` absent on the 10 publishing fixtures (the field named `accuracy`) |
| A | `23-29-44Z` | the same | the same | 1: the same | `topic_with(topic::TRAIN_UPDATE, ..)` over `quick_xml::de::from_reader(Cursor::new(..))` | 7/22: `gpsAccuracy` absent on 10; 5 skips answered as refusals (`not_relevant_station` ×4, `no_vehicles` ×1) |
| B | `23-41-59Z` · 1 | XML, from the bullet | the same | 1: SLICE-002 `unconsumed_streams` with `unreached_publishers` | `topic_with(topic::TRAIN_UPDATE, ..)` over `quick_xml` | 6/22: no `key` header on 10 (its specification lacks `publish-message-key`); 5 skips as refusals; `static/0010` published where a refusal was recorded; `replay/4145` refused as before the minimum delay |
| B | `23-41-59Z` · 2 | the same | the same | 1: the same two | `topic_with(topic::TRAIN_UPDATE, ..)` over `quick_xml` | 11/22: `no_actual_update` on 7 replay fixtures; 4 fixtures panicked on the allocation lookup under `externalRefId=9299669`, the registration number, where the reference sends the train id |
| B | `23-41-59Z` · 3 | the same | the same | 2: SLICE-001 `stem_manifest_drift`, SLICE-002 the same two | `topic_with(topic::TRAIN_UPDATE, ..)` over `quick_xml` | 22/22 |

- **R1 holds.** Every build of both arms ends with the consumer registered through `.topic_with` over `quick_xml`. `unconsumed_streams` was seen exactly once per run, each time at SLICE-002, the part's last slice, and each time on a first candidate that had answered every requirement as a `pub fn` behind `Handlers::from([])`: six of six first candidates registered nothing, so the rule carried the registration on every run, and every correction added the topic's constant, the `topic_with` registration with its `decode`, the `quick-xml` line, and the entry's `<part>::handlers(),` in one round. The four distinct leads the designs wrote (`**Inbound message stream (Kafka consumer)** — Endpoints from …`, `**Inbound message stream (Kafka)** — …`, `**Inbound message stream** — hosts from …`, `**Inbound message stream** — Kafka consumers connect to …`) were each read and quoted. In arm B the same candidate was also refused by `unreached_publishers`, since the slice spelled `Publish`; in arm A it was not, so the new rule was the one lever there.
- **R2 holds.** `default_decoders` fired on no run and no run's files record `malformed JSON payload`.
- **R3 fails on the floor.** Every run was driven, six of six against five of seven in the round above. Oracle 22, 12, 7 (arm A) and 6, 11, 22 (arm B): two runs under the 11 floor, and `no_actual_update` on a replay fixture on one run (B2). The failures are logic, none a decoder's or a registration's.
- **R4 holds.** Arm B's three specifications: stems `{"train-update"}`, leaks 0, recall 13/14 on each, the design-draft turn corrected once on each (25s, 25s, 23s), specify wall 92s, 72s, 73s. Builds 477s, 461s, 687s (arm A) and 556s, 516s, 644s (arm B).

**Reading under the rule.** R1, R2, and R4 hold, so the rule and the per-part read stand: the inbound stream is consumed on every build, where two of seven builds left it unconsumed before, and the consumer decodes the wire the design names. R3's floor fails on two driven runs, so the per-fixture failures are read as the rule says, counted across the six:

- **The departure overwrite applied to an arrival: 0 of 6** (2 of 5 in the round above). **The registration number taken for the external id: 1 of 6** (B2's allocation lookup sent `numeroRegistro` where the reference sends the train id; the round above had it once in `remoteData.externalId`). Neither recurs within the round, so no check.
- **A skip answered as a refusal: 2 of 6** (A3, B1). REQ-008, REQ-009, and REQ-010 state `produces no vehicle events and increments a not-relevant-… counter`, and their ids lead with `skip-` in all four revisions (`skip-filtered-station`, `skip-no-vehicles`, `skip-unfiltered-station`, …); the two builds returned `Err(bad_request)` from the consumer for them, which the host would retry, where the reference answers `Ok(())` after the counter with nothing published. It recurs, so a check is proposed: `refused_skips`, over a part registering a consumer, a function laid under a `// REQ-…` line whose statement says the message produces no events and names no rejection (`rejected`, `invalid`, `refused`) while its body spells `bad_request!` or answers `Err(..)`, the repair `Ok(())` after the counter and nothing published; keyed on the statement rather than the `skip-` id, since B3's `skip-after-validation-failure` names a refusal's consequence. With it, a `## A skipped message` passage in `handlers.md` that the consumer idiom selects. Not built in this round.
- **`locationData.gpsAccuracy` absent: 2 of 6**, both arm A. The base revision's design is the older engine's, with no `## Type:` blocks (`VehicleEvent` and `LocationData` are `connector-common`'s), and its one mention is prose, `` `locationData` gps accuracy 0 ``, so the wire key was the turn's to guess, `gpsAccuracy` once and `accuracy` twice. The fresh designs spell `gpsAccuracy` in two of three (B1, B3). A specify-side gap, not the build's: a field assigned on an imported class (`locationData.gpsAccuracy = 0`) is a wire key the typescript adapter's `excerpt` should carry verbatim. No check.
- **`no_actual_update` on the replay fixtures: 1 of 6 built, 2 of 3 specified.** B2's build read its REQ-002 scenario, `has departed without a positive actual departure time, or has arrived without a positive actual arrival time`, as the `or` it says. B1's scenario says the same `or` and its build read the body instead; B3's says `when departed else actual arrival time when arrived` and reads right. With `08-14-57Z`, three of the nine fresh `composer-2.5` specifications (`08-06-00Z`, `08-14-57Z`, `18-14-21Z`, `19-26-01Z` ×3, `23-41-59Z` ×3) flatten the source's `if / else if` into an `or` in the scenario `when` while the body keeps the precedence, so the "variance" reading above is withdrawn: it is a spec-draft pattern, the scenario's `when` losing the statement's precedence, for the engine's spec-draft turn to hold (the typescript `extract.md` precedence rule reaches the claim, not the scenario). Outside this round.
- **The rest, 1 of 6 each:** no `key` header (B1, whose specification dropped the key clause from its publish requirement, as `19-26-01Z` run 2's did: 2 of 6 fresh specifications across the two rounds); `static/0010` published where a refusal was recorded and `replay/4145` refused as before the minimum delay (B1, the delay computation).

Follow-ups, in this order: the `refused_skips` check and the `## A skipped message` passage; the scenario `when` precedence at the engine's spec-draft turn; the wire key of a field assigned on an imported class in the typescript adapter's `excerpt`; the two rules of the round above stand as built.

## The skipped message

The first follow-up of the round above, built in the build-quality round ([scenarios.md](scenarios.md#the-next-arm) carries that round's express-orders arm; this section is its `train` arm). A skip answered as a refusal was 2 of 6 there: REQ-008, REQ-009, and REQ-010 say the message `produces no vehicle events and increments a not-relevant-… counter`, and two consumers returned `Err(bad_request!(..))` for them, which the host reads as a rejected delivery and may redeliver, where the reference answers `Ok(())` after the counter with nothing published.

- **Check.** `refused_skips`, at a build, over the stems of the slice that register a consumer or whose part the design lists an inbound stream for: a requirement of the slice whose statement says the message produces no events, or is skipped, ignored, or discarded, and names no refusal (`rejected`, `refused`, `invalid`, `malformed`, `fails`, `error`), is read from its `// REQ-…` line to the next marker or the item's end, and an `Err(..)`, a `bad_request!`-family macro, or an `Error::bad_request(..)` within that window is one finding at its line, the header naming the repair: the `tracing` counter event, `Ok(())`, nothing published. Keyed on the statement rather than the `skip-` id, since `skip-after-validation-failure` names a refusal's consequence and its `Err` is right.
- **Passage.** `## A skipped message` in `handlers.md`, selected by a new idiom over the slice's own words (`skip`, `skipped`, `ignored`, `irrelevant`, `relevant`): a delivery the rules set aside answers `Ok(())` after its counter and publishes nothing, `Err` being the malformed or invalid delivery's alone.

The arm is the revision of `18-14-21Z` rebuilt three times, each a card of its own, on the component carrying the check and the three levers of the express-orders arm:

```bash
CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T18-14-21Z --target omnia train
```

Its control is arm A of the round above (22, 12, 7 of 22 on the same revision), the two rules of that round standing as built.

### Predictions

- **S1, no skip is refused.** No run's `run-N.oracle.log` records a fixture refused under `not_relevant_station`, `not_relevant_type`, `no_vehicles`, or `no_stop_info`; the five fixtures A3 refused (`static/0003`, `0004`, `0005`, `replay/0049`, `4247`) answer `0 events` and pass on every run. `refused_skips` is seen at most once per run (a `candidate rejected` carrying its header in `run-N.build.stderr`), at the slice whose consumer is filled, and on no run twice.
- **S2, the floor holds.** Every run is driven, registers the consumer through `.topic_with` over `quick_xml` (`unconsumed_streams` at most once per run, as before), and passes at least 11 of 22.
- **S3, the other classes are counted, not moved.** `gpsAccuracy` absent (2 of 6 above, the base design's gap) recurs on the runs whose turn guesses the key; the registration number for the external id and the departure overwrite stay at or under 1 of 3 each; no `no_actual_update` on this revision's replay fixtures.

### Decision rule

- **The arm holds** when S1 and S2 hold on all three runs; the check and the passage stand.
- **S1 fails with the check never seen:** the statement words missed the revision's phrasing; the three statements are read from `run-1.spec.json` against the word list before anything else.
- **S1 fails with the check seen and the fixture still refused:** the repair was made elsewhere than the skip's path (a refusal moved into the decoder, or the counter answered `Err`); the header is the lever.
- **S2's floor fails:** the per-fixture failures are logic defects recorded against the build turn, counted across the three runs with the six above, and a check is proposed only for one that recurs on two or more of these three.
- **S3:** a class at 2 of 3 is a check proposed; `gpsAccuracy` stays the specify-side follow-up whatever its count, since the base design spells no key.

### Results

Three runs, each the `18-14-21Z` revision rebuilt under `composer-2.5` on the component carrying `refused_skips` and the express-orders levers: the cards `2026-11-08T02-14-22Z`, `02-24-15Z`, and `02-35-01Z`. The oracle is read from `run-1.fidelity.md`'s `## Reference comparison`, the tree from the label branch of each card's `.emery/vcs/repos/run-1`, and the failed slice of `02-35-01Z` from the worktree its build left under `.emery/vcs/worktrees/SLICE-002`.

| card | first SLICE-002 candidate | rejections | registration | the five skip fixtures | oracle | build |
| --- | --- | --- | --- | --- | --- | --- |
| `02-14-22Z` | every requirement a `pub fn`, `Handlers::from([])` | 1: SLICE-002 `unconsumed_streams` | `topic_with(topic::TRAIN_UPDATE_INBOUND, on_train_update, decode_train_update)` over `quick_xml::de::from_reader` | `0 events`, pass | 12/22: `locationData` laid as `{ gps: { accuracy, latitude, longitude, speed } }`, so all four keys absent on the 10 publishing fixtures | 546s |
| `02-24-15Z` | the same | 1: the same | `topic_with(topic::TRAIN_UPDATE_SOURCE, ..)` over `quick_xml` | `0 events`, pass | 12/22: the same nested `gps` object | 590s |
| `02-35-01Z` | the same | 2: SLICE-002 `unconsumed_streams`, then `format_settings` on the corrected candidate | `topic_with(topic::REALTIME_TRAIN_SOURCE, ..)` over `quick_xml`, in the rejected candidate | not driven | not run: the build exited `bad_request` at SLICE-002, `budget exhausted` after `attempts=2` | 518s, exit 1 |

- **S1 holds.** No run's oracle records a fixture refused under `not_relevant_station`, `not_relevant_type`, `no_vehicles`, or `no_stop_info`; `static/0003`, `0004`, `0005`, `replay/0049`, and `4247` answer `0 events` and pass on both driven runs, against the control's 7/22 run refusing all five. `refused_skips` was seen on no run: the three trees write `skip_filtered_station`, `skip_no_vehicles`, and `skip_no_stop_info` answering `Ok(..)` after the counter event, and the consumer returns `Ok(())` at each, so the check had nothing to find; whether the `## A skipped message` passage steered the turn or the turn would have written it so is not told apart by this arm (the control's two refusals were 2 of 6 on the same model).
- **S2 fails on the third run, not on the floor.** `02-14-22Z` and `02-24-15Z` are driven, register the consumer through `.topic_with` over `quick_xml` after one `unconsumed_streams` correction at SLICE-002, and pass 12 of 22, over the 11 floor. `02-35-01Z`'s first SLICE-002 candidate registered nothing, as every first candidate of this slice has (9 of 9 across this round and the one above); the corrected candidate registered the consumer and decoded the XML, and added a `DATE_FORMAT` field to `TrainUpdateSettings` for the `DD/MM/YYYY` issue date the other two trees hold as the literal `"%d/%m/%Y"`, which `format_settings` refused; the backend allows one correction round per turn, so the slice failed `bad_request` and the build exited 1 with SLICE-001 merged. Two rules in series on one slice end a build: the first spends the round on a registration the design states before the turn, and any second finding has no round left.
- **S3 counts.** `gpsAccuracy` absent: 2 of 2 driven (4 of the 5 driven runs of this revision across the two rounds), both as the same shape, the four location fields nested under a `gps` object, so `latitude`, `longitude`, and `speed` are absent with it; the base design's gap, the specify-side follow-up as the rule says. The registration number for the external id: 0 of 2. The departure overwrite applied to an arrival: 0 of 2 (`static/0001` and `replay/4156` differ in the location keys alone). `no_actual_update`: 0 of 2.

**Reading under the rule.** S1 holds, so the check and the passage stand; the check is a backstop that this arm did not need. S2 fails on `02-35-01Z` with no logic defect to count: the failure is the correction budget, spent on `unconsumed_streams` first on every run. The registration is something the plan knows before the turn (`Scaffold::inbound()` and `Scaffold::last()` are read at `plan`), so the lever is to put it in the brief's scaffold section at the part's last slice, the recipe `tree/lists.rs`'s `registration(wire)` already spells for the finding, or to lay the `.topic_with(..)` registration and its `decode` stub by the scaffold where the stream's requirements carry no `Trigger::Message`; the rule stays as the check behind either. Recurring on 3 of 3, proposed, not built in this round. The `DATE_FORMAT` setting is 1 of 3 and already a rule's; nothing more.

Follow-ups, in this order: the inbound registration stated in the brief before the turn, or laid by the scaffold; the scenario `when` precedence and the wire key of a field assigned on an imported class, both at specify, as above.

## The registration before the turn

The first follow-up of the round above, built in the second build-quality round ([scenarios.md](scenarios.md#the-gate-by-code) carries that round's express-orders arm; this section is its `train` arm). Every first SLICE-002 candidate of the `18-14-21Z` revision has registered nothing, 9 of 9 across the two rounds above, `unconsumed_streams` corrected each, and on `02-35-01Z` a second finding on the corrected candidate (`format_settings`) had no round left, so the build exited 1. The registration is a fact the plan holds before the turn: `Scaffold::inbound()` is the stream's bullet, `Scaffold::last()` says the slice is the part's last, `Scaffold::wire()` the payload's format, and `registration(wire)` the recipe the finding's header already spells.

The round above left the choice between stating the registration and laying it. This arm states it, for one reason: a laid `.topic_with(..)` needs a topic constant, and nothing gives code one. The source reads its topic from a setting (`KAFKA_SOURCE_TOPIC`), the survey derived no `Trigger::Message` for any requirement of the revision, and the design's `dev-realtime-train.v1` is the model's rendering of the source's fallback, not a literal the survey bound. Laying would be the scaffold inventing a trigger the survey never derived, which the engine's rule refuses: code decides what code can, and the `Trigger` variant is closed. So the brief's scaffold section states it at the part's last slice, and the component changes in `adapters/omnia/` alone:

- **The recipe shared.** `registration(wire)` moves from `tree/lists.rs` to sit beside `Wire` in `scaffold/reference.rs`, so the brief and the two checks that name it (`unconsumed_streams`, `unreached_publishers`) spell one recipe.
- **The statement.** Where the plan holds an inbound stream and the slice is the part's last, the brief names, for each stem the bullet's requirements lead with (every stem of the slice where it cites none), the bullet's lead sentence, that this slice is the part's last so the registration is written now or never, the recipe over the wire the plan read (`.topic_with(..)` over the part's `decode` for XML, `.topic(..)` where the wire is JSON or unknown), the handler calling the part's functions in the order the requirements state, and the topic as the design names it for the stream. A stem whose scaffold lays a consumer is skipped, since the registration is laid. A stem whose crate already registers a topic, read at `plan` from the crate's modules, gets the sentence that the stream is consumed there and no second registration is wanted.
- **The check stays.** `unconsumed_streams` is the backstop behind the statement, unchanged.

The arm is the revision of `18-14-21Z` rebuilt three times, each a card of its own, on the component carrying the statement and the express-orders arm's gate:

```bash
CURSOR_MODEL=composer-2.5 cargo run -p evals -- --rebuild 2026-11-07T18-14-21Z --target omnia train
```

Its control is the round above (12, 12, and one build lost on the same revision; 9 of 9 first candidates registering nothing across the two rounds).

### Predictions

- **T1, the first candidate registers.** `unconsumed_streams` is seen on no run (0 of 3, against 9 of 9 first SLICE-002 candidates above); every first SLICE-002 candidate registers the consumer through `.topic_with` over `quick_xml`.
- **T2, the floor holds.** Every run is driven and passes 12 of 22, the base design's `locationData` gap capping it where it capped the two driven runs above; no fixture is refused under `not_relevant_station`, `not_relevant_type`, `no_vehicles`, or `no_stop_info`; `refused_skips` is seen on no run.
- **T3, no build is lost.** No run exits on the correction budget; at most one rejection per run, and none of them `unconsumed_streams`.
- **T4, the other classes are counted, not moved.** `gpsAccuracy` absent stays the base design's gap on every driven run; the registration number for the external id and the departure overwrite stay at or under 1 of 3 each; a `format_settings` or other rule's rejection is recorded where seen.

### Decision rule

- **The arm holds** when T1–T3 hold on all three runs; the statement stands and the check is its backstop.
- **T1 fails with the first candidate still registering nothing:** the turn did not act on the statement; laying is proposed, with the topic read from the design's clause beside the setting, and not built in this round.
- **T1 fails with a registration under `.topic(..)`:** the wire paragraph is the lever, read against `default_decoders`.
- **T3 fails on a second rule in series:** that rule's lever, named in the results.
- **T2's floor fails on a driven run:** logic defects, counted across the three runs with the driven runs above; a check is proposed only for one recurring on two of these three.

### Results

Not yet run.
