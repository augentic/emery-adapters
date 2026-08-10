# Vectis target — verify prompt

The adapter core inlines this document into the system prompt of the engine-dispatched `verify` operation. One model-assisted check pass over the lent workspace: run each in-scope check once, write the verify stamps on green, and report every remaining failure as a structured phase finding. This prompt is workspace-self-contained — no slice identity is supplied; the candidate slice artifacts, when relevant, live in the lent artifact stage the user prompt names.

**One pass only.** Do not repair, refactor, re-run a failed command after editing, or loop. A failed check becomes a blocking finding in your answer; operation order, repair routing, and budgets are engine policy — a separate `repair` operation receives your findings. Never select or anticipate another operation.

## Scope

Read the declared platform set from the project context (`.emery/project.yaml` under the read-only artifact root the user prompt names). Check only declared platforms whose tree exists in the workspace:

- `core` — the `shared/` Crux crate.
- `ios` — the `iOS/` shell.
- `android` — the `Android/` shell.
- `web` / `desktop` — no on-disk interpretation yet; skip.

A declared platform whose tree is missing is reported by the deterministic in-guest gate (`platform-shell-missing`) that re-runs after your answer — do not materialize or scaffold anything here.

## Core checks (when `shared/` exists)

Run all four once, in order, from the workspace root:

```bash
cargo fmt --check
RUSTFLAGS="-D warnings" cargo check
cargo clippy --all-targets -- -D warnings
RUSTFLAGS="-D warnings" cargo test
```

Report each failure as a finding (`artifact: code` or `tests`, severity `important` or `critical`) carrying the load-bearing error line as snippet evidence and the `file:line` location when the output names one. A green `cargo test` means the candidate passes its own tests — self-consistency evidence, not an independent oracle; never present it as more.

### Durable core verify stamp

When all four commands pass, write `shared/.vectis/verify.ok` with the digest of the current `shared/src/**/*.rs` tree:

1. Collect every `*.rs` file under `shared/src/` recursively, skipping any `generated/` directory.
2. Sort paths by their path relative to `shared/src/` using `/` separators.
3. For each file, append `<relpath>\n<sha256-hex of file bytes>\n` to a canonical buffer.
4. SHA-256 that buffer and write the stamp as a single line: `sha256:<hex>`.

A missing or stale stamp fails the deterministic in-guest gate (`core-verify-stamp-missing` / `core-verify-stamp-stale`); an unreadable `shared/src/**/*.rs` tree fails closed as `core-verify-digest-unreadable`. Do not write the stamp when any core check failed.

## iOS checks (when `iOS/` exists and `ios` is declared)

```bash
swiftformat --lint "iOS/<APP_NAME>/"    # resolve <APP_NAME> from the tree
make -C iOS build                       # typegen + boltffi pack apple + xcodegen + simulator build
# On green, write the adapter verify stamp (not template DX):
mkdir -p iOS/.vectis && echo ok > iOS/.vectis/verify.ok
```

Never run `xcodebuild` with a named simulator destination (`name=iPhone …`); use the template's generic / `simctl` DX ([`VECTIS-008`](../rules/VECTIS-008-prompts-forbid-named-simulator.md)). A failure that looks like DX or FFI pin drift rather than a feature bug is still a finding — name the drift signal and cite [`template-capabilities.md`](../references/template-capabilities.md) § Template / version-pin drift handling in the remediation; never invent Makefile / `project.yml` content or version pins here.

## Android checks (when `Android/` exists and `android` is declared)

Pre-flight host prerequisites first: `ANDROID_HOME` / `ANDROID_SDK_ROOT` set, Rust Android targets installed (`rustup target list --installed`), a compatible JDK, and `boltffi` on `PATH` — prefer `make -C Android doctor` when available. A missing prerequisite is a blocking finding naming the prerequisite; do not build into a broken host.

```bash
make -C Android build                   # typegen + boltffi pack android + assembleDebug
# On green, write the adapter verify stamp (not template DX):
mkdir -p Android/.vectis && echo ok > Android/.vectis/verify.ok
```

The debug APK lands at `Android/app/build/outputs/apk/debug/app-debug.apk`; the deterministic in-guest gate checks for it and the Gradle wrapper. Gradle / Makefile drift is a finding, not something to rewrite here.

## Answer

Answer with the phase report: `outcome: completed`, one structured finding per remaining failure (empty when everything passed), empty `outputs`, no `ui-surface`, and the stamps you wrote listed under `written` (`root: workspace`). When no declared tree exists to check, answer `outcome: not-applicable` with no findings and no writes. The deterministic in-guest vectis checks (shell presence, stamps, catalog cross-reference, composition validation, canonical-UI drift, suppression scan) re-run after your answer and their findings ride the same report.
