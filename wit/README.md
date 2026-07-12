# Vendored `specify:adapter` WIT

[augentic/specify](https://github.com/augentic/specify) owns and publishes the adapter contract as the wasm-pkg package `specify:adapter`. This repo consumes it, using a vendored copy in `wit/specify.wit`.

## Updating

Install [wkg](https://github.com/bytecodealliance/wasm-pkg-tools).

```bash
wkg get specify:adapter@0.1.0 --config wit/config.toml --output ./wit/specify.wit
```

### `wkg` Registry

 [.wkg-config.toml](./.wkg-config.toml) maps the `specify:` namespace to `augentic.io`, whose `/.well-known/wasm-pkg/registry.json` resolves to the backing OCI registry.

See [Composing and Distributing](https://component-model.bytecodealliance.org/composing-and-distributing/distributing.html) for more information.