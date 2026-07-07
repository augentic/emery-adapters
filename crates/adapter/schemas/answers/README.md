# Vendored judgment answer schemas

These documents are **vendored copies** of the generated answer schemas published by [augentic/specify](https://github.com/augentic/specify) under `crates/adapter/schemas/answers/`. Each is the `format: schema(...)` payload an adapter guest sends with a judgment `omnia:model/completion.create` call, derived there from the canonical embedded schemas (never hand-written — regeneration lives in the specify repo's `specify-schema` crate):

| Schema                 | Answer for                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------ |
| `leads.schema.json`    | source `survey` — `{ leads: [...] }`                                                       |
| `evidence.schema.json` | source `extract` — Evidence minus the envelope `lead`                                      |
| `report.schema.json`   | target `build` / `merge` — report minus `version` / `slice` / `target`, diagnostic inlined |

This copy is a temporary pin: once the `specify:adapter` package distribution carries the answer schemas (the WIT itself already flipped to a published-pin consume — see [`wit/README.md`](../../../../wit/README.md)), this directory is deleted. Until then, keep it byte-identical to upstream — `cargo make check-pins` compares against a sibling `../specify` checkout when one is present. Never edit these files here; change the canonical schema in the specify repo and re-vendor.
