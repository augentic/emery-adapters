# Example: outbound HTTP

## Scenario

A library with no bootstrap, so its entry's exports are its surfaces. The message names this one as `` Surface `process_message` — entry `src/handler.py` — stem `message-processing`: exported function L14–L22; id `message-processing`; reaches nothing beyond its entry ``, lists one boundary — `` `src/handler.py#L15` — env `API_URL` in `api_url = os.environ["API_URL"]` `` — one package — `` `requests` — `requests` — in `src/handler.py` `` — and one call through it — `` `requests:post` in `src/handler.py` at L16-L20 ``; `src/handler.py` is all the surface reaches, so it is the one module laid out.

## Source

`src/handler.py`:

```python
import os
from dataclasses import dataclass
from typing import Any

import requests


@dataclass
class Message:
    id: str
    content: str
    timestamp: int


def process_message(message: Message) -> dict[str, Any]:
    api_url = os.environ["API_URL"]
    response = requests.post(
        f"{api_url}/data",
        json={"id": message.id, "content": message.content, "timestamp": message.timestamp},
        headers={"Content-Type": "application/json"},
    )
    data = response.json()
    return {"status": "processed", "data": data}
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "message-processing.forwarding",
      "path": "src/handler.py#L14-L22",
      "statement": "Processing a message posts its id, content, and timestamp as JSON to {API_URL}/data and returns status \"processed\" with the response body parsed as JSON."
    },
    {
      "kind": "requirement",
      "id": "message-processing.target-config",
      "path": "src/handler.py#L15",
      "statement": "The forwarding target is read from the API_URL environment variable on every call, and a missing variable raises KeyError before anything is sent."
    },
    {
      "kind": "requirement",
      "id": "message-processing.failure",
      "path": "src/handler.py#L16-L21",
      "statement": "A connection failure or a response body that is not JSON propagates to the caller as an exception; the response status is not checked, no timeout is set, and nothing is retried."
    },
    {
      "kind": "excerpt",
      "path": "src/handler.py#L15-L22",
      "excerpt": "Reads API_URL with no default, POSTs the three message fields as the JSON body with Content-Type application/json and no timeout, parses the response as JSON without calling raise_for_status, and returns {\"status\": \"processed\", \"data\": data}."
    },
    {
      "kind": "call",
      "path": "src/handler.py#L16-L20",
      "callee": "requests:post",
      "synopsis": "POST {API_URL}/data with the message fields as the JSON body"
    }
  ]
}
```

## What to notice

- `Message` is no claim of the answer: the caller copies the dataclass from L8–L12. The response's shape is not a gap to fill either: the code reads nothing from it (`data` is returned as parsed), and the engine renders that.
- `API_URL` is the one Boundary and is no criterion: no requirement rests on a value it spells — there is no default, so "raises KeyError" is read from the subscript and stated in `target-config`.
- No retry, no timeout, and no status check are stated as the behaviour the code exhibits, not left out as omissions.
- `requests` is imported as a module, so its `callee` is `<package>:<symbol>`; a function of the tree would be `<file>:<symbol>`.
- Every `requirement` id leads with `message-processing`, the id the surface line gives, and every `path` anchors the span that exhibits the behaviour.
