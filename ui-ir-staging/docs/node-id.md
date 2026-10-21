# Node id rule (staging, version 1)

Every composition node that comes from a web capture gets one id, and every
behaviour entry refers to nodes by that id. The id is a pure function of two
strings, so any adapter (Rust or JavaScript) computes the same value with no
shared state.

## Function

```
node_id(screen, path) = screen + "/n-" + hex16(sha256(utf8(path)))
```

- `screen`: the screen slug of the IR directory (`login`, `index`), matching
  `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `path`: the element's `path` in `computed.json`, byte for byte. It is the
  string ui-ir `tools/capture/src/collect.ts#L191-L199` builds: from the
  root down, each element's lower-case tag name, followed by
  `:nth-of-type(k)` (1-based among siblings of the same tag) only when the
  parent has more than one child of that tag; joined with `" > "` (space,
  greater-than, space). The root is `html`.
- `sha256`: SHA-256 over the UTF-8 bytes of `path`, no trailing newline.
- `hex16`: the first 16 characters of the lower-case hexadecimal digest.

Schema pattern: `^[a-z0-9]+(-[a-z0-9]+)*/n-[0-9a-f]{16}$`.

## Test vectors

| screen | path | node id |
|---|---|---|
| `login` | `html` | `login/n-e5e239560bc125a2` |
| `login` | `html > body > div > div > div:nth-of-type(2) > div:nth-of-type(1) > div > button` | `login/n-cf2331bab5423b71` |
| `index` | `html > body` | `index/n-b364c0b0dce72fd0` |

Both implementations test against this table:
`ui-ir-staging/crates/ui-ir-behaviour/src/lib.rs` (`node_id`) and
`tools/capture-to-behaviour/lib/node-id.mjs`.

## Notes

- The id is stable for as long as the DOM shape above the element is stable.
  An element moved to another parent, or a same-tag sibling inserted before
  it, gets a new id. That is intended: the id names a place in one capture.
- A recorder computes the same `path` in the live page with the same
  algorithm (`tools/capture-to-behaviour/lib/inventory.mjs`), so a node seen in
  a recording and the composition node from `computed.json` share an id when
  the run renders the same DOM at that step.
- Nodes that only exist after an interaction (an opened sheet, a menu) get ids
  from the recording; they have no composition node until a capture of that
  state exists. Behaviour keeps them (the run proves they exist).
