# Harness review

Review of the refactored change example and native evaluation harness against their counterparts in `augentic/specify`.

## Conclusions

- Keep `eval/` as the single native harness. Contracts and Vectis need scenario data, not separate harness crates.
- Keep `composed/` and `examples/change/`: native execution cannot prove WIT dispatch, component composition, or mount behavior.
- A scenario can select any adapter already linked into `eval`. Supporting an arbitrary third-party adapter still requires a Cargo dependency and a `catalog.rs` entry; configuration alone cannot dynamically link a Rust crate.
- The adapters eval is intentionally broader than Specify's namesake: it adds the native CLI, HTTP/MCP serving, real linked adapters, and prompt scenarios. The underlying `native.rs` model bridge is currently in parity.

## Recommended work

### 1. Isolate native trial state

Add the scoped project-cache behavior used by Specify's eval to every phase in `eval/src/trial.rs`. The trial should not read or write the operator's normal cache, and its result should not depend on prior local state.

### 2. Finish and harden scenario expectations

Keep the new `expect` artifact gate, with these adjustments:

- Require at least one expectation for `build` scenarios rather than defaulting to an empty list.
- Reject absolute paths, `..`, and paths that resolve outside the scratch root.
- Avoid recursive traversal through symlink cycles.
- Use collision-proof run directories; second-resolution timestamps can reuse an earlier run and accept stale artifacts.
- Persist `outcome: pass` only after the report and artifact expectations both pass.
- Reuse one config parser/validator in the runner and wiring tests instead of maintaining duplicate `Config` shapes.

### 3. Do not retry mutation-capable model calls in the example host

The narration retry in `examples/change/host.rs` can replay a completion after the first attempt has already changed the workspace. It also bypasses judgment repair accounting and makes native and WASM runs behave differently.

Fix terminal-result parsing in the Cursor backend instead. Until that is available, prefer a visible failure over silently replaying a potentially non-idempotent completion.

### 4. Align the native and WASM scenarios

Centralize the values shared by `eval/src/trial.rs` and `Makefile.toml`:

- change name;
- operator intent;
- source bindings;
- seeded `orders-api.md`.

This ensures differences between `cargo make eval` and `cargo make change-run` come from the execution boundary rather than different inputs.

Correct the Vectis scenario fixture to use the current project key:

```yaml
specify: 0.27.2
```

The existing `specify_version: '2.0'` key is unknown to `ProjectConfig` and is silently ignored.

### 5. Strengthen the change-example completion gate

The current gate only proves that one YAML file exists. At minimum, verify:

- plan execution drained;
- every entry is `done`;
- the merged contracts baseline passes the contracts validator.

The example need not duplicate the full native grader.

### 6. Make the quick-start path genuinely quick

- Build only `documentation`, `intent`, and `contracts` for `change-run`, not every adapter.
- Avoid a fixed MCP port where practical.
- Keep the core-fetch prerequisite explicit, with a direct authentication hint.
- State that runs take minutes and document `GUEST_TIMEOUT_MS` and `SPECIFY_EVAL_MODEL`.
- Correct documentation that implies adding a third-party scenario directory is sufficient without linking its crate.

### 7. Reduce cross-repository duplication later

Consider an engine-owned eval-support crate for:

- the native model bridge;
- telemetry;
- lazy Cursor connection;
- sandbox/cache helpers;
- common seam projections where practical.

Do not generalize the full trial to Vectis without a concrete cross-phase need. Contracts has a natural full-change validator; Vectis's platform setup, cost, outputs, and grading are different, while its native prompt scenario already provides the fast iteration rung.

## Suggested order

1. Cache isolation.
2. Scenario expectation correctness.
3. Remove or relocate host retries.
4. Shared scenario inputs.
5. Stronger WASM completion gate.
6. Onboarding and command cleanup.
7. Shared eval infrastructure.

