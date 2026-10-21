# code-to-behaviour

Reads a React + TypeScript source tree with oxc and writes ui-ir
`behaviour.json`: routes, triggers, guards, state, validation (transitions
stay empty). Deterministic and model-free. Every entry carries `file#L`
evidence. A fact is written only when a row of the hand-written mapping
table (`ui-ir-staging/vocab/behaviour-mappings.json`) or one of the fixed
language rules below says how to read it. Everything else is listed in
`behaviour.unmapped.json` (`unmapped`: a source key with no row;
`unconverted`: a row exists but the value is not a literal).

```
code-to-behaviour --source <root> --entry <file> --screen <slug> --out <dir> [--alias <prefix>=<dir>]...
code-to-behaviour evidence <behaviour.json>
```

`evidence` prints the document as Emery `Evidence` (`emery_sdk`): a `call`
claim per request (`callee` extra, synopsis `METHOD url`) and an `example`
claim per observation that confirmed something (`replay-digest` extra), each
at its `file#L`. It exits 1 when `Evidence::findings` is not empty. This is
the shape an Emery adapter would join after its model turns; this crate is
not an adapter itself.

## How it reads the code

- Modules: the entry and everything reachable through local imports
  (relative, and aliases, default `@/` to `src/`), with `export *` and
  re-exports followed. Package imports become keys like
  `@tanstack/react-router#Link`.
- Components: the entry module's components, then every local component
  used as a JSX element, once each (the render tree).
- Values: a small evaluator over the owned tree in `src/syntax.rs`:
  bindings, destructuring, object and array literals, local function
  returns, `.bind(this)`, class static methods, `React.forwardRef` /
  `memo` (rows with converter `wrapper`), `.map` over a literal array.
- Effects: from a handler, following local calls. `useMutation` places the
  effects after the request into `on_success`, `onError` into `on_failure`.
  `form.handleSubmit(h)` is `validate` then `h`. An early `if (...) return`
  becomes a trigger `guard`. A request is a call whose callee member is
  `get/post/put/patch/delete` with a literal `url`; the call is named after
  the enclosing function (`src/client/sdk.gen.ts:LoginService.loginAccessToken`).
- Match tuple: role (row, or `type` for inputs), visible text (static JSX
  text, as `textContent` would read it; unknown when a child is a local
  component or a dynamic value), href, form-field `name`, and whether a
  library overlay content element is a lexical ancestor. When the text is
  one of a few literal alternatives (`{show ? 'Hide' : 'Show'}`, at most 8),
  `text_in` lists them.
- Routes: file routes drop `(group)` and `_pathless` segments from the URL;
  the route's `component` option is read as a component.
- Validation: zod messages are taken from the message argument or
  `{ message }` / `{ error }` when literal (else `null`); native
  `required`, `minLength`, `maxLength`, `pattern` and `type=email|url` on
  inputs become rules with converter `constraint` and message `null` (the
  browser shows its own text).

The trigger id is `t-` + the first 12 hex of sha256 of the element's
`file#L`, the `.map` index path, and the gesture.

