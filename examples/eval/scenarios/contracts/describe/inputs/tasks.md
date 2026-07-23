# Tasks — user-adapter-api

1. Author the shared JSON Schemas (`create-adapter-request`, `adapter`, `update-adapter-request`, `error-response`) under the slice's `contracts/schemas/`.
2. Author the OpenAPI binding `contracts/http/adapter-api.yaml` referencing those schemas by `$ref`.
3. Verify: every `$ref` resolves, every schema carries its metadata, every endpoint's request/response bodies bind to a schema.
