# Logic Review Checks

These checks cannot be detected by pattern-matching alone. They require simulating event sequences, enumerating state transitions, and reasoning about what happens when async operations interleave. Each check includes a simulation technique and a concrete example drawn from real issues found in generated code.

The reviewer should read `spec.md` alongside `app.rs` when applying these checks -- many logic bugs originate from gaps between what the spec describes and what common sense requires.

---

## LOG-001: State machine completeness

**Severity**: critical

**What to check**: For every enum used as a page, phase, or connection state (`Page`, `SyncStatus`, `SseConnectionState`, etc.), enumerate all transitions in `update()`. For each transition, verify that every required side-effect fires.

**Simulation technique**:

1. List all values of each state enum.
2. For each Event arm in `update()`, identify assignments to state fields.
3. For each assignment, list the side-effects in the returned `Command`:
   - `render()` -- does the view read this field?
   - `save_state()` -- is this field persisted?
   - Sync/reconnect -- does the transition require follow-up actions?
4. Flag any transition that mutates a view-visible field without `render()`.

**Example**: `ConnectSse` sets `model.sse_state = Connecting` but returns only the SSE command without `render()`. The UI never shows a "connecting" indicator. Similarly, navigating from `Page::Error` back to `Page::Loading` changes the page but omits `render()`, so the Error view stays visible until `DataLoaded` fires.

**State machine to verify** (for a typical sync app):

```
Page:  Loading -> TodoList (on DataLoaded Ok)
       Loading -> Error    (on DataLoaded Err)
       Error   -> Loading  (on Navigate/Retry)

SseConnectionState:  Disconnected -> Connecting  (on ConnectSse)
                     Connecting   -> Connected   (on SseReceived)
                     Connected    -> Disconnected (on SseDisconnected)

SyncStatus:  Idle    -> Syncing  (on start_sync)
             Syncing -> Idle     (on OpResponse Ok)
             Syncing -> Offline  (on OpResponse Err)
             Offline -> Syncing  (on RetrySync)
```

Each edge must emit `render()` if the `view()` function reads the field.

---

## LOG-002: Operation coalescing

**Severity**: critical

**What to check**: When a destructive operation (Delete, ClearCompleted) targets an item that only exists as a pending Create (never synced to the server), the code must skip the server call entirely.

**Simulation technique**:

1. Trace this sequence:
   - User creates item A (pending Create pushed)
   - Sync has NOT yet run
   - User deletes item A (or clears completed including A)
2. After the delete handler runs, inspect `model.pending_ops`:
   - Does it contain a `PendingOp::Delete` for item A?
   - If yes: the sync loop will send a DELETE to the server for an item the server has never seen -> 404 or error
3. Correct behavior: the handler detects that A's only pending op is `Create`, removes the `Create`, and does NOT push a `Delete`.

**What to look for in code**:
```rust
// BAD: blindly replaces all ops with a Delete
model.pending_ops.retain(|op| op.item_id() != id);
model.pending_ops.push(PendingOp::Delete(id));

// GOOD: inspects what ops existed before deciding
let mut saw_create = false;
let mut saw_non_create = false;
model.pending_ops.retain(|op| { /* categorize and remove */ });
if saw_create && !saw_non_create {
    continue; // nothing to delete on server
}
```

Also apply this check to `ClearCompleted`, which must apply the same logic per-item in a loop.

---

## LOG-003: Concurrent operation conflicts

**Severity**: critical

**What to check**: When a sync operation is in-flight (`model.syncing_id` is `Some(id)`) and a real-time event (SSE) arrives for the same item, the pending-op cleanup must not corrupt the sync state.

**Simulation technique**:

1. Trace this sequence:
   - Sync starts for item A: `syncing_id = Some("A")`, the first pending op for A is being sent to the server
   - SSE `item_deleted` arrives for item A
   - SSE handler runs `pending_ops.retain(|op| op.item_id() != "A")` -- this removes the op that is currently being synced
   - Server responds with `OpResponse(Ok(...))` or `DeleteOpResponse(Ok(...))`
   - Handler does `syncing_id.take()` and `pending_ops.retain(|op| op.item_id() != synced_id)`
