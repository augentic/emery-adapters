# Example: branching and caching

## Scenario

A library with no bootstrap. The message names `` Surface `validate_and_process` — entry `src/data_service.py` — stem `data-lookup`: exported function L22–L45; id `data-lookup`; reaches `src/cache.py`, `src/api_client.py` `` and lays out the three modules the surface reaches — `src/data_service.py`, `src/cache.py`, `src/api_client.py`; the example shows the entry alone. Its Boundaries list holds one line for this module — `` `src/data_service.py#L9` — `ID_PATTERN = re.compile(r"^[A-Z0-9]{8}$")` `` — and the TTL is written into the calls that pass it.

## Source

`src/data_service.py`:

```python
import re
from dataclasses import dataclass
from typing import Any, Literal

from .api_client import fetch_from_api
from .cache import cache

ID_PATTERN = re.compile(r"^[A-Z0-9]{8}$")


@dataclass
class InputData:
    id: str
    force_refresh: bool = False


@dataclass
class Result:
    success: bool
    data: Any = None
    error: str | None = None
    source: Literal["cache", "api"] | None = None


def validate_and_process(data: InputData) -> Result:
    if not data.id:
        return Result(success=False, error="Invalid ID")
    if not ID_PATTERN.match(data.id):
        return Result(success=False, error="ID must be 8 alphanumeric characters")
    if data.force_refresh:
        fresh = fetch_from_api(data.id)
        cache.set(f"data:{data.id}", fresh, ttl=3600)
        return Result(success=True, data=fresh, source="api")
    cached = cache.get(f"data:{data.id}")
    if cached is not None:
        return Result(success=True, data=cached, source="cache")
    try:
        fresh = fetch_from_api(data.id)
        cache.set(f"data:{data.id}", fresh, ttl=3600)
        return Result(success=True, data=fresh, source="api")
    except Exception as error:
        return Result(success=False, error=f"API fetch failed: {error}")
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "data-lookup.id-required",
      "path": "src/data_service.py#L23-L24",
      "statement": "A lookup with a missing or empty id returns success False with error \"Invalid ID\" before anything is read."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.id-format",
      "path": "src/data_service.py#L25-L26",
      "statement": "A lookup whose id does not match ID_PATTERN returns success False with error \"ID must be 8 alphanumeric characters\"."
    },
    {
      "kind": "criterion",
      "id": "data-lookup.id-format.pattern",
      "path": "src/data_service.py#L9",
      "criterion": "The id matches ^[A-Z0-9]{8}$: exactly 8 upper-case letters or digits."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.force-refresh",
      "path": "src/data_service.py#L27-L30",
      "statement": "A lookup with force_refresh set fetches from the API without reading the cache, caches the result under data:{id} for 3600 seconds, and returns it with source \"api\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-hit",
      "path": "src/data_service.py#L31-L33",
      "statement": "A lookup that finds a value under data:{id} returns it with source \"cache\" without calling the API."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-miss",
      "path": "src/data_service.py#L34-L37",
      "statement": "On a cache miss the lookup fetches from the API, caches the result under data:{id} for 3600 seconds, and returns it with source \"api\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-miss-failure",
      "path": "src/data_service.py#L34-L39",
      "statement": "On the cache-miss path any exception from the API fetch or the cache write returns success False with error \"API fetch failed: <error>\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.force-refresh-failure",
      "path": "src/data_service.py#L27-L31",
      "statement": "An exception from the API fetch or the cache write on the force_refresh path, or from the cache read, propagates to the caller rather than becoming a Result."
    },
    {
      "kind": "excerpt",
      "path": "src/data_service.py#L23-L26",
      "excerpt": "Two guards before any I/O: an empty id, then an id failing ID_PATTERN, each returning a failure Result and ending the call."
    },
    {
      "kind": "excerpt",
      "path": "src/data_service.py#L27-L39",
      "excerpt": "Cache-aside over key data:{id} with ttl=3600: force_refresh skips the read and writes through; otherwise a hit returns source \"cache\" and a miss fetches, writes, and returns source \"api\". Only the miss path sits inside the try, so only its fetch and write become a failure Result; the except catches Exception, so every error class is reported the same way."
    },
    {
      "kind": "call",
      "path": "src/data_service.py#L28",
      "callee": "src/api_client.py:fetch_from_api",
      "synopsis": "fetch by id on the force_refresh path"
    },
    {
      "kind": "call",
      "path": "src/data_service.py#L29",
      "callee": "src/cache.py:cache.set",
      "synopsis": "write data:{id} with ttl 3600 on the force_refresh path"
    },
    {
      "kind": "call",
      "path": "src/data_service.py#L31",
      "callee": "src/cache.py:cache.get",
      "synopsis": "read data:{id}"
    },
    {
      "kind": "call",
      "path": "src/data_service.py#L35",
      "callee": "src/api_client.py:fetch_from_api",
      "synopsis": "fetch by id on a cache miss"
    },
    {
      "kind": "call",
      "path": "src/data_service.py#L36",
      "callee": "src/cache.py:cache.set",
      "synopsis": "write data:{id} with ttl 3600 on a cache miss"
    }
  ]
}
```

## What to notice

- Five paths through the function, one `requirement` each, then the two failure behaviours. The extent of the `try` decides which failures become a `Result` and which propagate, and the statements follow the code: a cache write failing inside the `try` is reported as `API fetch failed`.
- One `criterion`: `ID_PATTERN` is bound to a name at module level, so the message lists it as a Boundary, and the criterion sits at that anchor extending `data-lookup.id-format`. Written into the guard — `re.match(r"^[A-Z0-9]{8}$", data.id)` — it would be listed nowhere and the pattern would be the rule's value in the statement alone. The TTL and the key pattern are values inside the statements likewise: `3600` is passed in place, not bound to a name.
- `cache` and `fetch_from_api` live in other modules of the tree. They are `call` claims for what this surface does with them; the extract call follows them and claims what they reach, but never claims them as surfaces of their own.
- `InputData` and `Result` are no claims of the answer: the caller copies both dataclasses, `Result.source` as the `Literal` it spells. What the answer carries of them is the behaviour that turns on a field — `force_refresh` in the statements.
