# Proposal — loyalty-api-contract

Author API contracts for loyalty enrollment (authorship mode: generate from prose). This is the regression path of the `contracts-update-boundary` scenario: the same endpoint description that must *not* produce contract YAML inside an implementation slice is introduced here through a proper `contracts@1.0.0` slice.

Participants:

- loyalty-service: producer
- storefront: consumer

Scope: the single enrollment endpoint below, its request/response schemas, and a shared error shape. Contract YAML only, under the slice's `contracts/` delta.
