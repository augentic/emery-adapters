# Wasm example

End-to-end run over this repo's `documentation`, `intent`, and `contracts` adapters, composed with a specify engine guest built from this package.

See `cargo make eval` for the graded native trial (more detail in [TESTING.md](../../TESTING.md)).

## Quick start

Requires authenticated `cursor-agent` on `PATH` (`cursor-agent login` or `CURSOR_API_KEY`).

```bash
cargo make wasm-run
cargo make wasm-clean
```

`wasm-run` builds the specify guest plus the three adapter components (debug), seeds `sandbox/wasm/` from [fixture/](fixture/), and drives `init → plan author → plan transition approved → plan execute`. Override the MCP loopback with `HTTP_ADDR=127.0.0.1:<port>` if needed. `GUEST_TIMEOUT_MS` defaults to one hour. Artifacts land under gitignored `sandbox/wasm/`.

## Layout

Declared on the workspace root package (`adapters`) as Cargo examples:

| Path | Role |
| --- | --- |
| `specify.rs` | `--example specify` engine guest (`cdylib`) — one `guest::export!()` over the engine's `guest` crate |
| `omnia.rs` | `--example wasm` Omnia host |
| [omnia.toml](omnia.toml) | deployment: guest + adapters + mounts |
| [fixture/](fixture/) | seed inputs shared with the graded `eval` trial |

The engine guest is byte-for-byte the engine's: the `guest` crate (a git dependency on `augentic/specify`) owns the WIT bindings, provider, and transport wiring, and both this example and the engine's root cdylib are the same single macro invocation.

## What it demonstrates

1. Sources survey; core reconciles leads into a plan.
2. The drained loop refines each slice and runs the `contracts` build.
3. Merge promotes the delta into `contracts/`, gated by the adapter's merge validators.

Inspect the result at `sandbox/wasm/workspace/contracts/`. `SPECIFY_EVAL_MODEL=<model-id>` overrides the model.
