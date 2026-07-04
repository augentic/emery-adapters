# Tasks — import-ticket-api-contract

1. Read `vendor/ticket-api.openapi.yaml` from the project workspace.
2. Decompose its inline schemas into `contracts/schemas/create-ticket-request.yaml` and `contracts/schemas/ticket.yaml` under the slice delta.
3. Author `contracts/http/ticket-api.yaml` as the OpenAPI 3.1 upgrade of the source document, referencing the decomposed schemas by `$ref`.
4. Verify: endpoint behavior matches the source, every `$ref` resolves, every schema carries its metadata.
