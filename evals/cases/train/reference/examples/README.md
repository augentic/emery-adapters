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

Publish a train update to the `dev-realtime-train.v1` topic of the
runtime's in-memory broker; `crates/position-adapter/data/sample.xml` is one
such message.
