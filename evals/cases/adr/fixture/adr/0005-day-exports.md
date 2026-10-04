# 0005 — Day exports

Status: Accepted

## Context

Finance loads each day's entries into the accounting system, which imports
files and nothing else. The first export was a spreadsheet an engineer
produced by hand on request; it was produced differently each time and
twice omitted a column the accounting import required.

## Decision

The ledger exports one file per closed day, in CSV with a fixed column
order: entry id, posted-at, debit account, credit account, amount in minor
units, currency code, idempotency key, description, and the id of the entry
reversed if any. An export covers exactly one closed day; a day still open
cannot be exported. Exporting a day twice produces byte-identical files.
Amounts are exported as the integer and the code of record 0003, never
formatted.

## Consequences

- The accounting import is configured once for the fixed column order and
  does not change when the ledger does; a new column is a new record.
- Because a closed day never changes, an export can be produced at any
  later time and still match the one produced on the day.
- An export is requested by day; a request for a range of days produces
  one file per day, not one file.
- The engineer's hand-made spreadsheet is retired.
