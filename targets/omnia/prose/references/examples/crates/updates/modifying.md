# Modifying update checklist

Use when behaviour, validation, output shape, or provider bounds change inside existing operations. Prefer idioms evidenced in the consumer crate when its Omnia pin differs from the exemplar — see [`exemplar.md`](../../../exemplar.md). Strategy detail: [`update-patterns.md`](../../../update-patterns.md).

1. Diff artifacts against the current operation; list every changed validation / side effect / bound.
2. Edit the operation in place — do not rename or move files in this category.
3. Update `impl From<DomainError> for omnia_guest::Error` arms when error variants change.
4. Adjust guest projectors only when the transport envelope changes.
5. Align tests with the new expected behaviour; treat unrelated failures as regressions.
6. Update `CHANGELOG.md` for observable behaviour changes.
7. `cargo check` / `cargo test` before leaving the category.
