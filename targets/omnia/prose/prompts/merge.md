# Omnia target — merge prompt

> The omnia adapter core inlines this document into the system prompt of the merge leg (`src/operations.rs`). The engine dispatches the adapter's merge operation twice around its deterministic core merge — `preflight` before the engine folds the slice's spec deltas into the baseline, `postflight` after the commit and archive. Only the preflight gate carries Omnia judgment: the build already wrote the slice's code in place in the lent workspace, so the preflight leg runs the § Omnia pre-merge gate over it and answers with the merge report. Omnia declares no merged-baseline validator, so the adapter answers postflight deterministically without a judgment leg. The engine owns spec-delta folding, baseline coherence, the lifecycle transition to `merged`, and the archive move; never perform any of those from this prompt.

## Inputs and bindings

```text
$SLICE_NAME     = slice name from the leg's user prompt
$CRATE_NAME     = $SLICE_NAME with kebab → snake (or the slice's plan-level `crate:` override)
$CRATE_PATH     = crates/$CRATE_NAME
$WORKSPACE_ROOT = repo root (Cargo workspace + guest package in root `Cargo.toml`; guest sources under `src/`)
```

The slice's built code is already present in the lent workspace — the build phase wrote it in place. There is no delta to apply; the gate verifies the workspace as it stands.

## § Omnia pre-merge gate

Run these from `$WORKSPACE_ROOT` (or `$CRATE_PATH` where noted). All four MUST pass. Any failure means the merge report is `status: failure` (see `## Merge report`).

### 1. Format and lint

```bash
cd $CRATE_PATH && cargo fmt --check
cd $CRATE_PATH && cargo clippy --all-targets -- -D warnings
```

Formatting failures are repaired with `cargo fmt` and the gate re-run. Clippy failures are build regressions — report them as blocking findings and fail the merge.

### 2. Workspace check

```bash
cargo check --workspace
```

Catches missing workspace members, broken `Cargo.toml` paths, and provider-trait mismatches that the slice's standalone build did not surface. A failure here typically means the slice introduced or renamed a crate that the workspace root has not been updated to include.

### 3. Test suite

```bash
cd $CRATE_PATH && cargo test
```

The build's verify-repair loop already enforces a passing test suite. Re-running here catches drift caused by sibling slices landing between the build and the merge attempt. A regression is a blocking finding naming the failing tests.

### 4. WASM target build

```bash
cargo build --target wasm32-wasip2 --release --workspace
```

The wasm32-wasip2 build is the definitive deployment-target check. A native `cargo check` will accept code that uses forbidden std APIs or non-WASM-compatible crates; only the wasm32 build proves the slice compiles for the real target. A failure here is a guardrail violation that the build missed. Reference [`../references/guardrails.md`](../references/guardrails.md) for the forbidden crate / API table.

## Merge report

Answer the leg with a schema-valid merge report (the schema-gated report answer — no report file is written). A `status: success` report means all four gate steps passed. Any gate failure means `status: failure`, with the failing step's output mapped into blocking `findings[]` (the report answer schema's diagnostic shape); the engine then aborts the merge with the slice still at `built` for human review — never transition the lifecycle yourself. Omnia adds no postflight validator or adapter-specific adoption mechanics — every artefact under `specs/` is promoted by the engine's deterministic merge, and there are no generated outputs to refresh at merge time.

## References

- [`../references/guardrails.md`](../references/guardrails.md) — Forbidden crates / std APIs the wasm32 build proves are absent.
- [`../references/runtime.md`](../references/runtime.md) — Identity OAuth env vars + `omnia::runtime!` host enumeration the workspace check exercises.
