# Returns API Design

The returns service lets customers request a return authorization for shipped orders.

Producer: returns-service
Consumers: storefront, customer-support-console

## HTTP Interface

POST /returns
Creates a return request.

Request ReturnRequest:

- order_id: string, required
- customer_id: string, required
- reason: string, required, enum: damaged, wrong_item, no_longer_needed, other
- items: array of ReturnItem, required, minItems 1

ReturnItem:

- sku: string, required
- quantity: integer, required, minimum 1

Responses:

- 202 ReturnRequestAccepted with return_id: string, status: string enum pending_review|approved|rejected, created_at: date-time
- 400 ErrorResponse for invalid input
- 404 ErrorResponse when order_id is unknown
- 409 ErrorResponse when the order is not returnable

GET /returns/{return_id}
Returns current return status.

Responses:

- 200 ReturnStatus with return_id, status, updated_at
- 404 ErrorResponse when return_id is unknown

ErrorResponse has code: string, message: string.
