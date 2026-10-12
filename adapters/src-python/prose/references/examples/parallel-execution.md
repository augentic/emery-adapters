# Example: parallel execution

## Scenario

A library with no bootstrap. The message names `` Surface `EventProcessor` — entry `services/event_processor/__init__.py` — stem `event-processing`: exported class L25–L58; methods `process` L29–L45; ids `event-processing`, `event-processing.process`; reaches `services/event_processor/publisher.py` ``, lists one boundary — `` `services/event_processor/__init__.py#L22` — `TENANT_ID = os.environ["AZURE_TENANT_ID"]` `` — one package — `` `azure.identity.aio` — `DefaultAzureCredential` — in `services/event_processor/__init__.py` `` — and one call through it — `` `azure.identity.aio:DefaultAzureCredential.get_token` in `services/event_processor/__init__.py` at L31 `` — and lays out the two modules the surface reaches; the example shows the entry alone, with the private method bodies elided.

## Source

`services/event_processor/__init__.py`:

```python
import asyncio
import logging
import os
import time
from dataclasses import dataclass
from typing import Any

from azure.identity.aio import DefaultAzureCredential

from .publisher import Publisher

log = logging.getLogger(__name__)


@dataclass
class EventInput:
    id: str
    type: str
    data: dict[str, Any]


TENANT_ID = os.environ["AZURE_TENANT_ID"]


class EventProcessor:
    def __init__(self, publisher: Publisher) -> None:
        self._publisher = publisher

    async def process(self, event: EventInput) -> None:
        try:
            token = await DefaultAzureCredential(tenant_id=TENANT_ID).get_token("https://graph.microsoft.com/.default")
            enriched, metadata = await asyncio.gather(
                self._enrich(event, token.token),
                self._fetch_metadata(event.id),
            )
            await self._publisher.send("events-topic", enriched)
            await asyncio.sleep(5)
            await self._publisher.send("audit-topic", {"timestamp": time.time(), "metadata": metadata})
        except Exception as error:
            if getattr(error, "code", None) == "AUTH_FAILED":
                raise RuntimeError("Authentication failed") from error
            log.error("Processing error: %s", error)

    async def _enrich(self, event: EventInput, token: str) -> dict[str, Any]:
        ...

    async def _fetch_metadata(self, event_id: str) -> dict[str, Any]:
        ...
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "event-processing.process.authentication",
      "path": "services/event_processor/__init__.py#L31",
      "statement": "Processing an event first acquires a Microsoft Graph token for the tenant named by AZURE_TENANT_ID."
    },
    {
      "kind": "requirement",
      "id": "event-processing.process.tenant-config",
      "path": "services/event_processor/__init__.py#L22",
      "statement": "The tenant id is read from AZURE_TENANT_ID once when the module loads, and a missing variable fails the import with KeyError."
    },
    {
      "kind": "requirement",
      "id": "event-processing.process.enrichment",
      "path": "services/event_processor/__init__.py#L32-L35",
      "statement": "Enrichment with the token and the metadata lookup by event id run in parallel, and publishing waits for both to complete."
    },
    {
      "kind": "requirement",
      "id": "event-processing.process.publication",
      "path": "services/event_processor/__init__.py#L36-L38",
      "statement": "The enriched event is published once to events-topic; 5 seconds later an audit record carrying the current time and the metadata is published once to audit-topic."
    },
    {
      "kind": "requirement",
      "id": "event-processing.process.auth-failure",
      "path": "services/event_processor/__init__.py#L40-L41",
      "statement": "An exception whose code attribute is AUTH_FAILED, from any step, is re-raised as RuntimeError(\"Authentication failed\") and nothing further is published."
    },
    {
      "kind": "requirement",
      "id": "event-processing.process.other-failure",
      "path": "services/event_processor/__init__.py#L42",
      "statement": "Any other exception is logged at error level as \"Processing error: <error>\" and processing returns normally, so an event may reach events-topic with no audit record following it."
    },
    {
      "kind": "excerpt",
      "path": "services/event_processor/__init__.py#L30-L42",
      "excerpt": "One try around the whole flow: token, asyncio.gather over _enrich and _fetch_metadata, publish to events-topic, asyncio.sleep(5), publish {\"timestamp\": time.time(), \"metadata\": metadata} to audit-topic. The except re-raises AUTH_FAILED alone and swallows everything else with log.error."
    },
    {
      "kind": "call",
      "path": "services/event_processor/__init__.py#L31",
      "callee": "azure.identity.aio:DefaultAzureCredential.get_token",
      "synopsis": "acquire a token for https://graph.microsoft.com/.default under TENANT_ID"
    },
    {
      "kind": "call",
      "path": "services/event_processor/__init__.py#L36",
      "callee": "services/event_processor/publisher.py:Publisher.send",
      "synopsis": "publish the enriched event to events-topic"
    },
    {
      "kind": "call",
      "path": "services/event_processor/__init__.py#L38",
      "callee": "services/event_processor/publisher.py:Publisher.send",
      "synopsis": "publish {timestamp, metadata} to audit-topic after a 5 second delay"
    }
  ]
}
```

## What to notice

- Parallelism is stated where it is observable: nothing is published until both lookups complete. The order of the two publishes and the delay between them are behaviour, with the value the source spells.
- One `try` covers the whole flow, so the two arms of the `except` are two requirements with different consequences, and the partial-publication consequence is stated because the code exhibits it.
- `EventInput` is no claim of the answer: the caller copies the dataclass. The audit payload is an inline dict with no declaration, so nothing copies it; its shape lives in the statement and the excerpt.
- Every id leads with `event-processing.process`, the id the surface line gives the one public method the behaviour sits in. `TENANT_ID` is the one Boundary and is no criterion: `tenant-config` states what the code does with it — reads once at import, fails without it — and no requirement rests on a value it spells.
- The private method bodies are elided here. In a real tree the extract call follows them, and the outbound calls enrichment makes are this surface's `call` claims; nothing is claimed about what they do until it is read.
- `DefaultAzureCredential` comes from a package, so its `callee` is `<package>:<Type>.<method>` as the Calls list spells it; `Publisher.send` is a method of the tree, so its `callee` is `<file>:<symbol>` — the list shows the call inside `publisher.py`, and the surface's site is where it is made from.
