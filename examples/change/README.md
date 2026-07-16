# Change example

The end-to-end example over the real WIT seam: the published `specify:core` guest composed with this repo's built adapter components — `documentation` and `intent` on the source axis, `contracts` on the target axis — in one Omnia deployment, driven through the operator rhythm with the live cursor backend. The [`change-example`](host.rs) binary beside this README is the deployment's host runtime: the cursor backend behind `wasi-model` plus the HTTP trigger serving each adapter's MCP reference route.

This is the adapters mirror of the engine's `examples/change`, with real adapters in place of the fixture guest and the published core in place of the workspace build. Operator-invoked demo posture: exit codes plus a final artifact-exists check, not a graded test. The graded native trial is `cargo make eval` (see [TESTING.md](../../TESTING.md)).

## Quick start

Requires an authenticated `cursor-agent` on `PATH` (`cursor-agent login` or `CURSOR_API_KEY`) and authenticated `wkg` credentials for the `specify:` registry.

```bash
cargo make core-fetch   # once per pin: specify:core -> target/core/specify.wasm
cargo make change-run
```

Clean up afterwards:

```bash
cargo make change-clean
```

Artifacts land under the gitignored `sandbox/change/`.

## What it demonstrates

The run drives `init → plan author → plan transition approved → plan execute` against [omnia.toml](omnia.toml):

1. `documentation` surveys the seeded `docs/` tree and `intent` surveys the operator intent; the core reconciles their leads into a plan.
2. The drained loop refines each slice (extract per source, synthesis) and dispatches the `contracts` build, which authors the slice's contract delta under `.specify/slices/<slice>/contracts/`.
3. The merge promotes the delta into the workspace's `contracts/` baseline, gated by the adapter's phased merge validators.

After running, inspect the merged baseline at:

```text
sandbox/change/workspace/contracts/
```

`SPECIFY_EVAL_MODEL=<model-id>` overrides the model for a run, exactly as in the eval trial and the prompt scenarios.

## Pinning

The core component version is `SPECIFY_CORE_VERSION` in the root [Makefile.toml](../../Makefile.toml); it advances together with the engine revision pin in [eval/Cargo.toml](../../eval/Cargo.toml).
