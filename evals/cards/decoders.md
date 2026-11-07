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

The reader's paragraph rule was added during the round, before any treatment run completed: the first treatment run's design (set aside under `target/eval/arms/decoders-treatment-aborted-19-16-16Z/`, specify only) rendered the prompt's sub-bullet as `Wire payload is XML; decoding uses `x2js` (`X2JS.xml2js`) before `ta-json` deserialisation under the configured root element`, two sentences the sentence rule as first written reads neither of, so the run was stopped at its first build and the arm restarted over the component carrying both rules. P1's second clause is read against the reader as restarted.

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

Not yet run. The control card, the treatment card, and the three isolation cards are read into this section against P1–P5.
