# Orders service

The orders service accepts customer orders, tracks their state, and
exposes both over HTTP. This document is the written specification the
`documentation` source extracts; the operator intent scopes the
specification to the service's externally observable behaviour.

## Placing an order

A client places an order by submitting the customer id, the line items
(product id plus quantity, at least two lines), and an optional courier
note. The service validates the submission, assigns an order id, and
answers with the created order.

- An order with fewer than two line items is rejected.
- A line quantity below one is rejected.
- The order id is opaque to clients; clients never mint their own.

## Order state

Every order is in exactly one state: `pending`, `paid`, `shipped`, or
`cancelled`. Orders start `pending`. A `paid` or `shipped` order can no
longer be cancelled.

Clients read a single order by id and receive its full detail: the
lines, the current state, and the timestamps of each state change.

## Cancelling an order

A client cancels an order by id. Only a `pending` order can be cancelled;
cancelling a `paid` or `shipped` order is refused with a conflict answer
that names the current state.
