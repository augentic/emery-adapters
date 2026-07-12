# Tasks — orders-api-contract

1. Read the three source files under `vendor/orders-service/` from the project workspace.
2. Author `contracts/schemas/create-order-request.yaml`, `contracts/schemas/order-item.yaml`, `contracts/schemas/order.yaml`, and `contracts/schemas/error-response.yaml` from the TypeScript types, under the slice delta.
3. Author `contracts/http/orders-api.yaml` covering exactly `POST /orders` and `GET /orders/{orderId}`, referencing the schemas by `$ref`, with `[unknown]` markers for wire-level details the source does not encode.
4. Verify: every `$ref` resolves, every schema carries its metadata, no endpoint beyond the declared entry points is transcribed.
