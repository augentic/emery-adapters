# Omnia Worked Examples

The **primary worked-code reference is the exemplar checkout** — a compiling, CI-green Omnia workspace the generation leg clones per [`exemplar.md`](../exemplar.md). Read real crates, guests, and test suites there first.

This folder retains explanatory walkthroughs **only** for subjects the exemplar does not (yet) demonstrate. Missing coverage is upstream backlog: extend the exemplar rather than growing this tree.

Pick the one file that matches the task; do not read the tree wholesale.

## crates/

| File | Read when |
| --- | --- |
| [crates/anti-patterns.md](crates/anti-patterns.md) | Reviewing generated code for known bad shapes before they ship. |

### crates/capabilities/ — traits not exercised upstream

`Config`, `Publish`, `HttpRequest`, `Identity`, and `StateStore` are demonstrated by the exemplar's production code and tests; no static walkthroughs remain for them.

| File | Read when the spec requires |
| --- | --- |
| [crates/capabilities/broadcast.md](crates/capabilities/broadcast.md) | Pushing data to WebSocket clients (`Broadcast`). |
| [crates/capabilities/blobstore.md](crates/capabilities/blobstore.md) | Binary blob storage (`Blobstore`). |
| [crates/capabilities/documentstore.md](crates/capabilities/documentstore.md) | JSON document storage / queries (`DocumentStore`). |
| [crates/capabilities/tablestore.md](crates/capabilities/tablestore.md) | Tabular storage (`TableStore`). |

### crates/updates/ — change-classification checklists

The exemplar is a snapshot; update flows stay here as short procedural checklists (no full crate listings). Compiling shapes still come from the checkout.

| File | Read when the slice is classified |
| --- | --- |
| [crates/updates/additive.md](crates/updates/additive.md) | Additive — a new handler joins an existing crate. |
| [crates/updates/modifying.md](crates/updates/modifying.md) | Modifying — business logic changes inside existing handlers. |
| [crates/updates/structural.md](crates/updates/structural.md) | Structural — the domain model is refactored. |
| [crates/updates/subtractive.md](crates/updates/subtractive.md) | Subtractive — an endpoint or handler is removed. |

## replay/

Emery capture-replay wiring that is not Omnia-SDK idiom. Prefer the exemplar's `crates/pulse-adapter/tests` + `data/replay` for SDK replay shape first.

| File | Read when |
| --- | --- |
| [replay/handler.md](replay/handler.md) | Writing the replay handler entry point for a crate. |
| [replay/fixtures.md](replay/fixtures.md) | Authoring replay text fixtures from captured traffic. |
| [replay/tests.md](replay/tests.md) | Wiring replay fixtures into the crate's test suite. |

## tests/

HTTP / Publish / StateStore testing is demonstrated by the exemplar. Remaining walkthroughs cover traits the exemplar does not exercise.

| File | Read when testing |
| --- | --- |
| [tests/testing.md](tests/testing.md) | General testing-pattern overview (start here only if the exemplar suite is a poor match). |
| [tests/testing-blobstore.md](tests/testing-blobstore.md) | Blob storage over `Blobstore`. |
| [tests/testing-documentstore.md](tests/testing-documentstore.md) | Document storage over `DocumentStore`. |
