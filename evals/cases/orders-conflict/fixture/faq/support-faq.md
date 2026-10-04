# Orders — support FAQ

Answers the support team gives about the orders service. Where this FAQ
and the service's written specification differ, the one found to be wrong
is corrected; until then both stand as written.

## Can I cancel an order I have already paid for?

No. Once an order is `paid` it can no longer be cancelled; only a
`pending` order can be cancelled. A request to cancel a `paid` or
`shipped` order is refused with a conflict answer that names the order's
current state. To return a paid order, ask for a refund after it ships.

## Why was my order rejected?

An order is rejected when it has no line items, or when any line has a
quantity below one. Check each line's quantity and try again.

## I cannot find my order

Reading an order by an id the service does not know answers not-found.
Order ids are issued by the service when the order is placed; an id you
typed yourself, or one from another environment, will not be found.

## How do I know what state my order is in?

Read the order by id. The answer carries its current state — `pending`,
`paid`, `shipped`, or `cancelled` — and the timestamp of each change of
state, so you can see when it was paid or shipped.

## Can I change an order after placing it?

Not in this release. An order's lines and courier note are fixed once it
is placed; cancel it and place a new one while it is still `pending`.
