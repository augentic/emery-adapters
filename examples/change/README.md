# Change example

End-to-end run over this repo's `documentation`, `intent`, and `contracts` adapters. 

See `cargo make eval` for the full native test harness (more detail in[TESTING.md](../../TESTING.md)).

The `[change-example](host.rs)` binary is the deployment host (cursor backend + HTTP MCP reference routes).

## Quick start

Requires authenticated `cursor-agent` on `PATH` (`cursor-agent login` or `CURSOR_API_KEY`) and `wkg` credentials for the `specify:` registry.

```bash
make core-fetch
make change-run
make change-clean
```

`change-run` depends on `change-build` (core check, the three composed components, sandbox fixture, host), picks a free loopback port (`HTTP_ADDR=host:port` to override), and takes tens of minutes of live model time. `GUEST_TIMEOUT_MS` defaults to one hour — omnia's 30s default would cut off live judgment legs. Artifacts land under gitignored `sandbox/change/`.

## What it demonstrates

Drives `init → plan author → plan transition approved → plan execute` against [omnia.toml](omnia.toml), replaying the same inputs as the native trial (`[trial.env](trial.env)` + `[fixture/](fixture/)`):

1. Sources survey; core reconciles leads into a plan.
2. The drained loop refines each slice and runs the `contracts` build.
3. Merge promotes the delta into `contracts/`, gated by the adapter's merge validators.

The completion gate (`[change-support](change-support.rs)`) checks the plan is drained, every entry is `done`, and the merged baseline validates. Inspect it at `sandbox/change/workspace/contracts/`.

`SPECIFY_EVAL_MODEL=<model-id>` overrides the model. Core pin: `SPECIFY_CORE_VERSION` in the root [Makefile.toml](../../Makefile.toml) (advances with the engine revision in [Cargo.toml](../../Cargo.toml)).