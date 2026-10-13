# Example: outbound HTTP

## Scenario

A library with no bootstrap, so its entry's exports are its surfaces. The message names this one as `` Surface `processMessage` — entry `src/handler.ts` — stem `message-processing`: exported function L12–L21; id `message-processing`; reaches nothing beyond its entry ``, lists one boundary — `` `src/handler.ts#L13` — `process.env.API_URL` in `const apiUrl = process.env.API_URL;` `` — and no package; `src/handler.ts` is all the surface reaches, so it is the one module laid out.

## Source

`src/handler.ts`:

```typescript
export interface Message {
  id: string;
  content: string;
  timestamp: number;
}

export interface Output {
  status: string;
  data: unknown;
}

export async function processMessage(message: Message): Promise<Output> {
  const apiUrl = process.env.API_URL;
  const response = await fetch(`${apiUrl}/data`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(message),
  });
  const data = await response.json();
  return { status: "processed", data };
}
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "message-processing.forwarding",
      "path": "src/handler.ts#L12-L21",
      "statement": "Processing a message posts it as JSON to ${API_URL}/data and returns status \"processed\" with the response body parsed as JSON."
    },
    {
      "kind": "requirement",
      "id": "message-processing.target-config",
      "path": "src/handler.ts#L13",
      "statement": "The forwarding target is read from the API_URL environment variable on every call, with no default."
    },
    {
      "kind": "requirement",
      "id": "message-processing.failure",
      "path": "src/handler.ts#L14-L19",
      "statement": "A network failure or a response body that is not JSON propagates to the caller as an error; the response status is not checked and nothing is retried."
    },
    {
      "kind": "excerpt",
      "path": "src/handler.ts#L13-L20",
      "excerpt": "Reads API_URL, POSTs the message as the JSON body with Content-Type application/json, parses the response as JSON without consulting response.ok, and returns { status: \"processed\", data }."
    },
    {
      "kind": "call",
      "path": "src/handler.ts#L14",
      "callee": "fetch",
      "synopsis": "POST ${API_URL}/data with the message as the JSON body"
    }
  ]
}
```

## What to notice

- `Message` and `Output` are no claims of the answer: the caller copies both exported declarations from L1–L10. The response's shape is not a gap to fill either: the code reads nothing from it (`data: unknown`), and the engine renders that.
- `API_URL` is the one Boundary and is no criterion: no requirement rests on a value it spells — there is no `??`, so "no default" is read from the code and stated in `target-config`.
- No retry and no status check are stated as the behaviour the code exhibits, not left out as omissions.
- `fetch` is a global, so its `callee` is bare; a function of the tree would be `<file>:<symbol>`.
- Every `requirement` id leads with `message-processing`, the id the surface line gives, and every `path` anchors the span that exhibits the behaviour.
