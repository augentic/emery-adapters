# A change build

When `emery build` runs over a revision whose ancestor is already built, only the plan parts the change reaches are scheduled, and each slice arrives over the built head with `## The change` in its brief. Carried stems stand in the tree as the earlier build left them. Within a scheduled slice, the adapter classifies each stem against the change before your turn opens, rewrites or removes crates in its own code, and holds every file outside your slice's reached crates to its pre-turn syntax save at the touch points the brief lists.

## How each stem lands

- **Rewritten (`crates/<stem>/` removed and re-laid).** The first slice of a change build that reaches a stem — through an `[added]` or `[changed]` requirement, a departure, a changed owned or used design type, or a changed design section — has `crates/<stem>/` removed before the turn and scaffolded fresh from the slice, as a first build lays it in the [layout](layout.md). Its three lines in the root `Cargo.toml` and `src/lib.rs` (`P: <stem>::Requires,` and `<stem>::handlers(),`) are already held. The scaffold brief says `Rewritten for this change: crates/<stem>/ is written from the slice, the prior crate is not in the tree.` Fill every marker of that stem from the brief's current specification and design: `[added]`, `[changed]`, and `[unchanged]` requirements alike start as fresh markers and ignored scenario tests in the re-laid crate, so leaving an `[unchanged]` requirement's marker unfilled leaves it unimplemented. Never reconstruct the old crate from memory, and never run `git` to read an earlier commit.
- **Extended (`held, extended`).** When a part is cut into several slices and an earlier slice of this build already rewrote the stem (`Built before in earlier slices of this part`), the adapter extends the held module and test file with this slice's markers and registrations. Fill only your new markers through `edit_files` and keep the handlers, types, functions, and tests that earlier slice wrote.
- **Kept.** When a slice spans more than one stem and one of those stems has only `[unchanged]` requirements, no departure, and no changed type, the adapter filters that stem out of the scaffold. Its crate stands in the tree with no markers laid; leave it untouched (save any listed touch point) and the adapter records its requirement ids to `covered`.
- **Departed requirements.** Requirements listed under `Departed from this part` (`[retired]` or `[moved to PART-..]`) belong to this part no longer. The re-laid skeleton carries no marker and no route or topic for them; write nothing for them.
- **Vanished stems and retiring slices.** When no requirement of the revision leads with a stem (`Vanished stems`), the adapter removes `crates/<stem>/` and drops its lines from the root `Cargo.toml` (`[dependencies]`), `src/lib.rs` (`P: <stem>::Requires,` and `<stem>::handlers(),`), and every stem manifest's `[dependencies]` before the turn. A retiring slice (cut for a removed part, holding no requirements) answers with no turn when nothing else in the tree named the vanished crate; when another crate named it, your turn opens to update those listed touch points so no `use` or item mentions the removed crate, runs the workspace check command, and reports `"covered": []`.

## Signatures and touch points

Before removing a rewritten crate, the adapter reads every `pub fn` its modules exported and lists them in the scaffold brief under `Exported signatures of the rewritten crates before this slice, which other crates call; keep them unless the slice changes them`. Keep each function's module path and signature in the rewritten crate whenever the current specification and design still permit it, as [handlers](handlers.md) spells exported helpers and rules, so callers in carried crates keep compiling with no edit.

Where another crate in the tree mentions a rewritten or vanished crate, the adapter lists each site under `Touch points outside the slice's reached crates`:

- `- crates/<other>/Cargo.toml:<line>: dependency <pkg>` — a path dependency on the crate;
- `- crates/<other>/src/<part>.rs:<line>: use` — an import from the crate;
- `- crates/<other>/src/<part>.rs:<line>: fn <name>` (or `struct`, `enum`, `const`, `impl`) — a top-level item whose body or signature mentions the crate or an imported symbol from it.

Outside your slice's reached crates, the adapter compares every file after your turn against its pre-turn syntax tree:

