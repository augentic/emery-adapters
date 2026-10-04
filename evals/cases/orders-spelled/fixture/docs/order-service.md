# OrderService

`OrderService` is the one exported class of the orders module. It keeps
orders in memory, mints their ids, and exposes three methods: `place`,
`read`, and `cancel`. Each method either returns the `Order` or throws one
of `ValidationError`, `NotFoundError`, and `ConflictError`.

## place

`place` takes a `PlaceOrder` — a `customerId`, the `lines` (each a
`productId` and a `quantity`), and an optional `courierNote` — and returns
the created `Order`.

- `place` with no lines throws `ValidationError`.
- `place` with any line whose `quantity` is below one throws
  `ValidationError`.
- `place` assigns the id `ord_<n>`, with `n` counting up from one, stores
  the order in state `pending` with one `pending` transition, and returns
  it.

## read

`read` takes an order id and returns the `Order`.

- `read` of an id no order carries throws `NotFoundError` naming the id.

## cancel

`cancel` takes an order id, reads the order, and returns it cancelled.

- `cancel` of a `shipped` order throws `ConflictError` carrying the state.
- `cancel` of any other order sets its state to `cancelled`, appends a
  `cancelled` transition, and returns the order.
