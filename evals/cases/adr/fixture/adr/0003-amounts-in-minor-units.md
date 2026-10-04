# 0003 — Amounts in minor units

Status: Accepted

## Context

The first version stored amounts as decimal strings and parsed them on
every read. Two bugs came from the parsing: one locale wrote a comma where
another expected a point, and a rounding step in a report summed to a
figure a cent off the ledger's own total. Both were found by customers.

## Decision

An amount is an integer count of the currency's minor units together with
the currency's ISO 4217 code: 1234 with code `EUR` is twelve euros and
thirty-four cents. The ledger stores and transmits amounts in this form
and never as a decimal or a float. The currency's exponent — two for most
currencies, zero for yen, three for dinar — comes from a table the ledger
ships; an amount in a currency the table does not list is refused.

## Consequences

- Sums of amounts are integer sums and are exact.
- Every amount carries its currency, and an entry whose two sides carry
  different currencies is refused: currency conversion is a pair of
  entries against an exchange account, not an arithmetic step inside one.
- Formatting for display is the reader's concern. The ledger's API returns
  the integer and the code, and clients place the decimal point from the
  exponent table.
- The table of exponents is part of the ledger's release and changes only
  with a release.
