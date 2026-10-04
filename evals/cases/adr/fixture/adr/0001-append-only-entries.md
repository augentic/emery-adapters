# 0001 — Append-only entries

Status: Accepted

## Context

The ledger records money moving between accounts. Early prototypes let an
operator edit an entry in place when a mistake was found, and the audit
trail lost the mistake along with the fix: an auditor reading the ledger a
month later saw only the corrected figure and had no way to know it had
ever been otherwise. Two incidents were traced to edits made in good faith
that moved a balance without anyone noticing.

## Decision

Entries are append-only. A posted entry is immutable: nothing about it —
its accounts, its amount, its date, its description — changes after it is
posted. A mistake is corrected by posting a reversing entry that references
the original by its entry id and carries the opposite amount, followed by a
fresh entry with the right figures. The ledger exposes no update and no
delete for entries.

## Consequences

- Every balance is the sum of the entries that touch the account, and that
  sum can be recomputed from the entries alone at any time.
- A reversing entry names the entry it reverses; an entry is reversed at
  most once, and a second reversal of the same entry is refused.
- Reports show reversed entries and their reversals rather than hiding
  them, so a reader sees what happened and what was done about it.
- Storage grows with every correction. The ledger is small enough that this
  is accepted; archiving closed periods is a later decision.
