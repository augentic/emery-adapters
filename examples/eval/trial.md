# Full trial

End-to-end live eval: the operator rhythm over a persistent gitignored `sandbox/` project, with real source bindings and a real target. Production verbs run through the native catalog; the cursor backend sits at the model seam. Deterministic grading only.

Day-to-day hub: [repo README](../../README.md). Scenario loop (one operation): [scenarios.md](scenarios.md).

## Stock trial (contracts)

The `cargo make eval` task pins a contracts-shaped change against the shared orders fixture:

```bash
cargo make eval           # init → plan → execute → finalize → clean
cargo make eval init      # one phase: init | plan | execute | finalize | clean
```

Defaults (from [`examples/Makefile.toml`](../Makefile.toml)):

| Flag | Value |
| ---- | ----- |
| `--fixture` | `examples/wasm/fixture` (docs tree for the orders service) |
| `--target` | `contracts` |
| `--change` | `orders` |
| `--source` | `docs=documentation:docs` |
| `--intent` | Author API contracts for the orders service (contracts only) |

Expect tens of minutes of live model time. A **full** passing run removes `sandbox/`; a failing phase retains it for review or per-phase re-runs.

### Rhythm

```text
init        specify init <target>  (+ copy --fixture into sandbox/)
plan        specify plan author → Gate 1 approved
execute     specify plan execute   (refine → build → merge per slice, until drained)
finalize    specify plan archive
```

`execute` is the real drained loop, not a hand-driven breakout sequence.

### Grading

Hard assertions only (shared `probe` runner):

| Stage   | Check      | Pass condition                                               |
| ------- | ---------- | ------------------------------------------------------------ |
| plan    | Entries    | `plan author` produces at least one entry                    |
| execute | Lifecycle  | Every plan entry is `done`                                   |
| execute | Provenance | Every evidenced requirement carries sources; ids are present |

Per-leg request / repair counts are **reported, not asserted**. A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer. In per-phase mode, counts cover only that phase's requests.

Grading does **not** assert target-specific quality (contract YAML shape, generated Rust compiling outside the adapter's own verify-repair, etc.). Inspect the retained `sandbox/` tree for that.

## Custom trials

Use a custom trial when the stock contracts defaults are wrong — different **target**, **fixture**, **sources**, or **intent**.

### Prefer a full argv

`cargo make eval -- …` appends your flags after the makefile defaults. That is fine for replacing `--target` / `--intent` / `--change` / `--fixture` (clap last-wins on those). It is **not** safe for `--source`: sources are a `Vec` and **append**, so you would keep the stock `docs=documentation:docs` binding alongside yours.

For anything non-contracts, invoke the binary with a complete argument list:

```bash
export EVAL_TIMEOUT_SECS="${EVAL_TIMEOUT_SECS:-600}"

cargo run -p eval -- eval \
  --fixture <path-to-fixture-tree> \
  --target <adapter> \
  --change <change-name> \
  --intent "<operator intent>" \
  --source "<key>=<adapter>:<path>" \
  # optional phase: init | plan | execute | finalize | clean
```

Repeat `--source` for multiple bindings. Source grammar is the locked plan-author form: `<key>=<adapter>:<path>` (or `value:<literal>` for intent-like bindings). Paths are relative to the trial project root after the fixture is copied in.

### Fixture shape

`--fixture` is optional. When set, its tree is copied into a fresh `sandbox/` on `init`. Put everything the sources need there — docs, vendored legacy code, design notes — at the paths your `--source` bindings name.

Example layout for a TypeScript → Omnia migration:

```text
my-fixture/
  legacy/
    at_r9k_position_adapter/    # vendored legacy service tree
      package.json
      src/
      …
```

Then bind `--source legacy=typescript:legacy/at_r9k_position_adapter`.

### Phases

Same as the stock trial. After a failed `execute`, re-run only that phase against the retained sandbox:

```bash
cargo run -p eval -- eval \
  --fixture … --target … --change … --intent … --source … \
  execute
```

(`init` always replaces `sandbox/`; do not re-init if you mean to resume.)

## Example: Omnia legacy migration (test-spec / r9k shape)

The consumer project [`augentic/test-spec`](https://github.com/augentic/test-spec) already did this once: vendor the Propellerhead `at_r9k_position_adapter` TypeScript service under `legacy/`, bind it as a `typescript` source, target `omnia`, and run the full plan → execute loop until an Omnia crate lands under `crates/`. Replicate that here as a **custom trial** (there is no stock `cargo make eval` recipe for omnia yet).

### 1. Build a fixture

Copy or clone the legacy tree into a fixture directory the trial can copy verbatim. Using the tree already vendored in `test-spec` is the shortest path:

```bash
# from specify-adapters/
mkdir -p /tmp/omnia-r9k-fixture/legacy
cp -R ../test-spec/legacy/at_r9k_position_adapter \
  /tmp/omnia-r9k-fixture/legacy/
```

(Or clone from Propellerhead's BitBucket into that same `legacy/at_r9k_position_adapter/` path.)

### 2. Run the trial

```bash
export EVAL_TIMEOUT_SECS="${EVAL_TIMEOUT_SECS:-900}"

cargo run -p eval -- eval \
  --fixture /tmp/omnia-r9k-fixture \
  --target omnia \
  --change at-r9k-position-adapter \
  --intent "Migrate the legacy TypeScript at_r9k_position_adapter under legacy/ into a new Omnia WASM crate with provider-based dependency injection. Survey and extract behaviour from the TypeScript tree; build the guest, crate, and tests; merge specs into the baseline." \
  --source "legacy=typescript:legacy/at_r9k_position_adapter"
```

Or phase it:

```bash
# …same flags… init
# …same flags… plan
# …same flags… execute
# …same flags… finalize
```

### 3. Inspect outputs

On success the trial archives and (on a full unphased run) cleans `sandbox/`. On failure — or if you stop after `execute` — inspect:

```text
sandbox/
  plan.yaml / change.md / discovery.md
  .specify/slices/<slice>/     # proposal, specs, design, tasks, evidence
  crates/…                     # Omnia crate(s) the build wrote
  src/lib.rs                   # guest wiring (create mode)
  report-adjacent artifacts    # e.g. REVIEW.md under the crate
```

Pass/fail from grading is lifecycle + provenance. For migration quality, treat the generated crate, guest, tests, and `REVIEW.md` as the real signal — the same bar you would use in `test-spec`.

### 4. When to use a scenario instead

If you are editing omnia `prose/` and only need to know whether **build** still produces a crate for a known slice, use [`omnia/health`](scenarios/omnia/health/) (or add a richer scenario). Do not burn a full r9k trial for prompt typos.

## Manual native verbs

For maximum control (skip the trial driver, keep a project across sessions), drive the catalog yourself:

```bash
cargo make specify -- --project-dir /path/to/project init omnia --name <name>
cargo make specify -- --project-dir /path/to/project plan author <change> \
  --intent "…" \
  --source "legacy=typescript:legacy/at_r9k_position_adapter"
cargo make specify -- --project-dir /path/to/project plan transition <change> approved
cargo make specify -- --project-dir /path/to/project plan execute
```

This is the same native seam as the trial; you own lifecycle and grading.

## See also

- [repo README](../../README.md) — run → debug → repair loop
- [scenarios.md](scenarios.md) — single-operation prompt scenarios
- [docs/testing.md](../../docs/testing.md) — five-rung map and test-layer policy
- Engine tutorial [Legacy migration at scale](https://github.com/augentic/specify/blob/main/docs/tutorials/legacy-migration-at-scale.md) — operator-facing migration orientation
