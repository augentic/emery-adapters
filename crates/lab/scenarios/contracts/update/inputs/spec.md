# Spec — Loyalty Enrollment API

All endpoints use `application/json`.

## Endpoints

`POST /loyalty/enrollments`

Request `LoyaltyEnrollmentRequest`:

- `customer_id`: string, required
- `email`: string, required, format email
- `referral_code`: string, optional

Responses:

- 201 `LoyaltyEnrollment` with `id`, `customer_id`, `tier`, `created_at`
- 400 `ErrorResponse` for invalid email
- 409 `ErrorResponse` when `customer_id` is already enrolled

`ErrorResponse` has `code`: string, `message`: string.

## Expected artifacts

- `contracts/http/loyalty-api.yaml`
- `contracts/schemas/loyalty-enrollment-request.yaml`
- `contracts/schemas/loyalty-enrollment.yaml`
- `contracts/schemas/error-response.yaml`
