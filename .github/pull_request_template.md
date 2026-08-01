# Summary

<!-- What and why, in a sentence or two. -->

## Testing checklist

Placement rules: [docs/testing.md](../docs/testing.md). The default write path is the adapter's `tests/` suite; a `src` unit test is the exception.

- [ ] Every new assertion names the layer that owns it: kernel unit (`src` `#[cfg(test)]`) or crate integration (`{sources,targets}/<name>/tests/`) — and the rung that owns it (native crate tests, eval case, wasm example). No assertion is duplicated across rungs.
- [ ] Any new `src` `#[cfg(test)]` carries a one-line **Keep** or **Collapse** reason (unreachable defensive branch, or dense pure matrix cheap only in-process).
- [ ] No `pub` / `pub(crate)` was widened solely to make a test reachable, and no test-only trait pairs were added.
- [ ] If unit coverage was deleted or re-homed, the coverage brake ran before and after (`CRATE=<adapter> cargo make cov`) and `TOTAL` held on still-live code.
- [ ] `cargo make ci` passes (or the PR states exactly which narrower checks ran and why).

## DCO

- [ ] All commits carry a `Signed-off-by` line (`git commit -s`).
