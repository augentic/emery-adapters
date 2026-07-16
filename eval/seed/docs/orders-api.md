# Orders service

The orders service accepts customer orders, tracks their state, and
exposes both over HTTP. This document is the written specification the
`documentation` source surveys; the operator intent narrows the change
to the service's API contracts.

## Placing an order

A client places an order by submitting the customer id, the line items
(product id plus quantity, at least one line), and an optional courier
note. The service validates the submission, assigns an order id, and
answers with the created order.

- An order with no line items is rejected.
- A line quantity below one is rejected.
- The order id is opaque to clients; clients never mint their own.

## Order state

Every order is in exactly one state: `pending`, `paid`, `shipped`, or
`cancelled`. Orders start `pending`. A `shipped` order can no longer be
cancelled.

Clients read a single order by id and receive its full detail: the
lines, the current state, and the timestamps of each state change. An
unknown order id answers not-found rather than an empty document.

## Cancelling an order

A client cancels an order by id. Cancelling a `pending` or `paid` order
succeeds and moves the order to `cancelled`; cancelling a `shipped`
order is refused with a conflict answer that names the current state.
