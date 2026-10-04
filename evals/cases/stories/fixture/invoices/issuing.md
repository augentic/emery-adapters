# Issuing an invoice

## Issue an invoice for a completed order

As a seller, I want an invoice issued for every completed order, so that my
buyers have a document for their records and I have one for my accounts.

Acceptance:

- An invoice is issued when the order is marked complete, not before.
- Each invoice carries a number that is unique per seller and increases by
  one with each invoice the seller issues, with no gaps.
- The invoice lists every line of the order with its quantity, unit price,
  and tax, and totals them.
- An issued invoice is immutable; a correction is a credit note that refers
  to the invoice it corrects.
- The buyer receives the invoice as a PDF by email within an hour of the
  order completing.
- An invoice is issued in the currency the order was paid in.

Decision: invoice numbers are per seller, not global, because sellers file
their own tax returns and auditors expect an unbroken sequence per filer.
