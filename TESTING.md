# Testing

Integration-first test posture for `specify-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the composed-deployment tests — never against private kernels. Read this before adding or deleting a test.

## The development loop

Five rungs, fastest feedback first. Every behavior is asserted on exactly one rung — duplicating an assertion across rungs is a defect, not extra safety. In particular: prompt-assembly checks belong on rung 1, seam and wiring checks on rung 2, and only prose-effectiveness judgments on rung 3.

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through the standard auto-discovered suite at `{targets,sources}/<name>/tests/<area>.rs`. Judgment-leg prompts are asserted here against the mock `Model` from `crates/testkit` — "did my prompt edit land in the assembled text". The wasm32-only guest shims (`src/guest.rs`) are hand-written export glue and carry no native tests.

```bash
cargo nextest run -p vectis   # one adapter
cargo make test               # the whole workspace, matching CI
```

`nextest` is mandatory, not a preference: it runs each test in its own process, and that isolation is what lets the CWD/env-mutating suites pass. Never use bare `cargo test`.

### 2. Composed-deployment tests — model-free component checks

`evals/composed.rs` (the `composed` test target of the `evals` package) hosts the built adapter components on the Omnia runtime and exercises the deterministic seams: `describe` / `guidance` through host-mediated dispatch, the judgment legs against a stub model backend (which must come back as the WIT error variant, not a trap), and each guest's MCP references over `wasi:http`. Guests build from source on first use when artifacts are absent under `target/wasm32-wasip2/debug/`.

```bash
cargo test -p evals --test composed
```

### 3. Live evals — the only rung that judges prose effect

The `#[ignore]`d tests in `evals/live.rs` drive one adapter operation end-to-end against the real cursor backend. They require [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`; `SPECIFY_EVAL_MODEL=<model-id>` overrides the model. Raw output lands under `evals/<adapter>/runs/`, and a failing report `status` fails the test.

```bash
cargo make eval-contracts   # every contracts scenario
cargo make eval-vectis
cargo test -p evals --test live -- --ignored --nocapture contracts::describe   # one scenario
cargo test -p evals --test live wiring   # the model-free smokes (not ignored; CI runs them)
```

Scenario anatomy and seeds are documented beside the scenarios: [`evals/contracts/README.md`](evals/contracts/README.md), [`evals/vectis/README.md`](evals/vectis/README.md).

### 4. Prose overlay — iterate on prompts without rebuilding

`SPECIFY_PROSE_OVERLAY=1` switches a live run into overlay mode: the harness seeds the adapter's `prose/` tree into the scratch `.eval/prose/`, forwards the grant to the guest (whose registry probes the overlay at runtime), and skips the cargo legs entirely once the run artifacts exist. Edit `{targets,sources}/<name>/prose/**` and re-run — one model leg per save, no build. The overlay overrides document bodies only (the doc set stays the embedded table's), and the guest prints an attestation to stderr so an overlaid run can never pass as an embedded run.

```bash
SPECIFY_PROSE_OVERLAY=1 cargo test -p evals --test live -- --ignored --nocapture contracts::design
```

### 5. Consumer project — code changes through the engine

For code (not prose) iteration against a real consumer project, `cargo make dev [adapter]` builds components with fast profile settings (LTO off, opt-level 1) into `target/wasm32-wasip2/release/<name>.wasm` — the exact path the engine's bare-name resolution probes from a sibling checkout — in seconds instead of a full `cargo make release`. Caveat: switching between the `dev` and `release` flavors changes the profile fingerprint and forces a rebuild; publishing is rare, so the trade is accepted.

```bash
cargo make dev contracts   # one adapter; bare `cargo make dev` builds the workspace
```

## The two layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test. The standing bias is **fewer unit tests**.

| Layer | Location | Required when | Forbidden when |
| --- | --- | --- | --- |
| **Kernel unit** | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code | The branch is genuinely unreachable through the public API (a defensive guard, an error variant no caller triggers), **or** the behavior is a dense pure parse/projection/render-math matrix whose case-per-cell integration port would inflate the suite | The behavior is reachable through the crate's public surface and an integration test already covers it — or could, without a matrix explosion |
| **Crate integration** | `<adapter>/tests/` (one auto-discovered binary per area) | The behavior is reachable through the library API: engine invariants, filesystem-shape corners, render output, judgment-leg prompts against a mock `Model` | The same observable behavior is already asserted elsewhere and needs no coverage backfill |

## Triage rubric

Applied to every existing `#[cfg(test)]` / `tests.rs`:

- **Delete** — the observable behavior is already asserted by an integration test, or it is tautological, mock-heavy, or an internal snapshot that gives an agent no boundary signal.
- **Collapse (stay unit)** — a dense pure `(input → output/code)` matrix (e.g. `app_icon/canvas` render math, `svg`, `materialize/paths`) becomes one table-driven `#[test]` with a block per case. Coverage-neutral by construction.
- **Re-home** — behavior reachable through the library lands in the crate's `tests/` tree.
- **Keep** — a genuinely unreachable defensive branch or error variant no caller can trigger, with a one-line comment saying why an agent cannot get the same signal from integration.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The [`Makefile.toml`](Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

```bash
cargo llvm-cov nextest -p vectis --summary-only
cargo llvm-cov nextest -p contracts --summary-only
```

A `TOTAL` line/region drop on still-live code means real coverage was lost: backfill with an integration assertion (preferred) or revert that specific deletion. A pure collapse of redundant cases is coverage-neutral.

**Watch for non-deterministic coverage.** Code that walks `std::fs::read_dir(...).flatten()` (e.g. `shell/launcher` icon detection) hits a different branch set depending on directory iteration order, so the `TOTAL` can wobble a line or two run-to-run on identical source. When a deletion appears to drop 1–2 lines, re-run the *baseline* too before chasing it — and if results look stale, force a full instrumented rebuild (`rm -rf target/llvm-cov-target`) rather than trusting `cargo llvm-cov clean` alone.

## Test naming

Test function names are identifiers, not sentences. The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. Push the narrative into the test body or a `//` comment above the `fn`.

## Definition of done for a reduction

- Source-side `#[test]` count is materially reduced, and every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `cargo make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
