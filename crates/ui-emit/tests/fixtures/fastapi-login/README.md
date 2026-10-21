# fastapi-login design system

The ui-ir design system of the full-stack FastAPI template's login page
(fastapi/full-stack-fastapi-template, MIT), extracted by ui-ir's `web-to-ir`
(`augentic/ui-ir-experiment`, branch `experiment/behaviour` at `c5decdf`) from the
committed capture `captures/fastapi-template/login`:

```sh
web-to-ir extract captures/fastapi-template/login <out>/design-system --screen login
```

`composition.json`, `tokens.json`, `assets.json` and the four SVGs are copied
as written; `ext/web.json` and `audit.json` are left out, since the emit reads
neither. The composition carries 11 `TODO(...)` holes, so the golden is emitted
with holes allowed, as `vx emit --allow-holes` does.

The golden under `../../golden/fastapi-login/` is byte-identical to what
`vx emit design-system --shell vue --allow-holes` (vx `feat/ui-ir-integration`)
writes for the same input.