2. What goes wrong: the SSE handler already removed the op, so the response handler retains everything and moves on. But if a second pending op for A was queued after the first, it may also have been removed by the SSE handler's overly broad `retain`.

**What to look for**:
- SSE `item_deleted` handler: does it check `syncing_id` before removing ops?
- Does it only remove ops that are NOT currently being synced?
- Is there a test that covers this exact interleaving?

---

## LOG-004: Temporal ordering / conflict resolution

**Severity**: critical

**What to check**: Every conflict-resolution comparison must have timestamps available on both sides. If either side can be `None` or missing, the comparison logic must be explicitly designed for that case rather than falling through to a default.

**Simulation technique**:

1. Find the `apply_server_item` or equivalent function.
2. For each comparison between local and server state:
   - What data is available on the local side? (fields of `PendingOp`, fields of the local `TodoItem`)
   - What data is available on the server side? (fields of the server response)
   - Is there a comparison like `server_ts >= local_ts`?
3. For `PendingOp::Delete`: does it carry a `deleted_at` timestamp? Without it, the code cannot determine whether a server update happened before or after the local delete.
4. Check fallback cases: if either timestamp is `None`, does the code explicitly decide who wins, or does it fall through to a default?

**What to look for**:
```rust
// BAD: Option with implicit server-wins fallback
match (&local.updated_at, &server.updated_at) {
    (Some(l), Some(s)) => s >= l,
    _ => true, // server wins when either is None -- data loss risk
}

// GOOD: non-optional field eliminates the ambiguity
server_item.updated_at >= local_item.updated_at
```

---

## LOG-005: Fallback-on-None / default semantics

**Severity**: important

**What to check**: For every `unwrap_or_default()`, `Option` with a `_ => true` catch-all, or `None` fallback path, verify that the default value is semantically correct in the domain.

**Simulation technique**:

