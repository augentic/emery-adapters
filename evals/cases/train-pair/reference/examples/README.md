# Runtime Example

Runs the `train` guest on the omnia runtime using the default (in-memory)
implementations of the WASI services it imports (`config`, `http`,
`identity`, `keyvalue`, `messaging`, `otel`).

## Quick Start

```bash
cargo build --package train --target wasm32-wasip2 --release

set -a && source .env && set +a
cargo run --features host --example runtime -- run ./target/wasm32-wasip2/release/train.wasm
```

## Test

Post the vendor's SOAP envelope to `POST /inbound/xml`;
`crates/soap-connector/data/receive-message.xml` is one such request. The
connector publishes the embedded train update to the `dev-realtime-train.v1`
topic of the runtime's in-memory broker, where the adapter consumes it.
To drive the adapter alone, publish a train update to that topic directly;
`crates/position-adapter/data/sample.xml` is one such message.
