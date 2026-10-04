# Ledger — architecture decision records

The decisions that shape the ledger service, one record each, numbered in
the order they were taken. A record is never edited after it is accepted;
a decision that changes is superseded by a new record that names the old
one.

| Record | Title | Status |
| --- | --- | --- |
| [0001](adr/0001-append-only-entries.md) | Append-only entries | Accepted |
| [0002](adr/0002-idempotent-posting.md) | Idempotent posting | Accepted |
| [0003](adr/0003-amounts-in-minor-units.md) | Amounts in minor units | Accepted |
| [0004](adr/0004-daily-close.md) | Daily close | Accepted |
| [0005](adr/0005-day-exports.md) | Day exports | Accepted |

Each record has a context, the decision, and its consequences, in that
order.
