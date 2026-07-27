# ticket-api Specification

## Purpose

Import the vendor-supplied Ticket API OpenAPI document as this project's contract, preserving its endpoint behavior exactly.

### Requirement: Faithful import

The imported contracts MUST preserve the source endpoint surface and semantics of `vendor/ticket-api.openapi.yaml`, upgraded to OpenAPI 3.1 with the inline `components.schemas` decomposed into shared per-schema files bound by `$ref`. No surface the source does not carry may be invented.
