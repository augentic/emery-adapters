# orders-api Specification

## Purpose

Capture the orders service's HTTP contract from its TypeScript source.

### Requirement: Source-derived contract

The contract MUST describe exactly the analysis-identified entry points (`POST /orders`, `GET /orders/:orderId`), with paths, methods, status codes, and payload shapes taken from the source under `vendor/orders-service/`. Wire-level details the source does not encode remain `[unknown]`.
