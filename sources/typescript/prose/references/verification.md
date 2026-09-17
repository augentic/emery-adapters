# Verification

Before answering, check the Evidence against the source — not against a picture of the finished document.

- [ ] Every behaviour a caller observes through this surface is a `requirement`: one present-tense sentence, an `id` led by this surface's domain noun, a `path` anchored at the span that exhibits it.
- [ ] Every branch, early return, error path, and side effect the caller can observe is stated — in its own requirement or in the one it belongs to.
- [ ] Every outbound call, store access, publish, and metric emission is a `call` at its call site, with the method and target the source spells.
- [ ] Every type the surface takes, returns, persists, or publishes is a `type` with its declaration verbatim, nested types included, wire names where decorators set them.
- [ ] Every config key, constant, duration, count, status code, topic, and key pattern is spelled as the source spells it, with its default where one exists.
- [ ] Response shapes are what the code deserialises and reads, not a broader declared interface.
- [ ] Nothing is claimed the code does not exhibit, and nothing is claimed for another surface: a shared module is claimed for what this surface does with it.
- [ ] Nothing comes from a test, a `.d.ts`, a dependency, build output, or the engine's own files.
- [ ] An `excerpt` is a paragraph of focused context, not a paste of the span; the anchor is the citation.
- [ ] Every `requirement` has a `statement`; every `criterion` has a `criterion` and an id extending its requirement's; every id is dotted kebab-case.
