# Source Adapter Examples

Live `specify` journeys via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor): the shipped `emery` binary loads a built first-party component by path, the adapter extracts its fixture through the host model, the engine synthesises `spec.md` / `design.md`, and the revision commits — the same walk as the engine repository's [example](https://github.com/augentic/emery/tree/main/examples), over the adapters this repository ships.

Each example is an `emery.toml` naming its sources — the built component and the input it reads — plus, where the input is a workspace, the fixture it lends. Nothing here is compiled: the configs are data the `emery` binary runs, so an example exercises the adapter exactly as an operator's project would.

| Example                         | Input                                     | Lends                                                                                                                   | Exercises                                                                   |
| ------------------------------- | ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [all three](emery.toml)         | the rows below, as one run                | both trees                                                                                                              | the grouping judgment and authority precedence over the one orders service  |
| [documentation](documentation/) | workspace                                 | [documentation/docs/](documentation/docs/) — the orders service specification                                           | the `Workspace` arm over prose                                              |
| [typescript](typescript/)       | workspace                                 | [typescript/src/](typescript/src/) — the orders service behaviour                                                       | the `Workspace` arm over code                                               |
| [intent](intent/)               | inline value — the brief in `description` | nothing                                                                                                                 | the `Value` arm; claims anchor to `[unknown]`, an inline brief has no path  |

These are walks, not the graded live eval: nothing is scored. They are also, by construction, the cases the eval runner will drive once it is recreated.

## Prerequisites

- The `emery` binary on `PATH`. There is no published binary; build it from source (the `wasm32-wasip2` target this repository already needs):

  ```bash
  # track the engine repository's main
  cargo install --git https://github.com/augentic/emery --locked

  # or a sibling checkout, matching the path patches in Cargo.toml
  cargo install --path ../emery --locked
  ```

  Adapters pin the `emery-sdk` version they were built against as their `emery-version`; an `emery` older than that pin refuses `unsupported-version` (exit `1`) naming the version it needs. Reinstall after the `emery-*` pin in [`Cargo.lock`](../Cargo.lock) moves. Without installing, `cargo run --manifest-path ../emery/Cargo.toml --release -- specify …` runs a sibling checkout and keeps the working directory.

- [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge). See [below](#installing-cursor-sdk-bridge) for installation.
- `CURSOR_API_KEY`

## Build and run

Run from the repository root. `emery` mounts the invocation directory as the project, so every path an `emery.toml` names — the built component under `target/`, the fixture under `examples/` — must sit inside it; from any other directory the config is refused as escaping the project.

```bash
# build every adapter component (or one: cargo build -p documentation --target wasm32-wasip2 --release)
make release

# run one example
export CURSOR_API_KEY=<Cursor API key>
emery specify --debug --config examples/documentation/emery.toml

# review the committed revision
emery show spec
emery show design
```

The config binds the built component by path relative to itself and names the tree it lends the same way; the component is read fresh on every run and carries no `digest` pin, since a development build changes. Host tracing is stderr and is selected by the reserved log flags, peeled before the engine sees argv: bare invocations print INFO progress, `--debug` adds backend tracing, `--quiet` turns it off. The semantic result stays on stdout.

Revision state lives under `.omnia/storage` in the invocation directory (ignored by git), one revision per directory: each run replaces the last, and the success line reports the re-mine diff against the revision it displaced. Run the combined [`emery.toml`](emery.toml) to see all three adapters contribute to one specification.

*Extract* and *synthesis* both complete through the Cursor backend: one extraction turn per source, then the spec and design turns, plus a grouping turn when two or more sources are named, and a bounded correction round for any candidate the engine's checks reject.

## What to expect

`specify` prints the committed revision id and, on a re-run, the diff against the revision it displaced:

```text
committed revision 3f9c…
  diff vs 8a12…:
    spec.md + REQ-004 order.cancel
    design.md ~ preamble
```

`show spec` prints the Markdown projection of the current revision — the `emery:` / `revision:` front matter, then the specification: a preamble, and one requirement per reconciled subject carrying its id, sources, status, the winning statement as its body, and the scenarios the model drafted for it:

```text
---
emery: 2
revision: 3f9c…
---

# Specification

…

### Requirement: order.place

ID: REQ-001
Sources: [docs:order.place, api:order.place]
Status: …

A client places an order by submitting the customer id, the line items (product id plus quantity, at least one line), and an optional courier note. The service validates the submission, assigns an order id, and answers with the created order.

#### Scenario: Order placed

- **GIVEN** …
```

Failures are typed and exit non-zero: an adapter refusing its input (an empty brief, an unreadable tree) or a claim still missing its required extra after the backend's correction rounds is `bad_request` (exit `1`); a model or component-acquisition failure is `bad_gateway` (exit `4`); `show` before any run is `spec-not-generated` (exit `2`).

## Host-to-guest tool calls

The only tools an adapter's completion session declares are the reference tools — `list_docs` and `read_doc` — over its embedded prose corpus. `wasi-model` delivers them as two streams rather than direct callbacks: the host writes each `ToolCall` to the session's `calls` stream, and the guest answers with a `ToolResult` on a second stream it created and passed to `create`, carrying the same correlation ID so the host can resume the completion.

Every answer is served in-process from the embedded corpus (`emery_sdk::references`): `list_docs` returns the embedded document paths, `read_doc` returns one document body by adapter-relative path, and anything else comes back as a repairable error. The model reaches nothing but the adapter's own reference documents and the lent tree.

## Installing cursor-sdk-bridge

```bash
# download and install
curl -fsSL -o /tmp/cursor-sdk-bridge.tar.gz \
  https://github.com/cursor/sdk-bridge/releases/latest/download/cursor-sdk-bridge-standalone-darwin-arm64.tar.gz \
  && tar -xzf /tmp/cursor-sdk-bridge.tar.gz -C /tmp \
  && install /tmp/bin/cursor-sdk-bridge ~/.local/bin/cursor-sdk-bridge

# verify
cursor-sdk-bridge --help
```

See [bridge docs](https://cursor.com/docs/sdk/bridge) for more information.
