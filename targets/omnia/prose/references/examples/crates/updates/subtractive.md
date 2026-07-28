# Subtractive update checklist

Use when artifacts no longer name an operation, field, route, or topic. Strategy detail: [`update-patterns.md`](../../../update-patterns.md).

1. Delete the operation module and types the artifacts no longer name.
2. Remove the typed HTTP route and/or exact messaging-topic registration from `$GUEST_PATH/src/lib.rs`.
3. Drop unused capability bounds and crate dependencies.
4. Delete or rewrite tests that targeted the removed surface; never leave compile-broken tests.
5. Record the removal in `CHANGELOG.md` with the reason (artifacts no longer require the behaviour).
6. `cargo check` / `cargo test` before leaving the category.
