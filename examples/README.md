# Adapter Examples

Sample `emery.toml` configurations for running the first-party source and target adapters in this workspace against a sibling [`emery`](https://github.com/augentic/emery) checkout.

Run all commands from the `emery-adapters` repository root (which `emery` mounts as `.`).

## Configurations

| Config | Adapters | Fixture / Input |
| --- | --- | --- |
| [`emery.toml`](emery.toml) | `emery:typescript`, `emery:omnia` | `evals/cases/train/fixture/` |
| [`typescript/emery.toml`](typescript/emery.toml) | `emery:typescript` | `evals/cases/train/fixture/` |
| [`python/emery.toml`](python/emery.toml) | `emery:python` | `evals/cases/fastapi-routers/fixture/` |
| [`documentation/emery.toml`](documentation/emery.toml) | `emery:documentation` | [`documentation/docs/orders-api.md`](documentation/docs/orders-api.md) |
| [`intent/emery.toml`](intent/emery.toml) | `emery:intent` | Inline `description` |
| [`amend/emery.toml`](amend/emery.toml) | `emery:typescript` (`scope = "whole"`), `emery:omnia` | `evals/cases/orders/fixture/` |

## Prerequisites and setup

Requires a sibling `../emery` checkout, [`cursor-sdk-bridge`](https://github.com/cursor/sdk-bridge) on `PATH`, and `CURSOR_API_KEY` in `.env` or the environment (see [Your first specification and build](https://github.com/augentic/emery/blob/main/docs/tutorials/quickstart.md#prerequisites)).

Build the workspace's `wasm32-wasip2` components and copy them into `~/.emery/adapters`:

```bash
cargo build --workspace --target wasm32-wasip2 --release

version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml)"
mkdir -p ~/.emery/adapters
for wasm in target/wasm32-wasip2/release/emery_*.wasm; do
  cp "$wasm" ~/.emery/adapters/"$(basename "$wasm" .wasm)@${version}.wasm"
done

set -a; source .env; set +a
```

## Specify, apply, and build

```bash
# Extract claims and stage a candidate revision (swap --config for any file in the table above)
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/emery.toml
cargo run --manifest-path ../emery/Cargo.toml -- show spec --staged

# Seal the candidate into .emery/vcs/repos/omnia on branch emery/<revision>
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/emery.toml
cargo run --manifest-path ../emery/Cargo.toml -- show spec --config examples/emery.toml

# Build the plan through emery:omnia and inspect the resulting branch
cargo run --manifest-path ../emery/Cargo.toml -- build -v --config examples/emery.toml
git -C .emery/vcs/repos/omnia branch --list 'emery/*'
git -C .emery/vcs/repos/omnia checkout emery/<revision>
```

## Amend

[`amend/emery.toml`](amend/emery.toml) pairs `emery:typescript` (`scope = "whole"`) over `evals/cases/orders/fixture/` with `emery:omnia`:

```bash
# Seal the baseline revision from the whole fixture (--fresh if a lineage already exists)
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --apply --config examples/amend/emery.toml

# Remove `cancel` (lines 67–75) from OrderService so only `place` and `read` remain
python3 -c '
from pathlib import Path
p = Path("evals/cases/orders/fixture/orders.ts")
lines = p.read_text().splitlines(keepends=True)
p.write_text("".join(lines[:66] + lines[75:]))
'

# Stage and seal the amendment (`scope = "whole"` retires untouched requirements under `order-service`)
cargo run --manifest-path ../emery/Cargo.toml -- amend -v --config examples/amend/emery.toml
cargo run --manifest-path ../emery/Cargo.toml -- show amendment
cargo run --manifest-path ../emery/Cargo.toml -- apply --config examples/amend/emery.toml

# Restore the fixture
git checkout -- evals/cases/orders/fixture/orders.ts
```

## Further reading

- **Graded live evaluation harness:** [`evals/README.md`](../evals/README.md)
- **Emery documentation:** [`docs/index.md`](https://github.com/augentic/emery/blob/main/docs/index.md) ([Tutorials](https://github.com/augentic/emery/blob/main/docs/tutorials/quickstart.md), [Retiring requirements by scope](https://github.com/augentic/emery/blob/main/docs/how-to/retire-requirements.md), [Authoring and debugging adapters](https://github.com/augentic/emery/blob/main/docs/how-to/author-an-adapter.md), [Deployment profiles](https://github.com/augentic/emery/blob/main/docs/reference/deployment-profiles.md))
