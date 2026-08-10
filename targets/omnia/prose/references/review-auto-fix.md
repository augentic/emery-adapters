# Auto-Fix Protocol

> **When to read this**: Read this from the `repair` operation's review branch (`repair-origin: review`). It defines which standards findings are eligible for safe automatic repair, the regression guard (`cargo check`), expected success rates per category, and the recovery procedure if a fix breaks the build. The review operation itself never applies fixes — it reports findings and the engine routes them here.

## Applying safe fixes

The repair pass applies fixes directly. The finding prefix (SEC-, COR-, QUA-, UNI-) tracks which reviewer or pass identified the issue for accountability.

**FOR EACH** supplied auto-fixable finding (the engine's repair brief carries only confirmed blocking findings):

1. **Verify** fix is safe (no side effects, the finding carries no regression flag)
2. **Apply** fix using Edit tool
3. **Mark** issue as "Fixed" in the answer, noting the originating reviewer prefix
4. **Add** to auto-fix log

**RE-CHECK**: Run `cargo check` to verify fixes compile

```bash
cd $CRATE_PATH && cargo check 2>&1
```

If errors introduced:

- **REVERT** all auto-fixes
- **WARN** in report: "Auto-fix caused compilation errors; manual review required"

## Auto-Fix Success Rate (per category)

- **Error handling (unwrap→?)**: 90% success
- **WASM violations (std::env→Config)**: 80% success
- **Missing validation**: 60% success (some require business logic understanding)
- **Performance issues**: 10% success (most need architectural changes)
- **Code quality**: 5% success (semantic understanding required)

**Overall auto-fix rate**: ~40-50% of issues

## Common Issues and Resolutions

| Issue                              | Cause                                       | Resolution                                                               |
| ---------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------ |
| Crate path not found               | Incorrect `$CRATE_PATH` argument            | Verify the path exists and contains `src/` with `.rs` files              |
| No `.rs` files in `src/`           | Crate not yet generated or wrong directory  | Report as unrepairable; the build produced no candidate crate            |
| `cargo check` fails after auto-fix | Auto-fix introduced a compilation error     | Revert all auto-fixes and document issues for manual resolution          |
| Review report empty                | All files excluded or no issues found       | Verify `src/` directory is not empty; check file permissions             |
| Auto-fix modifies test files       | Test code scanned alongside production code | Review should focus on `src/` only; exclude `tests/` from auto-fix scope |

## Recovery Process

1. If a fix caused compilation errors: restore the pre-fix content of the specific files that fix edited (re-apply the content you read before editing — the lent workspace is a materialized snapshot, not necessarily a git checkout, and a tree-wide revert would destroy the build's own output)
2. Apply the remaining fixes manually based on the findings' remediations
3. Note anything left unrepaired in the answer — the engine's next verification and review passes are the authority; do not re-run either yourself

## Auto-Fix Verification Checklist

Before declaring an auto-fix pass complete:

- [ ] Only confirmed or upgraded auto-fixable issues fixed (not disputed)
- [ ] Antagonist regression flags respected (no fix applied if flagged)
- [ ] Lead applied all fixes (not delegated to specialists)
- [ ] All fixes verified with `cargo check`
- [ ] Modified files listed with originating prefix (SEC-, COR-, QUA-, UNI-)
- [ ] Revert performed if errors introduced