For each instance, ask these questions:
- What does the zero/empty/default value mean in this domain?
- Is `""` (empty string) a valid title? (Usually no.)
- Is `0` a valid count or does it mean "unknown"? (Context-dependent.)
- Is "no timestamp" older than all timestamps or newer? (Neither -- it's ambiguous, which is why it should not be `Option` in the first place.)
- Does `unwrap_or_default()` on a serialization failure silently produce an empty state that will overwrite valid persisted data?

**Example**:
```rust
// Risky: if serialization fails, replaces valid state with empty defaults
let state: PersistedState = serde_json::from_slice(&bytes).unwrap_or_default();
```

If `bytes` is corrupted, this silently initializes with an empty state, discarding all the user's data. Consider logging the error or returning it to the shell.

---

## LOG-006: Rapid-action sequences

**Severity**: important

**What to check**: Verify correct behavior when the user performs the same action multiple times faster than async operations can complete.

**Simulation technique**:

1. Trace: user toggles item A -> first sync starts -> user toggles item A again before sync completes
   - Does a second `PendingOp::Update` get pushed?
   - When the first sync completes and the handler runs `start_sync`, does it pick up the second op correctly?
   - Are there now duplicate `Update` ops for the same item?

2. Trace: user clicks "Add" rapidly 5 times with the same text
   - Are 5 items created with different IDs? (Correct if IDs come from shell.)
   - Or are 5 items created with sequential IDs that collide with existing items? (Bug if IDs are generated from a model counter.)

3. Trace: user deletes item A, then item B, then item C in rapid succession
   - Does `start_sync` only process one at a time (correct)?
   - Or does it start multiple syncs concurrently, potentially corrupting `syncing_id`?

**What to look for**:
- `start_sync` should check `syncing_id.is_some()` and return `Command::done()` if a sync is already in-flight.
- Pending ops should not accumulate duplicates for the same item unless the item's state genuinely changed between ops.

---

## LOG-007: Spec gap detection

**Severity**: important

**What to check**: Compare each user-facing Event variant against the Features section of `spec.md`. For each Event whose **happy-path THEN is already concrete** (page, state, validation, or effect named — not an open-GAP withhold), identify untrusted inputs and verify that common-sense **input validation** exists even when the spec is silent on edge cases.

**Scope (do not over-read)**: LOG-007 covers **validation / adversarial inputs** on otherwise specified actions (empty strings, unknown IDs, invalid toggle state). It is **not** a license to invent navigation, route/page changes, or domain outcomes when the scenario withholds the result (`unspecified`, operator-must-supply, design TBD, composition `# GAP`). That inventiveness case is **LOG-010** / [`open-gap-contract.md`](../open-gap-contract.md).

**Simulation technique**:

For each user-facing Event (not internal/callback events) with a concrete happy-path THEN:

1. What inputs does it accept? (Strings, IDs, booleans)
2. What are the preconditions the spec states? (Usually none for simple actions.)
3. What preconditions does common sense require for **input validity**?
   - Text inputs: non-empty after trimming?
   - IDs: does the referenced item exist in the model?
   - Toggles: is the item in a valid state for toggling?
4. What happens with adversarial input?
   - Empty string for title
   - ID that doesn't match any item
   - Duplicate add with same ID

**Example**: The spec says "Edit title -- user edits the title of a todo item." It does not mention empty titles. But accepting an empty title creates an invisible item in the list. The generated code should reject empty titles regardless of spec silence on validation.

**Cross-reference**: Each Event should map to at least one Feature in the spec. Events with no spec Feature may indicate dead code or missing spec coverage.

---

## LOG-008: Spec-to-test coverage gap

**Severity**: important

**What to check**: Perform a systematic spec-to-test coverage analysis. For each `#### Scenario:` in `spec.md`, verify that a test with a matching `/// Spec:` traceability comment exists in the `#[cfg(test)]` module. The traceability comment format is:

```
/// Spec: specs/<domain>/spec.md > REQ-XXX > Scenario: <scenario title>
```

If traceability comments are present (generated by test-writer), match by `REQ-XXX` ID **and** scenario title -- a single requirement can contain multiple scenarios, and each scenario must have its own test. If traceability comments are absent (legacy tests), fall back to matching by test function name against scenario title (approximate).

**Coverage analysis steps**:

1. Extract all `#### Scenario:` blocks from the spec, keyed by their parent `### Requirement:` block's `ID: REQ-XXX` line.
2. Extract all `/// Spec:` traceability comments from the test module.
3. For each spec scenario, check whether a test with a `/// Spec:` comment referencing the same `REQ-XXX` ID **and** scenario title exists. A requirement with 3 scenarios requires 3 distinct tests -- matching the `REQ-XXX` ID alone is insufficient.
4. For each test with a traceability comment, check whether the referenced scenario still exists in the spec (stale test detection -- see LOG-009).

**Additionally**, cross-reference the `#[cfg(test)]` module against the interaction sequences identified by LOG-001 through LOG-007. Each identified risk should have at least one test.

**Required edge-case scenarios** (minimum set for a sync app):

| Scenario | Checks | Why |
|---|---|---|
| SSE event during in-flight sync for same item | LOG-003 | Race condition between SSE and sync completion |
| SSE delete does not clobber next pending op | LOG-003 | Ensures retain() is scoped to the right op |
| EditTitle with empty string is a no-op | LOG-007 | Input validation on untrusted text |
| ClearCompleted with no completed items | LOG-007 | Edge case: nothing to do |
| ClearCompleted coalesces pending Creates | LOG-002 | No phantom server deletes |
| Server-wins conflict resolution | LOG-004 | Server has newer timestamp |
| Local-wins conflict resolution | LOG-004 | Local has newer timestamp |
| Rapid toggle of same item | LOG-006 | No duplicate pending ops |
| Delete of item that was never synced | LOG-002 | Create->Delete before sync |

**Detection**: For spec-to-test coverage, parse `/// Spec:` comments and compare against spec scenarios by `REQ-XXX` ID and scenario title. For edge-case coverage, search the test module for function names or assertion patterns that cover each scenario. Missing coverage is an `important` finding. List all missing scenarios in the review report, distinguishing spec-coverage gaps from edge-case gaps.

---

## LOG-009: Stale tests

**Severity**: important

**What to check**: Identify tests with `/// Spec:` traceability comments that reference scenarios no longer present in the spec. These are tests whose behavioral justification has been removed -- the test may be covering behavior that was intentionally changed or deleted.

**Detection**:

1. Extract all `/// Spec:` traceability comments from the `#[cfg(test)]` module. Parse each for the `REQ-XXX` ID and scenario title.
2. For each traceability comment, search the spec for a matching `### Requirement:` block with `ID: REQ-XXX` and a `#### Scenario:` with the referenced title.
3. Flag tests whose referenced scenario no longer exists. Distinguish:
   - **Requirement removed**: The `REQ-XXX` ID no longer appears in the spec.
   - **Scenario removed**: The `REQ-XXX` ID exists but the specific scenario title is gone (the requirement was modified).
   - **Scenario renamed**: The `REQ-XXX` ID exists with a similar but not identical scenario title (possible rename -- flag for human review).

**Example**: A test has `/// Spec: specs/todo/spec.md > REQ-003 > Scenario: Clear completed removes all done items`. The spec no longer has REQ-003. The test is stale -- it may test behavior that was intentionally removed or moved to a different requirement.

**Action**: Stale tests should be flagged in the review report. Do not auto-delete them -- the human decides whether the test covers behavior that moved to a different requirement or is truly obsolete. The test-writer skill marks stale tests with `// STALE: scenario removed from spec` comments during update mode.

---

## LOG-010: Open-GAP inventiveness

**Severity**: important

**What to check**: Flag handlers and tests that invent concrete navigation or domain state while any [open-GAP marker](../open-gap-contract.md#open-gap) remains for that interaction in the **current slice** artifacts. Normative contract: [`open-gap-contract.md`](../open-gap-contract.md).

**Detection**:

1. For each user-facing Event handler in `update()` that changes page / route / tab / domain state beyond `render()` (or documented stub side effects already present):
   - Does any open-GAP marker still apply? Spec scenario THEN/body withholds the outcome (`unspecified`, `operator must supply acceptance criteria`, or equivalent); `design.md` TBD / unanswered risk for that Event; composition `# GAP` on or adjacent to the wired control; missing concrete THEN; and/or `model.yaml` scenario text still unspecified when `spec.md` body was not closed under B′ eligibility in this build.
2. For each `/// Spec:` test that asserts a concrete destination or invented domain mutation: does the cited scenario still withhold the outcome (open GAP)?
3. **Clean when intentional**: open GAP + stub-faithful handler (`render()` / no invented page change) + stub-faithful asserts (page/route unchanged for the unspecified dimension) → **no blocking finding**.

**Not LOG-010** (keep on LOG-007): happy-path THEN is concrete and the only silence is input-validation / adversarial edge cases.

**Classification**:

- Default **`code-fix`** — emit the finding with a remediation naming either path: revert to stub-faithful, **or** [§ Closure eligibility](../open-gap-contract.md#closure-eligibility) B′ closure (close build-editable `spec.md` scenario body + design TBD + composition `# GAP`, wire only a grounded destination, no contradictory Evidence). The fix itself happens in the engine-dispatched `repair(origin: review)` pass — review only reports.
- Use **`spec-change`** only when Evidence truly blocks honest closure (operator must amend / refine first). Naming pressure alone (`Add list`, `CreateList`, prior-slice hints) is not Evidence and does not justify `spec-change` over `code-fix`.

**Severity posture**: Keep **`important`** (blocking). Do not downgrade inventiveness to advisory when `# GAP` / unspecified / TBD markers remain.

**Example**: Composition wires FAB `CreateList` with `# GAP: REQ-026 destination unspecified`; design lists CreateList behaviour TBD; scenario THEN says unspecified. Handler sets `model.page = Page::NewList` and a test asserts that page. → LOG-010 `code-fix` (revert to stub, or close B′ surfaces and assert the closed THEN).
