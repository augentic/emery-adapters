# Source Adapter Examples

Live `extract` walks via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor): one example per first-party adapter drives the built component over `emery:adapter/source` — the same seam the engine crosses — through the host model, and prints the claims it answers with.

Each example is a directory: a wasm32 driver guest (`guest.rs`), the deployment it runs in (`omnia.toml`: the link, the driver, the adapter, the tree it lends), and — where the input is a workspace — the fixture it lends. The [runtime](runtime.rs) host is shared: command mode over the Cursor model, compiling nothing in, so one host serves every adapter.

| Example | Input | Lends |
| --- | --- | --- |
| [documentation](documentation/) | workspace | [documentation/docs/](documentation/docs/) — the orders service specification |
| [intent](intent/) | inline value — the brief after `--`, or the built-in one | nothing |
| [typescript](typescript/) | workspace | [typescript/src/](typescript/src/) — the orders service behaviour |

These are walks of the seam, not the graded live eval: nothing is scored, and the `emery` binary is not involved.

## Prerequisites

- [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge). See [below](#installing-cursor-sdk-bridge) for installation.
- `CURSOR_API_KEY`

## Build and run

```bash
# build the adapter component and its driver guest
make adapter documentation
cargo build -p emery-adapters --example documentation --target wasm32-wasip2 --release

# run the example
export CURSOR_API_KEY=<Cursor API key>
cargo run -p emery-adapters --example runtime -- run --config examples/documentation/omnia.toml

# the intent example takes its brief after `--`; without one it puts the built-in brief
cargo run -p emery-adapters --example runtime -- run --config examples/intent/omnia.toml -- \
  "Ship the orders API with idempotent retries."
```

`make example <name>` runs the three steps for one adapter; anything after the name is the driver's argv.

The `omnia.toml` binds the built component and the driver by path relative to itself and marks the driver as the command guest; the driver dispatches to the adapter by its guest id. Host logging is `RUST_LOG` (for example `RUST_LOG=info,omnia_cursor=debug,opentelemetry_sdk=off`); the `run` grammar has no `--debug`.

## What to expect

The driver reports the adapter's `emery-version` pin on stderr, then prints the evidence on stdout: the document's authority and one entry per claim — kind, id, source anchor, and the statement (or the extra its kind carries instead). Any claim-gate finding is listed after the claims; the SDK already corrects candidates against the gate guest-side, so an empty list is the norm.

```text
documentation: emery-version 0.38.0
authority: documentation
- requirement order.place @ orders-api.md#L10-L13
  A client places an order by submitting the customer id, the line items (product id plus quantity, at least one line), and an optional courier note. The service validates the submission, assigns an order id, and answers with the created order.
- criterion order.place.no-line-items @ orders-api.md#L15
  An order with no line items is rejected.
- criterion order.cancel.shipped-conflict @ orders-api.md#L32-L33
  Cancelling a `shipped` order is refused with a conflict answer that names the current state.
```

The intent example's claims anchor to `[unknown]`: an inline brief has no path.

An adapter refusing its input (an empty brief, an unreadable tree) prints the refusal's code and description and exits `1`.

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
