# target.build

Build one slice of the plan into the lent project tree as an omnia HTTP guest in Rust, check it lints, its tests pass, and it compiles for `wasm32-wasip2`, and report what you covered and wrote.

## Inputs

- `$WORKSPACE` — the project tree, lent writable, sitting on the commit the brief names. Read it through the workspace tools; write it through this call's `write_files` tool alone, one or more files per call, each created or replaced whole, at a `/`-separated path relative to `$WORKSPACE`, and remove a file through the same call's `delete`. Lay the module, its lines, its test, and any manifest in one call; a fix the loop below asks for is a later call rewriting the file whole. The shell runs the checks below in `$WORKSPACE` and nothing else: a file the shell writes, through a redirect, a heredoc, `cp`, or a script, is outside the tool and is not yours. Nothing outside `$WORKSPACE` is reachable.
- **The slice's plan entry** — its id, name, requirements, the design types it owns, and the slices it depends on.
- **The specification, cut to the slice** — the requirements to implement, each with its acceptance scenarios.
- **The design, whole** — the types and sections every slice shares.

Build this slice completely in one pass. Other slices of the wave are building beside you from the same commit; what you write merges with theirs under the rules the [layout](references/layout.md) states, so keep to its shape.

## What to write

The [layout](references/layout.md) is the one shape every slice follows. In brief:

- **Your crate.** `<stem>` is the first segment of your requirement ids (`orders` for `orders.create`). A slice named by its stem builds it whole as `crates/<stem>/src/lib.rs`; any other slice builds a part of each stem its requirements lead with, as `crates/<stem>/src/<part>.rs`, `<part>` your slice's name less the stem and its hyphen where the name leads with them, in snake_case (`reading` for `orders-reading`). Everything your requirements need lives in that module: `mount`, the handlers, and the design types you own, `pub` where another module may use them.
- **Your lines.** One line in the router list of `src/lib.rs`, `let router = app_<stem>::mount(router);` for a stem built whole, `let router = app_<stem>::<part>::mount(router);` for a part; one line in the root manifest's `[dependencies]`, `app-<stem> = { path = "crates/<stem>" }`, the same line whichever slice writes it. A part adds one more in the stem's list, `crates/<stem>/src/lib.rs`: `pub mod <part>;`. A crate your files use that the stem's manifest does not name is one line at the end of its `[dependencies]`. Add your lines at the end of each list and change no other; where a list is missing, write the file holding your line alone.
- **Your wiring.** Your module exports `mount`, which takes the router and returns it with your routes merged in, bounding `P` on the capabilities your handlers use; the root's `router` calls every `mount` its list names. Never edit `crates/start/`, or another stem's crate, to be reached.
- **Your handlers**, as the [handlers](references/handlers.md) reference spells them: one typed route per requirement's surface, `async fn(input, context) -> omnia_sdk::Result<output>`, the design's types as `serde` structs, the host reached through `context.provider()` alone, and every failure a classified error. What a guest cannot do, and the capability it reaches instead, is in the [guardrails](references/guardrails.md).
- **Your test file**, `crates/<stem>/tests/<stem>.rs`, or `crates/<stem>/tests/<part>.rs` for a part: one `#[tokio::test]` per acceptance scenario, named by the scenario in snake_case, mounting your module alone on a fresh router over the doubles of `omnia_test::guest` and driving it in process with `oneshot`, as [testing](references/testing.md) spells it. A scenario the specification gives you is a test; invent none. A service the tree does not run is a double handed to the provider, never a live connection.
- **The root files**, `Cargo.toml`, `src/lib.rs`, `tests/routes.rs`, `examples/runtime.rs`, `rust-toolchain.toml`, and `.gitignore`, exactly as the layout spells them, when `$WORKSPACE` holds none, your lines in the two lists; a slice that finds them in the tree adds its lines and leaves the rest as it is. The root manifest's `[workspace.dependencies]` pins every shared crate and nothing is ever added to it by any slice.
- **Your stem's manifest**, `crates/<stem>/Cargo.toml`, as the layout spells it with your lines in its `[dependencies]`, when the tree holds none; your lines alone otherwise. A pinned crate is `<crate>.workspace = true`; one the pins lack carries its version; another stem is `app-<other> = { path = "../<other>" }`.
- **The `start` stem**, where your slice holds it: the root files are the program's start, and a `start` requirement they and the host satisfy — the component answers HTTP, the router mounts every module, the settings come from the host — is yours to cover once they are laid; what is code, a health route or a layer on your own routes, goes in `crates/start/` as in any stem. A `start` requirement nothing in the tree satisfies is left uncovered.
- **Another slice's module is not yours to change.** Depend on its crate where the design has yours depend on it, at the path the layout gives; where it is not in the tree yet, implement against the design's types in your own module and leave the dependency for the slice that lands it.

Read `crates/` and the two root lists before writing: a slice built before this one, or a resumed build, may already hold a crate or a line.

## The loop

Once the tree holds your files, run each check in `$WORKSPACE` and read what it prints, in this order:

1. `cargo clippy -p app-<stem> --all-targets -- -D warnings` — your crate and its tests, under every default lint. The first run resolves the workspace, which writes `Cargo.lock`, which stays, and `target/`, which `.gitignore` keeps out of the tree. A failure in a crate not yours is a type the design names and your module misuses, or a crate that is not there yet, which you work around in yours.
2. `cargo test -p app-<stem>` — your crate's tests; they fail only for what your module does not yet do.
3. `cargo check --target wasm32-wasip2` — the root crate for the guest, so your lines resolve and your module compiles for the target; a crate that reaches past the component fails here, and the module that named it is the one to change.

Fix what a check reports, through `write_files`, and run it again until all three pass. Report only once they pass, or once a check fails for a reason outside your slice, which the report then leaves uncovered.

## Report

Answer with one JSON object:

- `covered` — each requirement id the specification above holds that your module now implements and its test confirms, once each. Leave an id out rather than claim what the tree does not hold or a test does not pass.
- `written` — the files `write_files` wrote, once each, as a `/`-separated path relative to `$WORKSPACE`. A file a check wrote, `Cargo.lock`, is the tree's whether or not you list it.

## Worked example

A slice `orders-reading` (`SLICE-006`) implementing `REQ-030` and `REQ-031`, a part of the `orders` stem another slice also builds this wave, over a tree an earlier wave has already laid the root files in, its module using `chrono` for the order's timestamp; `REQ-031` depends on a `customers` module another slice is building this wave, so its test cannot pass yet and it is left uncovered:

```json
{
  "covered": ["REQ-030"],
  "written": [
    "crates/orders/Cargo.toml",
    "crates/orders/src/lib.rs",
    "crates/orders/src/reading.rs",
    "crates/orders/tests/reading.rs",
    "Cargo.toml",
    "src/lib.rs"
  ]
}
```