- Only a listed touch-point item (and `use` imports in a file holding a touch point, `Requires` and its `use` imports in `crates/<other>/src/lib.rs` when a touched handler's capability bounds change, or a listed dependency line in `Cargo.toml`) may change. If the rewritten crate kept the signature the caller uses, leave the touch point untouched too.
- Any edit, addition, or removal of another item in a carried or kept crate — a sibling function, a struct, a constant, a test — is refused with the item and line named; put it back through `edit_files`.
- Removing a file outside your slice's crates is refused with its prior text quoted; write it back through `write_files`.
- Any `use`, item, or manifest dependency still naming a vanished crate is refused at build and at verify; drop the mention or replace it with what the slice puts in its place through `edit_files`.

## Worked example

An earlier build sealed `crates/orders/` and `crates/checkout/`. `checkout` submits a cart after checking the order line limit through `orders::placing::within_limit` and reading stored orders under `orders::placing::order_key`. An amendment changes `REQ-010` so the line limit is passed by the caller rather than fixed at `24`, while `REQ-011` (`GET /orders/{id}`) is unchanged. `checkout` is carried; only `PART-001` (`orders-placing`, `SLICE-001`) is rebuilt.

The brief's `## The change` section reads:

```text
## The change

Built revision: `rev-001` at commit `8f3a1c2`.

Requirements in this slice:

- `REQ-010` `orders.within-limit` [changed]
  - Prior: Rejects a line count above 24.
  - Current: Rejects a line count above the caller's maximum.
- `REQ-011` `orders.read` [unchanged]
```

Before the turn, the adapter read `crates/orders/`'s exported signatures, removed `crates/orders/`, and laid `crates/orders/Cargo.toml`, `crates/orders/src/lib.rs`, `crates/orders/src/placing.rs`, and `crates/orders/tests/placing.rs` fresh, keeping `orders` in the root `Cargo.toml` and `src/lib.rs`. The scaffold brief lists the touch points in `crates/checkout/` and the prior exports of `crates/orders/`:

```text
Stem `orders`, part `placing`:

Rewritten for this change: `crates/orders/` is written from the slice, the prior crate is not in the tree.

...

Touch points outside the slice's reached crates that mention a reached or vanished crate, the only items outside those crates this turn may edit (along with `Requires` in that stem's `src/lib.rs` when a touched handler's capability bounds change):

- `crates/checkout/Cargo.toml:23`: `dependency orders`
- `crates/checkout/src/submitting.rs:4`: `use`
- `crates/checkout/src/submitting.rs:19`: `fn submit`

Exported signatures of the rewritten crates before this slice, which other crates call; keep them unless the slice changes them:

- `orders::placing::order_key` (`crates/orders/src/placing.rs:12`): `pub fn order_key(id: &str) -> String` — The state-store key of an order.
- `orders::placing::within_limit` (`crates/orders/src/placing.rs:18`): `pub fn within_limit(lines: usize) -> Result<()>` — Checks the line count.
```

The turn reads `crates/checkout/src/submitting.rs` once and sees `use orders::placing::{order_key, within_limit};`, `fn submit` calling `within_limit(input.lines.len())?` and `order_key(&input.id)`, and a sibling `fn cancel` below it. It then:

1. Widens `Requires` in `crates/orders/src/lib.rs` with `StateStore` and fills both markers in `crates/orders/src/placing.rs` from the current slice — implementing `within_limit(lines: usize, max: usize) -> Result<()>` for `REQ-010`, implementing `read` narrowed to `P: StateStore` for `REQ-011`, and keeping `pub fn order_key(id: &str) -> String` at its prior path and signature so `order_key`'s call site needs no change:

```json
{
  "edits": [
    {
      "path": "crates/orders/src/lib.rs",
      "find": "use omnia_sdk::api::Handlers;\n\n/// The capabilities the stem's handlers reach, implemented by every provider that has them.\npub trait Requires: Send + Sync + 'static {}\n\nimpl<P> Requires for P where P: Send + Sync + 'static {}\n",
      "replace": "use omnia_sdk::StateStore;\nuse omnia_sdk::api::Handlers;\n\n/// The capabilities the stem's handlers reach, implemented by every provider that has them.\npub trait Requires: StateStore + Send + Sync + 'static {}\n\nimpl<P> Requires for P where P: StateStore + Send + Sync + 'static {}\n"
    },
    {
      "path": "crates/orders/src/placing.rs",
      "find": "use omnia_sdk::Result;\n",
      "replace": "use omnia_sdk::{Result, StateStore, StateStoreExt as _, bad_request, not_found};\n"
    },
    {
      "path": "crates/orders/src/placing.rs",
      "find": "where\n    P: crate::Requires,\n",
      "replace": "where\n    P: StateStore,\n"
    },
    {
      "path": "crates/orders/src/placing.rs",
      "find": "REQ-011 orders.read: the handler",
      "replace": "    let order = context\n        .state()\n        .get_json(&order_key(&id))\n        .await?\n        .ok_or_else(|| not_found!(\"no order `{id}`\"))?;\n    Ok(Json(order))\n}\n\n/// The state-store key of an order.\n#[must_use]\npub fn order_key(id: &str) -> String {\n    format!(\"orders.order.{id}\")"
    },
    {
      "path": "crates/orders/src/placing.rs",
      "find": "REQ-010 orders.within-limit: the behaviour",
      "replace": "pub fn within_limit(lines: usize, max: usize) -> Result<()> {\n    if lines > max {\n        return Err(bad_request!(\"{lines} exceeds {max}\"));\n    }\n    Ok(())\n}"
    }
  ]
}
```

2. Fills the scenario tests for both `REQ-010` and `REQ-011` in `crates/orders/tests/placing.rs`, as [testing](testing.md) spells them.
3. Updates `fn submit` — the listed touch point in `crates/checkout/src/submitting.rs` — to pass the checkout part's configured limit, leaving `fn cancel`, `crates/checkout/Cargo.toml`, and `crates/checkout/tests/` untouched:

```json
{
  "edits": [
    {
      "path": "crates/checkout/src/submitting.rs",
      "find": "    within_limit(input.lines.len())?;\n",
      "replace": "    within_limit(input.lines.len(), settings.max_lines)?;\n"
    }
  ]
}
```

4. Runs the check command the brief spells (`cargo clippy -p orders --all-targets -- -D warnings && cargo test -p orders && cargo check --target wasm32-wasip3 && cargo +nightly fmt --all -- --check`), where the guest check compiles `checkout` against the rewritten `orders` crate, and reports every requirement of the slice and every file the tools wrote:

```json
{
  "covered": ["REQ-010", "REQ-011"],
  "written": [
    "crates/orders/src/lib.rs",
    "crates/orders/src/placing.rs",
    "crates/orders/tests/placing.rs",
    "crates/checkout/src/submitting.rs"
  ]
}
```
