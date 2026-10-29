# Source Adapter Examples

Live `specify`, `amend`, and `build` runs through [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor).

`specify` loads the named source adapters, extracts claims, and stages one candidate revision in `.emery/storage`. `show <artifact> --staged` prints it. `apply` seals the staged candidate under `spec/` in the target's repository as the current revision, at one commit labelled `emery/<revision>`, and `specify --apply` does both in one run. `amend` extracts its change sources against the current revision and stages an amendment for `show amendment` and `apply`, which seals it as the next revision over its base (`amend --apply` in one run). `show <artifact>` prints the current revision, and `build` builds its plan through one target adapter.

Run from the repository root. `emery` mounts that directory as the project, and every path an `emery.toml` names must sit inside it.

## Prerequisites

1. The [emery](https://github.com/augentic/emery) repository as a sibling checkout.
2. [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge) on `PATH`. [Install](#installing-cursor-sdk-bridge).
3. `CURSOR_API_KEY` in `.env`, or `export CURSOR_API_KEY=<key>`.
4. No `.emery/` left by a pre-0.39 run. The engine then kept the revision in `.emery/storage`, so the labelled commit in the repository its `build` created holds no `spec/`, which every verb now refuses as `spec-mismatch`. Remove the directory (`rm -rf .emery`) and regenerate with a fresh `specify`; a revision under the old grammar is unreadable either way.

### Build adapters and copy to cache

```bash
cargo build --workspace --target wasm32-wasip2 --release

version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml)"
dst=~/.emery/adapters

for wasm in target/wasm32-wasip2/release/emery_*.wasm; do
  base=$(basename "$wasm" .wasm)
  cp "$wasm" "$dst/${base}@${version}.wasm"
done

set -a; source .env; set +a
```

## Specify

```bash
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/emery.toml

# show the staged candidate; no config needed
cargo run --manifest-path ../emery/Cargo.toml -- show spec --staged
cargo run --manifest-path ../emery/Cargo.toml -- show design --staged
cargo run --manifest-path ../emery/Cargo.toml -- show plan --staged
```

Swap the config for [intent](intent/emery.toml), [python](python/emery.toml), [documentation](documentation/emery.toml), [amend](amend/emery.toml), or the combined [emery.toml](emery.toml). Copy each source adapter that config names the same way (`emery_<name>@0.12.0.wasm`). Replacing the file is how a rebuild takes effect. A config with no `[target]` table stages a candidate for `show --staged`; sealing it, and showing the current revision, need one.

## Apply

[emery.toml](emery.toml) names `emery:omnia` as its target and no `repository`. After `specify`, seal the staged candidate:

```bash
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/emery.toml

# show the current revision
cargo run --manifest-path ../emery/Cargo.toml -- show spec --config examples/emery.toml
```

`apply` creates `.emery/vcs/repos/omnia`, seals the three documents under `spec/` there as the commit `Revision <revision>`, labels it `emery/<revision>`, and clears the staged candidate. A later `apply` seals over that commit, so the repository carries each revision in turn. `specify --apply` stages and seals in one run.

`<revision>` is the id `apply` prints — `sealed revision <revision> as emery/<revision> at <commit>` — and the repository lists the label any time:

```bash
git -C .emery/vcs/repos/omnia branch --list 'emery/*'
```

## Build

After `apply`, copy the adapter and build the current plan:

```bash
cargo run --manifest-path ../emery/Cargo.toml -- build -v --config examples/emery.toml

# switch to the label to view the build; `emery/<revision>` as listed above
git -C .emery/vcs/repos/omnia checkout emery/<revision>
```

The adapter builds into `.emery/vcs/repos/omnia` over the `Revision` commit and moves the label `emery/<revision>` to the result. It leaves this checkout unchanged and pushes nothing. Clone the branch, or push it from that repository.

A `[target] repository` URL builds into a clone of that repository from its default branch instead, and `repository = "."` into this checkout's own repository from its sealed head. The init scaffolds only the files of the layout the tree lacks, so a held project's files stand; a repository that is not an omnia guest is better left for the created one.

Build and verify run `cargo` with access to crates.io, `~/.rustup`, and `~/.cargo/bin`. If a turn exceeds `CURSOR_TIMEOUT_SECS`, use smaller slices or raise the limit for that run.

## Amend

[amend/emery.toml](amend/emery.toml) names `emery:typescript` over `evals/cases/orders/fixture/` (`OrderService` with `place`, `read`, and `cancel` under `order-service`) with `scope = "whole"`, and `emery:omnia` as its `[target]`. Seal the baseline revision from the whole fixture tree (`--fresh` if `.emery/vcs/repos/omnia` already holds an amended lineage from an earlier run), remove `cancel` from `orders.ts`, and run `amend`:

```bash
# seal the baseline revision from the whole fixture tree
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --apply --config examples/amend/emery.toml

# remove `cancel` (lines 67–75) from `OrderService` so the tree states `place` and `read` alone
python3 -c '
from pathlib import Path
p = Path("evals/cases/orders/fixture/orders.ts")
lines = p.read_text().splitlines(keepends=True)
p.write_text("".join(lines[:66] + lines[75:]))
'

# stage the amendment; `scope = "whole"` retires what the tree no longer states under `order-service`
cargo run --manifest-path ../emery/Cargo.toml -- amend -v --config examples/amend/emery.toml

# review the staged amendment and seal it as the next revision
cargo run --manifest-path ../emery/Cargo.toml -- show amendment
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/amend/emery.toml

# restore the fixture
git checkout -- evals/cases/orders/fixture/orders.ts
```

Under `scope = "whole"`, a behaviour source (`typescript`, `python`) states the stems its `requirement` claims cover whole: `amend` matches the remaining `place` and `read` claims against the standing requirements under `order-service` and retires the untouched `cancel` requirements by the source's silence, printing `retired (N by scope)` on the counts line and `slices: order-service (brief kept)` on the plan line. `show amendment` renders `## Retired` with ``Note: retired by scope: `typescript` was read whole and states nothing under `order-service`.`` beneath each retired requirement and `## Plan` with `### Slice: order-service [changed]` followed by `Note: the brief is the base's; no slicing turn redrafted it.`. `apply` seals the amendment as the next revision over its base (`amends <parent>`, with `Parent:`, `Base:`, `Added:`, `Changed:`, `Retired:`, and `Slices:` trailers on `emery/<revision>`). A prose source (`intent`, `documentation`) read with `scope = "whole"` retires nothing by silence and withdraws a behaviour through a `retire` claim alone.

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

These examples name `0.12.0`, so the `cp` above stands in for the published release. Remove the file and the next run fetches it. A `-dev` version in both the config and the filename keeps a local build beside the release.

## What a run looks like

Completions are quiet between log lines. An `in progress` line every 15s means the run is still working. Each `completion` line reports tokens read and written, and time in `opening_ms`, `tool_ms`, `model_ms`, and `check_ms`.

- A single small module is `evidence`, `spec-draft`, and `design-draft`.
- [typescript/emery.toml](typescript/emery.toml) and [python/emery.toml](python/emery.toml) parse the tree and open one `survey-<source>` turn, then one `evidence` seam (one per stem once the tree is past the inline budget). More than one stem adds `grouping` and `slicing`.
- [emery.toml](emery.toml) is the TypeScript tree plus the documentation source. `build` is one turn per slice, every slice of a wave at once, then one `verify` per wave.

Wall time varies. Pin `CURSOR_MODEL` when comparing prompts. Current numbers are in the [eval](../evals/README.md).

| Variable                 | Default | Effect                                                                                                           |
| ------------------------ | ------- | ---------------------------------------------------------------------------------------------------------------- |
| `CURSOR_TIMEOUT_SECS`    | 600     | Cap per completion, build turns included. A seam that hits it should be cut finer; a build turn, sliced smaller. |
| `CURSOR_INACTIVITY_SECS` | 120     | Cancels a stream silent while waiting on the bridge.                                                             |
| `CURSOR_MAX_AGENTS`      | 4       | Live bridge agents. Further completions queue.                                                                   |
| `CURSOR_MODEL`           | `auto`  | Model for every completion.                                                                                      |

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