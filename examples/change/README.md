# Change example

End-to-end run over this repo's `documentation`, `intent`, and `contracts` adapters, composed with a specify workflow guest built from this package.

See `cargo make eval` for the graded native trial (more detail in [TESTING.md](../../TESTING.md)).

## Quick start

Requires authenticated `cursor-agent` on `PATH` (`cursor-agent login` or `CURSOR_API_KEY`).

```bash
cargo make change-run
cargo make change-clean
```

`change-run` builds the specify guest plus the three adapter components (debug), seeds `sandbox/change/` from [fixture/](fixture/), and drives `init → plan author → plan transition approved → plan execute`. Override the MCP loopback with `HTTP_ADDR=127.0.0.1:<port>` if needed. `GUEST_TIMEOUT_MS` defaults to one hour. Artifacts land under gitignored `sandbox/change/`.

## Layout

Declared on the workspace root package (`adapters`) as Cargo examples:

| Path | Role |
| --- | --- |
| `specify.rs` + `provider.rs` | `--example specify` workflow guest (`cdylib`) |
| `omnia.rs` | `--example change` Omnia host |
| [omnia.toml](omnia.toml) | deployment: guest + adapters + mounts |
| [fixture/](fixture/) | seed inputs shared with the native trial |

The engine's root `specify` package is `cdylib`-only, so the guest sources live here and depend on the engine crates from git.

## What it demonstrates

1. Sources survey; core reconciles leads into a plan.
2. The drained loop refines each slice and runs the `contracts` build.
3. Merge promotes the delta into `contracts/`, gated by the adapter's merge validators.

Inspect the result at `sandbox/change/workspace/contracts/`. `SPECIFY_EVAL_MODEL=<model-id>` overrides the model.
