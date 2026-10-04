# Error codes

Every refusal the service makes carries a code from this table, a message
a member can read, and, where it helps, the rule that refused the action.
The codes are stable: a code is never reused for a different refusal, and
a refusal that is retired keeps its code in the table marked retired. The
app shows the message; the code is for volunteers reading logs and for the
people who maintain the service.

The codes are grouped by the part of the service that raises them. The
"rule" column names the document the refusal comes from.

## Accounts

| Code | Message shown | Rule |
| --- | --- | --- |
| `ACC-001` | Give an email address or a mobile number so we can reach you. | accounts |
| `ACC-002` | Your application is waiting for a volunteer to approve it. | accounts |
| `ACC-003` | Your application lapsed after thirty days; please apply again. | accounts |
| `ACC-004` | Your membership has lapsed; renew it to borrow or reserve. | accounts |
| `ACC-005` | Your membership is suspended until the date in your notice. | accounts |
| `ACC-006` | That card has been cancelled; the replacement card number is shown. | accounts |
| `ACC-007` | A committee member is needed for that action. | accounts |
| `ACC-008` | A suspension needs a reason and an end date within six months. | accounts |
| `ACC-009` | Proof of address must be dated within the last three months. | membership eligibility |
| `ACC-010` | Applicants must be eighteen or over. | membership eligibility |
| `ACC-011` | That address is outside the area Toolshed serves. | membership eligibility |
| `ACC-012` | One membership per person; this person already has an account. | membership eligibility |

## Signing in

| Code | Message shown | Rule |
| --- | --- | --- |
| `SIG-001` | The details do not match an account. | accounts |
| `SIG-002` | Too many attempts; try again in fifteen minutes. | accounts |
| `SIG-003` | That code has expired or been used; ask for a new one. | accounts |
| `SIG-004` | Three wrong codes; ask for a new one. | accounts |
| `SIG-005` | Passwords are at least twelve characters. | accounts |
| `SIG-006` | That password is too commonly used; choose another. | accounts |
| `SIG-007` | That reset link has expired or been used; ask for a new one. | accounts |
| `SIG-008` | Your temporary password must be changed before you continue. | accounts |

## Catalogue

| Code | Message shown | Rule |
| --- | --- | --- |
| `CAT-001` | No tool has that label number. | catalogue |
| `CAT-002` | That tool is under repair and cannot be lent or reserved. | catalogue |
| `CAT-003` | That tool has been written off and retired. | catalogue |
| `CAT-004` | Damage has been reported on that tool; a volunteer must inspect it first. | catalogue |
| `CAT-005` | Toolshed does not accept tools of that kind. | catalogue |
| `CAT-006` | Replacement value must be a whole number of pounds. | catalogue |
| `CAT-007` | Each part in a case needs a value. | catalogue |
| `CAT-008` | That label number is already in use. | catalogue |

## Loans

| Code | Message shown | Rule |
| --- | --- | --- |
| `LOAN-001` | Your membership is not active. | loans |
| `LOAN-002` | That tool is not on the shelf. | loans |
| `LOAN-003` | That tool is held for another member. | loans |
| `LOAN-004` | That tool is reserved; it goes to the reservation first. | loans |
| `LOAN-005` | You have reached your loan limit. | fair use |
| `LOAN-006` | New members may have one tool out in their first month. | fair use |
| `LOAN-007` | You have an overdue tool; return it before borrowing another. | loans |
| `LOAN-008` | You have unpaid charges over ten pounds. | loans |
| `LOAN-009` | The due date cannot be later than the tool's loan period. | loans |
| `LOAN-010` | This loan has already been extended once. | loans |
| `LOAN-011` | This loan cannot be extended because the tool is reserved. | loans |
| `LOAN-012` | An overdue loan cannot be extended. | loans |
| `LOAN-013` | An extension is one to seven days. | loans |
| `LOAN-014` | Tools of this kind count double toward your limit. | fair use |
| `LOAN-015` | Only one high-demand tool may be out at a time. | fair use |

## Reservations

| Code | Message shown | Rule |
| --- | --- | --- |
| `RES-001` | That tool cannot be reserved while it is unavailable. | reservations |
| `RES-002` | You already have a reservation on that tool. | reservations |
| `RES-003` | You have reached your reservation limit. | reservation limits |
| `RES-004` | Your reservation limit is reduced after missed collections. | reservation limits |
| `RES-005` | You have an overdue tool; return it before reserving. | reservations |
| `RES-006` | This reservation is waiting for collection and cannot be cancelled without counting as missed. | reservations |
| `RES-007` | A shelf hold lasts until the end of the next desk session. | reservations |
| `RES-008` | That reservation has been withdrawn because the tool was retired. | reservations |

## Returns, damage, and charges

| Code | Message shown | Rule |
| --- | --- | --- |
| `RET-001` | That tool is not on loan. | returns |
| `RET-002` | A return cannot be backdated. | returns |
| `RET-003` | A condition finding needs a note, and a photograph when damaged. | returns |
| `RET-004` | A dispute must be raised within seven days of the receipt. | returns |
| `RET-005` | This loan was written off as lost; returning it refunds the charge. | returns |
| `DMG-001` | A damage report needs a sentence saying what happened. | damage and loss |
| `DMG-002` | Damage can only be reported on a loan that is open. | damage and loss |
| `DMG-003` | A police reference is needed to waive a theft. | damage and loss |
| `DMG-004` | Wear is not charged; a volunteer will confirm on inspection. | damage and loss |
| `CHG-001` | A charge can be paid at the desk in cash or card, or in the app by card. | damage and loss |
| `CHG-002` | A charge over fifty pounds may be paid in up to three monthly parts. | damage and loss |
| `CHG-003` | A disputed charge cannot be paid until the dispute is decided. | returns |
| `CHG-004` | Retired: charges were once capped per loan; the cap was removed. | — |

## Notifications

| Code | Message shown | Rule |
| --- | --- | --- |
| `NOT-001` | At least one notification channel must stay on. | notifications |
| `NOT-002` | We could not deliver to that channel; check the address or number. | notifications |
| `NOT-003` | Notifications are sent between eight in the morning and nine at night. | notifications |

## How codes are assigned

A code is a prefix naming the part of the service — `ACC`, `SIG`, `CAT`,
`LOAN`, `RES`, `RET`, `DMG`, `CHG`, `NOT` — and a three-digit number
assigned in the order the refusal was added. The number is never reused:
when a refusal is retired its row stays in the table, its message
replaced by a note beginning "Retired:" and its rule column struck
through, so that a log from before the retirement can still be read. A
new refusal takes the next number in its prefix however many have been
retired. When the rule behind a refusal moves from one document to
another — a policy folded into a reference, or the reverse — the code
stays and the rule column is updated, since the code names the refusal
and not the document.

The message in the table is the one shown to a member in the app and read
out by a volunteer at the desk; the people who maintain the service see
the code and the message together in the logs, with the account and the
tool the refusal concerned. Messages are written for the member who sees
them: they say what to do next where there is something to do, they name
the limit or the date where one applies, and they never blame.
