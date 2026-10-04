# 0002 — Idempotent posting

Status: Accepted

## Context

Callers post entries over a network that drops responses. A caller that
posts an entry and never hears back cannot tell whether the entry was
recorded, and the only safe thing it can do is post again. Without a rule
for repeated posting, every retry risks a duplicate entry, and duplicates
in a ledger are worse than lost entries because they look legitimate.

## Decision

Every posting carries an idempotency key the caller chooses, unique per
caller. Posting with a key the ledger has seen before returns the entry
recorded under that key and posts nothing. Posting with a key seen before
but a different body is refused as a conflict, since the caller is asking
for two different entries under one key. Keys are kept for ninety days;
posting with a key older than that is treated as new.

## Consequences

- A caller retries a posting with the same key until it gets an answer,
  and gets exactly one entry however many times it posts.
- The ledger stores the key beside the entry; the key is returned with the
  entry so a caller can match answers to requests.
- A key reused after ninety days produces a second entry. Callers that
  need longer protection keep their own record of what they have posted.
- The reversing entries of record 0001 are posted the same way and carry
  their own keys.
