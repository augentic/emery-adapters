# Composition validation checklist

Authoring-time checklist for `composition.yaml`, applied by the build's composition leg before the in-guest validator gate runs. The deterministic validator enforces the schema and wiring rules; this list is the self-review that keeps the repair loop short.

- `composition.yaml` conforms to the Vectis composition JSON Schema
- Screen slugs are kebab-case
- Every per-page view struct field has a `bind` on some item
- Every shell-facing Event has an `event` wiring
- `maps_to` values reference declared ViewModel variants from the design
- Overlay `trigger` values match an `event` name in the same screen
- `Navigate(X)` targets have corresponding screen slugs and Route variants

## See also

- [Component Catalog](./components.md) — shared component factoring and `components.yaml` validation surfaces.
- [Vectis runtime schemas](./schemas.md) — retrieving the composition schema body.
