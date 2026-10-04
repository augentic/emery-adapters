# 0004 — Daily close

Status: Accepted

## Context

Finance reconciles the ledger against the bank once a day. While entries
can still arrive for a day that is being reconciled, the figures move under
the reconciler's hands, and the team has twice signed off a day that
changed an hour later. The reconciler needs a day whose entries are fixed.

## Decision

Each calendar day in UTC closes at 23:59:59 that day, or when an operator
closes it early. A closed day accepts no new entries: an entry dated within
a closed day is refused, and the caller posts it with the current date and
a note naming the day it belongs to. Closing is one-way; a closed day is
never reopened. Reversals of entries in a closed day are posted in the
open day, as record 0001 requires of any correction.

## Consequences

- The entries of a closed day, and so its balances, never change, and a
  reconciliation signed against a closed day stays true.
- An entry that arrives late is visible in the day it was posted, with its
  note, rather than silently moving an earlier day's balance.
- The ledger keeps the close time of each day and who closed it, so an
  early close is attributable.
- Clients in other time zones see days cut at UTC midnight. The team
  accepted this over per-client day boundaries, which would make one entry
  fall in different days for different readers.
