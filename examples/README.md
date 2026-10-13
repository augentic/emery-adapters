# Source Adapter Examples

Live `specify` and `build` runs through [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor).

`specify` loads the named source adapters, extracts claims, and commits a revision under `.emery/storage/blobstore/`. `show` prints it. `build` builds the current plan through one target adapter.

Run from the repository root. `emery` mounts that directory as the project, and every path an `emery.toml` names must sit inside it.

## Prerequisites

1. The [emery](https://github.com/augentic/emery) repository as a sibling checkout.
2. [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge) on `PATH`. [Install](#installing-cursor-sdk-bridge).
3. `CURSOR_API_KEY` in `.env`, or `export CURSOR_API_KEY=<key>`.



## Specify

```bash
cargo build --workspace --target wasm32-wasip2 --release

mkdir -p ~/.emery/adapters
cp target/wasm32-wasip2/release/emery_typescript.wasm ~/.emery/adapters/emery_typescript@0.13.0.wasm

set -a; source .env; set +a
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/typescript/emery.toml

cargo run --manifest-path ../emery/Cargo.toml -- show spec
cargo run --manifest-path ../emery/Cargo.toml -- show design
cargo run --manifest-path ../emery/Cargo.toml -- show plan
```

Swap the config for [intent](intent/emery.toml), [python](python/emery.toml), [documentation](documentation/emery.toml), or the combined [emery.toml](emery.toml). Copy each source adapter that config names the same way (`emery_<name>@0.13.0.wasm`). Replacing the file is how a rebuild takes effect.

### Build

[emery.toml](emery.toml) names `emery:omnia` as the `[target]`. The adapter lays a whole tree, so it builds into a fresh repository: one whose root holds files alone, a `README.md` and a `.gitignore`. Lay a bare one with a root commit at the path the `[target]` table names:

```bash
git init --bare --initial-branch=main /tmp/emery-build.git
git init --initial-branch=main /tmp/emery-build.seed
cd /tmp/emery-build.seed
printf '.emery/\ntarget/\n' > .gitignore
printf '# shop\n' > README.md
git add --all && git commit --message "Greenfield"
git push -- /tmp/emery-build.git HEAD:refs/heads/main
cd - && rm -rf /tmp/emery-build.seed
```

After the `specify` above, build the current plan into it:

```bash
cp target/wasm32-wasip2/release/emery_omnia.wasm ~/.emery/adapters/emery_omnia@0.13.0.wasm

cargo run --manifest-path ../emery/Cargo.toml -- build -v --config examples/emery.toml

git -C /tmp/emery-build.git log --oneline --first-parent main..emery/<revision>
git clone --branch emery/<revision> /tmp/emery-build.git /tmp/emery-build
```

The result is the branch `emery/<revision>`, pushed to the bare repository as the `remote`. This checkout stays as it is. A `[target]` naming no `repository` builds from the checkout's own head, which the adapter refuses as `base-not-fresh` with the directories named, as it refuses any repository whose root holds a directory. A checkout with changes outside `.emery/` and `.git/` is refused as `base-not-sealed`, with the paths named.

The checks run `cargo` inside the worker boundary: `index.crates.io` and `static.crates.io` through the proxy, and the toolchain under `~/.rustup` and `~/.cargo/bin` with `RUSTUP_HOME` set. A rustup install in those paths is enough.

A build turn runs the toolchain as well as composing, so one can outlast `CURSOR_TIMEOUT_SECS`. Smaller slices are the fix — the engine floors a stem past twelve requirements at its sub-stems — and raising the variable for a build run is the fallback; it is process-wide, so it raises every turn's cap.

## Logging

Bare is `info`. `-v` and `-vv` raise it one step, `-q` and `-qq` lower it. `RUST_LOG` refines a crate:

```bash
export RUST_LOG="omnia_core=off,emery_sdk=trace"
export CURSOR_SDK_BRIDGE_LOG=1
```


| Level | Corrections                                                            |
| ----- | ---------------------------------------------------------------------- |
| bare  | `attempts` on the `completion` line                                    |
| `-v`  | Each finding: a `path` outside the seam's anchors, an id off its stems |
| `-vv` | The correction the model reads, and the accepted evidence as JSON      |


`RUST_LOG=emery_sdk=debug` shows the findings on their own.

## The store

An `adapter` is `emery:<name>@<version>`. `emery` reads `~/.emery/adapters/emery_<name>@<version>.wasm` before it fetches. The file there is that release on this machine.

These examples name `0.13.0`, so the `cp` above stands in for the published release. Remove the file and the next run fetches it. A `-dev` version in both the config and the filename keeps a local build beside the release.

## What a run looks like

Completions are quiet between log lines. An `in progress` line every 15s means the run is still working. Each `completion` line reports tokens read and written, and time in `opening_ms`, `tool_ms`, `model_ms`, and `check_ms`.

- A single small module is `evidence`, `spec-draft`, and `design-draft`.
- [typescript/emery.toml](typescript/emery.toml) and [python/emery.toml](python/emery.toml) parse the tree and open one `survey-<source>` turn, then one `evidence` seam (one per stem once the tree is past the inline budget). More than one stem adds `grouping` and `slicing`.
- [emery.toml](emery.toml) is the TypeScript tree plus the documentation source. `build` is one turn per slice, every slice of a wave at once, then one `verify` per wave.

Wall time varies. Pin `CURSOR_MODEL` when comparing prompts. Current numbers are in the [eval](../evals/README.md).


| Variable                 | Default | Effect                                                       |
| ------------------------ | ------- | ------------------------------------------------------------ |
| `CURSOR_TIMEOUT_SECS`    | 600     | Cap per completion, build turns included. A seam that hits it should be cut finer; a build turn, sliced smaller. |
| `CURSOR_INACTIVITY_SECS` | 120     | Cancels a stream silent while waiting on the bridge.         |
| `CURSOR_MAX_AGENTS`      | 4       | Live bridge agents. Further completions queue.               |
| `CURSOR_MODEL`           | `auto`  | Model for every completion.                                  |


What a build's worker can reach is the target adapter's grant. A `~/` root the home lacks is skipped. The proxy admits only the hosts that grant names.

## Reference tools

A completion's tools are `list_docs` and `read_doc`, answered in-process from the adapter's embedded prose. They return that corpus alone.

## Installing cursor-sdk-bridge

```bash
curl -fsSL -o /tmp/cursor-sdk-bridge.tar.gz \
  https://github.com/cursor/sdk-bridge/releases/latest/download/cursor-sdk-bridge-standalone-darwin-arm64.tar.gz \
  && tar -xzf /tmp/cursor-sdk-bridge.tar.gz -C /tmp \
  && install /tmp/bin/cursor-sdk-bridge ~/.local/bin/cursor-sdk-bridge

cursor-sdk-bridge --help
```

See the [bridge docs](https://cursor.com/docs/sdk/bridge).