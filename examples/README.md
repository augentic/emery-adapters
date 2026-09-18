# Source Adapter Examples

Live `specify` journeys via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor): the shipped `emery` binary loads a built first-party adapter by path, the adapter extracts claims from its fixture through the host model, the engine synthesises `spec.md` / `design.md`, and the revision commits.


| Example                                   | Input                                        | Exercises                                                     |
| ----------------------------------------- | -------------------------------------------- | ------------------------------------------------------------- |
| [documentation](documentation/emery.toml) | [docs/](documentation/docs/) — specification | `Workspace` over prose                                        |
| [typescript](typescript/emery.toml)       | [src/](typescript/src/) — behaviour          | `Workspace` over code                                         |
| [intent](intent/emery.toml)               | inline `description`; nothing lent           | `Value`; claims anchor `[unknown]`                            |
| [all three](emery.toml)                   | the three sources above                      | grouping and authority (`intent > documentation > behaviour`) |




## Prerequisites

1. The `emery` binary on `PATH`. There is no published binary:
  ```bash
  cargo install --git https://github.com/augentic/emery --locked
  ```
2. [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge). See [below](#installing-cursor-sdk-bridge) for installation.
3. `CURSOR_API_KEY`



## Build and run

Run from the repository root: `emery` mounts the invocation directory as the project, and every path an `emery.toml` names must sit inside it.

```bash
# build every source adapter into target/wasm32-wasip2/release/<name>.wasm
cargo build --workspace --target wasm32-wasip2 --release

# run the example
export CURSOR_API_KEY=<Cursor API key>
emery specify --debug --config examples/documentation/emery.toml

# review the committed revision
emery show spec
emery show design
```

Swap the config for [intent](intent/emery.toml), [typescript](typescript/emery.toml), or the combined [emery.toml](emery.toml). One adapter alone builds with `cargo build -p <name> --target wasm32-wasip2 --release`, to the same path.

The config binds the shipped component by path relative to itself, read fresh on every run. A bare name still only dispatches guests declared in the runtime invocation, and the shipped `emery` binary declares none. Revision state lives under `.omnia/storage` in the invocation directory; each run replaces the last and reports the diff against it.

*Extract* and *synthesis* both complete through the Cursor backend. Each adapter answers reference-tool calls in-process the same way the [omnia-cursor example](https://github.com/augentic/omnia-backends/tree/main/examples/cursor) does.

See [#host-to-guest-tool-calls](#host-to-guest-tool-calls) for more detail.

## Host-to-guest tool calls

The only tools an adapter's completion session declares are the reference tools — `list_docs` and `read_doc` — over its embedded prose corpus. `wasi-model` delivers them as two streams rather than direct callbacks: the host writes each `ToolCall` to the session's `calls` stream, and the guest answers with a `ToolResult` on a second stream it created and passed to `create`, carrying the same correlation ID so the host can resume the completion.

Every answer is served in-process by the SDK from the adapter's listed `PROSE`: `list_docs` returns the embedded document paths, `read_doc` returns one document body by adapter-relative path, and anything else — an unknown tool, malformed arguments, an unembedded path — comes back as a repairable error. No HTTP shelf, no MCP callback, and no access to the source input or the revision store crosses this boundary; the model reaches nothing but the adapter's own reference documents.

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