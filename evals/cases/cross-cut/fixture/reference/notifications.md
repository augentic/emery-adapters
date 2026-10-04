# Notifications

Depends on: [accounts](accounts.md).

The service tells members what has happened and what is about to happen
by message. Every notification is sent to the channels on the member's
account — email, text message, or both — and is also kept on the account
page for ninety days, so a member who missed a message can read it later.

## Channels

A member chooses, per account, whether to receive notifications by email,
by text message, or by both; at least one channel is always on, and the
service refuses to turn the last one off. A notification that cannot be
delivered — a bounced email, a failed text — is retried once after an hour
and then marked undelivered on the account page; three undelivered
notifications in a row to one channel turn that channel's indicator amber
on the account page and tell the member on their other channel, or at the
desk, that the service cannot reach them there. Notifications are sent
between eight in the morning and nine at night; a notification due outside
those hours is sent at eight the next morning, except a notification that a
reserved tool is waiting, which is sent at once because the hold clock has
started.

## What is sent

| Event | When | To |
| --- | --- | --- |
| Application received | On creating the account | The applicant |
| Application approved or declined | When the volunteer decides | The applicant |
| Application lapsed | Thirty days after applying with no decision | The applicant |
| Renewal reminder | Thirty days before the anniversary | The member |
| Membership lapsed or suspended | On the day, with the reason and end date | The member |
| Loan receipt | On opening, extending, or returning a loan | The member |
| Due reminder | Two days before the due date, and on it | The member |
| Overdue notice | Each seven days a loan is overdue | The member |
| Written off as lost | On the twenty-eighth day overdue | The member |
| Reservation made or cancelled | At once | The member |
| Tool waiting | When the reservation becomes waiting | The member |
| Hold about to lapse | On the morning of the last session of the hold | The member |
| Tool withdrawn | When a reserved tool is retired | Everyone in the queue |
| Charge added | With the receipt that carries it | The member |
| Charge disputed or decided | At once | The member and the committee |
| Password changed, sign-in locked | At once | The member |

A notification names the tool, the dates, and the amounts it refers to and
links to the page it concerns; it never carries another member's name or
contact details. The "tool waiting" notice says which desk sessions the
hold covers, and the "written off" notice says how to have the charge
refunded by returning the tool.

## Volunteer notifications

Volunteers staffing the next desk session are sent, on the morning of the
session, a list of the holds waiting for collection, the tools expected
back that day, and any damage reports since the last session. The
committee is sent each disputed charge, each suspension, and a monthly
summary of loans, late returns, and charges. Volunteer notifications go to
the volunteer's own account under the same channel rules as a member's.
