# Omnia Worked Examples

The **primary worked-code reference is the exemplar checkout** — a compiling, CI-green Omnia workspace the build's preparation leg places at `target/omnia-exemplar/` (see [`exemplar.md`](../exemplar.md)). Read real crates, guests, and test suites there first: every supported capability trait, its mock provider, and its test shape compiles upstream (`Broadcast`, `BlobStore`, `DocumentStore`, and `TableStore` in `crates/capability-examples/`; the rest across the transit crates).

This folder retains explanatory walkthroughs **only** for subjects the exemplar cannot demonstrate: consumer-repo change orchestration and Emery capture-replay wiring. Missing coverage is upstream backlog: extend the exemplar rather than growing this tree.

Pick the one file that matches the task; do not read the tree wholesale.

## crates/

| File | Read when |
| --- | --- |
| [crates/anti-patterns.md](crates/anti-patterns.md) | Reviewing generated code for known bad shapes before they ship. |

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
