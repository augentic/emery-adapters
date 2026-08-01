# Template materialize (greenfield)

This is **template materialize** — the allowlisted copy procedure below, performed by the build agent by hand — not asset materialize (the in-guest prepare prelude that exports design-system assets). The build prompt's template-materialize prelude names which declared trees are absent; this reference carries the full procedure. Runs on the **host** filesystem — the guest cannot see a sibling checkout, and there is no CLI for this step.

## Procedure

1. **Resolve `$TEMPLATE_DIR`.** Default `${PROJECT_DIR}/../vectis-exemplar`, else `VECTIS_EXEMPLAR_DIR`. If the directory is missing or is not a `vectis-exemplar` checkout, **stop** (`deferred`) — clone https://github.com/augentic/vectis-exemplar.git; never invent scaffold files or pins.
2. **Mechanical allowlisted copy** into `${PROJECT_DIR}` with identity substitution (`APP_NAME`, `ANDROID_PACKAGE`). Copy root DX (`Makefile`, `Makefile.toml`, `Cargo.toml`, `Cargo.lock` when present, `rust-toolchain.toml`, `deny.toml`, `README.md`, `.gitignore`), plus `shared/`, `iOS/`, `Android/` (including the Gradle wrapper), `supply-chain/`, and `.maestro/`. **Never** copy `.git/`, `.github/`, `web/`, or `AGENTS.md`. Skip machine junk (`target/`, `.gradle/`, `*.xcodeproj/`, `local.properties`, …) — the denylist. One materialize stands up the whole workspace — do not invent per-shell scaffolds. Materialize refuses to overwrite any existing root DX file **except** `.gitignore`: an `emery init` stub is replaced with the template file (then Emery lines `.emery/scratch/` and `workspace/` are re-asserted). Do not hand-merge `.gitignore` around materialize.
3. **Strip `VECTIS-OPTIONAL`.** Follow **`$TEMPLATE_DIR/AGENTS.md`** (not a consumer copy) against the `design.md` `## Adapters` capability matrix: remove unused `cap=http|kv|time|sse` units and always strip `cap=demo` for product apps. Do not invent FFI shapes or dependency versions while stripping. Keep Maestro infra (`.maestro/config.yaml`, `.maestro/test-ids.yaml`, `.maestro/scripts/load-test-ids.sh`) and root / shell DX after strip — see [`template-capabilities.md`](template-capabilities.md).
4. **iOS project generation.** After materialize, run `make -C iOS generate-project` (or `xcodegen`) — committed `.xcodeproj` trees are denylisted on purpose.
5. **Then** run the existing generate/update logic in the core / shell write prompts.

## Late capability adoption (update mode)

When a later slice's `## Adapters` turns a previously stripped cap on, copy that `cap=` strip-unit from `$TEMPLATE_DIR` per [`template-capabilities.md`](template-capabilities.md) — do not invent versions or handler shapes. Strip grammar remains `$TEMPLATE_DIR/AGENTS.md`.
