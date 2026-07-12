# Proposal — import-ticket-api-contract

Import an externally supplied OpenAPI document (authorship mode: import existing contracts).

Source material: `vendor/ticket-api.openapi.yaml` (present in the project workspace).

Participants:

- ticket-service: producer

Scope: normalise the vendor document — preserve the source endpoint behavior exactly, upgrade the document to OpenAPI 3.1, decompose the inline `components.schemas` into shared per-schema files under the slice's `contracts/schemas/`, and bind the HTTP document to them by `$ref`. Do not invent surface the source does not carry.
