# Tasks — loyalty-api-contract

1. Author the shared JSON Schemas (`loyalty-enrollment-request`, `loyalty-enrollment`, `error-response`) under the slice's `contracts/schemas/`.
2. Author the OpenAPI binding `contracts/http/loyalty-api.yaml` referencing those schemas by `$ref`.
3. Verify: every `$ref` resolves, every schema carries its metadata, every endpoint's request/response bodies bind to a schema.
