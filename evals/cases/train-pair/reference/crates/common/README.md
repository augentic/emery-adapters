# Common

Shared logic for the train domain, trimmed to what the SOAP connector and
position adapter read:

- `config` — the configuration key catalog plus `env()` / `topic()` helpers
  with a single documented resolution policy.
- `routes` — the canonical HTTP path and messaging topic tables consumed by
  both the guest and the domain crates.
- `block_mgt` — the Block Management API client, retrieving the vehicles
  allocated to a trip.
- `TIMEZONE` — the operator's IANA zone (`Pacific/Auckland`), used wherever
  a service date or local time is derived.

The API client is written against the `omnia-sdk` capability traits
(`Config`, `HttpRequest`, `Identity`) so the same code runs inside the WASM
guest and against native `omnia_test::guest::Provider` doubles in tests.
