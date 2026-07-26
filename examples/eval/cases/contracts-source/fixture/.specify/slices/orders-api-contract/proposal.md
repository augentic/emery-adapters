# Proposal — orders-api-contract

Reverse-engineer API contracts from an existing TypeScript service (authorship mode: extract from source code).

Source material (present in the project workspace):

- `vendor/orders-service/src/index.ts`
- `vendor/orders-service/src/orders/handlers.ts`
- `vendor/orders-service/src/orders/types.ts`

Analysis context:

- discovery adapter: orders
- entry_points: `POST /orders`, `GET /orders/:orderId`

Participants:

- orders-service: producer
- storefront: consumer
- fulfillment-console: consumer

Scope discipline: capture endpoint paths and methods from the express route registrations, status codes from the handler return sites, and payload shapes from the TypeScript interfaces and literal types. Mark wire-level details the source does not encode — Content-Type, auth headers, pagination semantics, rate limits, idempotency-key conventions — with `[unknown]` rather than guessing. Stay within the analysis-identified entry points; flag any additional surface as `[manual review required]` rather than silently expanding the contract change.
