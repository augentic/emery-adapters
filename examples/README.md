# Adapter Examples

This guide walks through running the first-party source and target adapters live using the `emery` CLI and the Cursor model backend ([`omnia-cursor`](https://github.com/augentic/omnia-backends/tree/main/crates/cursor)). You will build the WebAssembly (Wasm) adapters, stage and seal a specification (`specify` and `apply`), build the resulting Rust or TypeScript project (`build`), and evolve the specification incrementally (`amend`).

## Contents

- [Prerequisites](#prerequisites)
- [Step 1: Build and cache the adapters](#step-1-build-and-cache-the-adapters)
- [Step 2: Specify](#step-2-specify)
- [Step 3: Apply](#step-3-apply)
- [Step 4: Build](#step-4-build)
- [Step 5: Amend](#step-5-amend)
  - [Amend](#amend)
- [Reference](#reference)
  - [Adapter cache (`~/.emery/adapters`)](#adapter-cache-emeryadapters)
  - [Logging and verbosity](#logging-and-verbosity)
  - [Progress and environment variables](#progress-and-environment-variables)
  - [Reference tools](#reference-tools)
- [Installing cursor-sdk-bridge](#installing-cursor-sdk-bridge)

---

## Prerequisites

Run all commands from the root of the `emery-adapters` repository (`emery` mounts the working directory as `.`, so all paths in `examples/**/emery.toml` resolve relative to this root):

1. A sibling checkout of [`augentic/emery`](https://github.com/augentic/emery) at `../emery`.
2. [`cursor-sdk-bridge`](https://github.com/cursor/sdk-bridge) on `PATH` (see [Installing `cursor-sdk-bridge`](#installing-cursor-sdk-bridge)).
3. `CURSOR_API_KEY` set in `.env` or exported in your shell.
4. No leftover `.emery/` directory from pre-`0.39` runs. Older versions stored sealed revisions in `.emery/storage` instead of under `spec/` in the target Git repository, which causes `spec-mismatch` errors on current builds. If upgrading from an older version, remove `.emery/` first (`rm -rf .emery`).

---

## Step 1: Build and cache the adapters

Compile all workspace adapters for `wasm32-wasip2` and copy each `.wasm` component into `~/.emery/adapters/` under the workspace version:

```bash
cargo build --workspace --target wasm32-wasip2 --release

version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml)"
dst=~/.emery/adapters
mkdir -p "$dst"

for wasm in target/wasm32-wasip2/release/emery_*.wasm; do
  base=$(basename "$wasm" .wasm)
  cp "$wasm" "$dst/${base}@${version}.wasm"
done

set -a; source .env; set +a
```

Whenever you rebuild an adapter locally, re-run the `cp` loop above so `~/.emery/adapters/` holds your updated `.wasm` binary.

---

## Step 2: Specify

Run `emery specify` to extract typed claims from the configured source adapters, synthesize the specification, design, and build plan, and stage the candidate revision in `.emery/storage`:

```bash
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/emery.toml

# Inspect the staged candidate revision (no --config needed with --staged)
cargo run --manifest-path ../emery/Cargo.toml -- show spec --staged
cargo run --manifest-path ../emery/Cargo.toml -- show design --staged
cargo run --manifest-path ../emery/Cargo.toml -- show plan --staged
```

You can pass any example configuration to `--config`:

- [`examples/emery.toml`](emery.toml) — combines TypeScript (`evals/cases/train/fixture`) and documentation (`examples/documentation/docs`) sources with the `emery:omnia` target.
- [`examples/documentation/emery.toml`](documentation/emery.toml) — extracts from Markdown documentation alone.
- [`examples/intent/emery.toml`](intent/emery.toml) — extracts from an inline operator brief (`description`).
- [`examples/typescript/emery.toml`](typescript/emery.toml) — extracts from a TypeScript service.
- [`examples/python/emery.toml`](python/emery.toml) — extracts from a FastAPI Python service.
- [`examples/amend/emery.toml`](amend/emery.toml) — configured with `scope = "whole"` for the amendment walkthrough in [Step 5](#step-5-amend).

> **Note:** Configurations without a `[target]` table stage a candidate revision that you can view with `emery show <artifact> --staged`. Sealing with `emery apply` and viewing the sealed revision with `emery show <artifact>` require a `[target]` table.

---

## Step 3: Apply

[`examples/emery.toml`](emery.toml) configures `emery:omnia` as its `[target]` without a `repository` URL. Seal the staged candidate revision into an engine-managed Git repository at `.emery/vcs/repos/omnia`:

```bash
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/emery.toml

# Inspect the sealed revision from the target repository
cargo run --manifest-path ../emery/Cargo.toml -- show spec --config examples/emery.toml
```

What `emery apply` does:

1. Initializes `.emery/vcs/repos/omnia` on the first run.
2. Commits `spec/spec.json`, `spec/design.json`, and `spec/plan.json` in a single `Revision <revision>` commit.
3. Creates or updates the Git branch `emery/<revision>` at that commit and clears the staging area.

Tip: Pass `--apply` to `emery specify` (`emery specify --apply --config ...`) to stage and seal in a single step.

You can list the sealed revision branches in the target repository at any time:

```bash
git -C .emery/vcs/repos/omnia branch --list 'emery/*'
```

---

## Step 4: Build

Build the sealed plan wave by wave through the target adapter (`emery:omnia`):

```bash
cargo run --manifest-path ../emery/Cargo.toml -- build -v --config examples/emery.toml

# Check out the emery/<revision> branch in the managed repository to inspect the generated code
git -C .emery/vcs/repos/omnia checkout emery/<revision>
```

How `emery build` works with target repositories:

- **Managed repository (no `repository` key):** Builds into `.emery/vcs/repos/<name>` on top of the `Revision <revision>` commit and advances branch `emery/<revision>` as each wave verifies. Your current working checkout remains untouched.
- **Remote clone (`repository = "<url>"`):** Clones or fetches the remote repository into `.emery/vcs/repos/<name>` and builds from `origin/HEAD`.
- **Current checkout (`repository = "."`):** Builds in your project's own Git repository from its clean `HEAD` commit.

During `Task::Init`, target adapters only create layout files that do not already exist, leaving existing project files intact. During `Task::Build` and `Task::Verify`, the `emery:omnia` worker grant permits network access to `index.crates.io` and `static.crates.io` and read-only access to `~/.rustup` and `~/.cargo/bin`.

---

## Step 5: Amend

### Amend

[`examples/amend/emery.toml`](amend/emery.toml) configures `emery:typescript` over `evals/cases/orders/fixture/` (`OrderService` with `place`, `read`, and `cancel` methods under stem `order-service`) with `scope = "whole"`, targeting `emery:omnia`.

When a behavior source (`typescript` or `python`) is configured with `scope = "whole"`, `emery amend` treats that source as exhaustive for every stem its `requirement` claims mention. Any existing requirement under a covered stem that the source no longer states is automatically retired ("retired by scope"). Prose sources (`intent` and `documentation`) never retire requirements by silence; they retire behaviors only when they emit an explicit `retire` claim.

Run the following steps to seal a baseline revision, remove the `cancel` method from `orders.ts`, stage and inspect the resulting amendment, and seal it:

```bash
# 1. Seal the baseline revision from the orders fixture (add --fresh if .emery/vcs/repos/omnia already has a lineage)
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --apply --config examples/amend/emery.toml

# 2. Remove the cancel method (lines 67–75) from OrderService so only place and read remain
python3 -c '
from pathlib import Path
p = Path("evals/cases/orders/fixture/orders.ts")
lines = p.read_text().splitlines(keepends=True)
p.write_text("".join(lines[:66] + lines[75:]))
'

# 3. Stage the amendment; scope = "whole" retires the removed cancel behavior under order-service
cargo run --manifest-path ../emery/Cargo.toml -- amend -v --config examples/amend/emery.toml

# 4. Review the staged amendment and seal it as the next revision
cargo run --manifest-path ../emery/Cargo.toml -- show amendment
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/amend/emery.toml

# 5. Restore the fixture file
git checkout -- evals/cases/orders/fixture/orders.ts
```

What to look for in the output:

- **`emery amend` summary:** Reports `retired (N by scope)` on the counts line and `slices: order-service (brief kept)` on the plan line.
- **`emery show amendment`:** Lists the retired requirements under `## Retired` with ``Note: retired by scope: `typescript` was read whole and states nothing under `order-service`.``, and shows `### Slice: order-service [changed]` under `## Plan` with `Note: the brief is the base's; no slicing turn redrafted it.`.
- **`emery apply`:** Seals the new revision over its base (`amends <parent>`) and records `Parent:`, `Base:`, `Added:`, `Changed:`, `Retired:`, and `Slices:` commit trailers on branch `emery/<revision>`.

---

## Reference

### Adapter cache (`~/.emery/adapters`)

Every adapter is addressed as `emery:<name>@<version>`. Before fetching from the registry, `emery` checks `~/.emery/adapters/emery_<name>@<version>.wasm`:

- Copying a locally built `.wasm` file to `~/.emery/adapters/emery_<name>@0.12.0.wasm` overrides the published release on your machine.
- Deleting that file causes the next `emery` run to download the published artifact from GHCR.
- Using a `-dev` suffix in both `emery.toml` (`emery:<name>@0.12.0-dev`) and the filename (`emery_<name>@0.12.0-dev.wasm`) lets you keep a local build alongside the published release.

### Logging and verbosity

The default log level is `info`. Pass `-v` (`debug`) or `-vv` (`trace`) to increase verbosity, or `-q` / `-qq` to reduce it. You can also filter specific targets with `RUST_LOG`:

```bash
export RUST_LOG="omnia_core=off,emery_sdk=trace"
export CURSOR_SDK_BRIDGE_LOG=1
```

| Verbosity | Validation and correction details logged |
| --- | --- |
| Default (`info`) | Total `attempts` count on the `completion` summary line. |
| `-v` (`debug`) | Individual validation findings (such as an anchor outside the seam's allowed spans or an invalid stem prefix). |
| `-vv` (`trace`) | Full correction prompts sent back to the model and the accepted JSON evidence payload. |

### Progress and environment variables

During long model turns, the runtime logs an `in progress` heartbeat every 15 seconds. When a turn finishes, its `completion` log line reports token usage and elapsed milliseconds across `opening_ms`, `tool_ms`, `model_ms`, and `check_ms`:

- **Single small module:** Runs `evidence`, `spec-draft`, and `design-draft` turns.
- **Multi-module codebases (`typescript` / `python`):** Parses the codebase, runs one `survey-<source>` turn, and runs one `evidence` turn per seam (plus `grouping` and `slicing` when multiple stems exist).
- **`emery build`:** Runs one `build-<slice>` turn per slice (in parallel up to `--jobs` within each wave), followed by one `verify` turn per wave.

| Environment variable | Default | Description |
| --- | --- | --- |
| `CURSOR_TIMEOUT_SECS` | `600` | Maximum wall-clock seconds per model turn (including build and verify turns). |
| `CURSOR_INACTIVITY_SECS` | `120` | Cancels a model stream if no events arrive from the bridge within this window. |
| `CURSOR_MAX_AGENTS` | `4` | Maximum concurrent bridge agents; additional turns queue automatically. |
| `CURSOR_MODEL` | `auto` | Model identifier used for completions (pin a specific model when benchmarking prompts). |

### Reference tools

During extraction, survey, build, and verify turns, the SDK serves `list_docs` and `read_doc` in-process from the adapter's embedded `PROSE` corpus (plus the shared runtime references `claims.md` and `reconciliation.md` on source turns).

---

## Installing cursor-sdk-bridge

```bash
curl -fsSL -o /tmp/cursor-sdk-bridge.tar.gz \
  https://github.com/cursor/sdk-bridge/releases/latest/download/cursor-sdk-bridge-standalone-darwin-arm64.tar.gz \
  && tar -xzf /tmp/cursor-sdk-bridge.tar.gz -C /tmp \
  && install /tmp/bin/cursor-sdk-bridge ~/.local/bin/cursor-sdk-bridge

cursor-sdk-bridge --help
```

See the [Cursor SDK Bridge documentation](https://cursor.com/docs/sdk/bridge) for platform downloads and configuration details.
