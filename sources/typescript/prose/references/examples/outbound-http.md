# Example: outbound HTTP

## Scenario

The survey named one surface: the exported API `processMessage`, entered at `src/handler.ts`. The extract call is lent the tree and told the surface and its entry; the tree holds this one file.

## Source

`src/handler.ts`:

```typescript
interface Message {
  id: string;
  content: string;
  timestamp: number;
}

interface Output {
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
      "kind": "type",
      "path": "src/handler.ts#L1-L5",
      "signature": "interface Message { id: string; content: string; timestamp: number }"
    },
    {
      "kind": "type",
      "path": "src/handler.ts#L7-L10",
      "signature": "interface Output { status: string; data: unknown }"
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

- The response has no `type` claim: the code reads nothing from it (`data: unknown`), so the source holds no shape to carry. The engine renders that gap; the answer does not fill it.
- `API_URL` is spelled as the source spells it, and "no default" is read from the code — there is no `??`.
- No retry and no status check are stated as the behaviour the code exhibits, not left out as omissions.
- `fetch` is a global, so its `callee` is bare; a function of the tree would be `<file>:<symbol>`.
- Every `requirement` id leads with the surface's noun, `message-processing`, and every `path` anchors the span that exhibits the behaviour.
