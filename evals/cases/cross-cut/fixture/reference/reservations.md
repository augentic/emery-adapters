# Reservations

Depends on: [catalogue](catalogue.md), [loans](loans.md),
[reservation limits](../policies/reservation-limits.md),
[notifications](notifications.md).

A reservation is a member's place in the queue for one tool. Reservations
are for a particular catalogue entry — a member who wants any drill
reserves one drill — and a tool's queue is served strictly in the order
the reservations were made.

## Making a reservation

An `active` member may reserve a tool whose availability is `on loan`,
`held` for someone else, or `on shelf`; a tool that is `unavailable` cannot
be reserved. The service refuses a reservation when the member already has
a reservation on the same tool, when the member has reached the number of
open reservations the reservation limits policy allows, when the member is
under the reduced limit for missed collections, or when the member has an
overdue loan, and in each case says which rule refused it. A reservation
on a tool that is `on shelf` is a shelf hold: it holds the tool until the
end of the next desk session, and lapses then if the member has not
collected it.

## The queue

When a reserved tool is returned, or when the member at the head of the
queue lets their turn lapse, the next reservation becomes `waiting`: the
member is notified that the tool is at the desk for them, the tool's
availability becomes `held`, and the hold lasts until the end of the
second desk session after the notice. Collecting the tool opens a loan and
closes the reservation as `collected`. A hold that lapses closes the
reservation as `missed` and the tool passes to the next in the queue, or
returns to the shelf if the queue is empty.

A member's position in the queue, and an estimate of when their turn will
come from the open loan's due date and the holds ahead of them, is shown on
the tool's page and on the account page. The estimate assumes every loan
returns on its due date and every hold is collected on its first session.

## Cancelling

A member may cancel a reservation that is not yet `waiting` at any time
and it closes as `cancelled`, counting against nothing. Cancelling a
reservation that is `waiting` closes it as `missed`. A volunteer may cancel
any reservation at the desk on the member's behalf under the same rules.

## Withdrawn tools

When a tool with a queue is written off or retired, every open reservation
on it closes as `withdrawn`, which counts against no member, and each
member in the queue is notified; the notice names another tool of the same
kind if the catalogue has one available. A `waiting` hold on a withdrawn
tool is cancelled with the rest.

## Interaction with loans

A tool with any open reservation cannot have its current loan extended,
and is never lent to a member who is not at the head of its queue. A
member who is `waiting` on a tool and has an overdue loan may still
collect it only once the overdue tool is returned; the hold runs on
meanwhile and lapses on its normal date.
