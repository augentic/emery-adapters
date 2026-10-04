# Settling invoices

Three stories: recording a payment against an invoice, reminding a buyer
whose invoice is overdue, and voiding an invoice issued in error.

## Record a payment

As a seller, I want to record a payment against an invoice, so that the
invoice shows what is still owed.

Acceptance:

- A payment is recorded with its date, amount, and method; the amount is
  positive and in the invoice's currency.
- An invoice may receive several payments; it is marked paid when the
  payments recorded against it total its amount.
- A payment that would take the total over the invoice amount is refused;
  overpayments are handled as a refund outside the invoice.
- Recording a payment against a voided invoice is refused.
- Every payment recorded is kept; a mistaken payment is reversed by a
  negative entry that refers to it, never deleted.

## Remind a buyer of an overdue invoice

As a seller, I want buyers reminded when an invoice is overdue, so that I
am not chasing payments by hand.

Acceptance:

- An invoice is overdue when it is unpaid thirty days after it was issued.
- A reminder is sent by email on the day the invoice becomes overdue and
  again every fourteen days while it stays unpaid, to a maximum of three
  reminders.
- No reminder is sent for an invoice that is paid or voided.
- The seller can turn reminders off for one buyer; the setting applies to
  every invoice of that buyer.

## Void an invoice

As a seller, I want to void an invoice issued in error, so that the
sequence stays complete without my carrying a bogus invoice as owed.

Acceptance:

- Only an invoice with no payment recorded against it can be voided.
- A voided invoice keeps its number and is shown as void; the number is
  never reissued.
- Voiding sends the buyer a notice that the invoice is void.

Decision: voiding is preferred to deletion so that the per-seller sequence
in the issuing story never has a gap an auditor would question.
