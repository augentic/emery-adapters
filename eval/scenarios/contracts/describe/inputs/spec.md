# Spec — User Adapter HTTP API

Define a User Adapter HTTP API. All endpoints use `application/json`.

## Endpoints

1. `POST /adapters`
   - Request body `CreateProfileRequest`:
     - `user_id`: string, required
     - `display_name`: string, required, 1-80 chars
     - `timezone`: string, optional, IANA timezone
   - 201 response `Adapter`:
     - `id`: string
     - `user_id`: string
     - `display_name`: string
     - `timezone`: string|null
     - `created_at`: string date-time
   - 400 `ErrorResponse` for invalid fields
   - 409 `ErrorResponse` when a adapter already exists for `user_id`

2. `GET /adapters/{adapter_id}`
   - path parameter `adapter_id`: string, required
   - 200 response `Adapter`
   - 404 `ErrorResponse` when not found

3. `PATCH /adapters/{adapter_id}`
   - Request body `UpdateProfileRequest`:
     - `display_name`: string, optional, 1-80 chars
     - `timezone`: string|null, optional
   - 200 response `Adapter`
   - 400 `ErrorResponse` for invalid fields
   - 404 `ErrorResponse` when not found

## Error shape

`ErrorResponse` has `code`: string, `message`: string, and optional `details`: object.

## Expected artifacts

- `contracts/schemas/create-adapter-request.yaml`
- `contracts/schemas/adapter.yaml`
- `contracts/schemas/update-adapter-request.yaml`
- `contracts/schemas/error-response.yaml`
- `contracts/http/adapter-api.yaml`
