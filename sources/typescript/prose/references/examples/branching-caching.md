# Example: branching and caching

## Scenario

The survey named one surface: the exported API `validateAndProcess`, entered at `src/data-service.ts`. The tree also holds `src/cache.ts` and `src/api-client.ts`, which the surface reaches; the example shows the entry file alone.

## Source

`src/data-service.ts`:

```typescript
import { cache } from "./cache";
import { fetchFromAPI } from "./api-client";

interface InputData {
  id: string;
  forceRefresh?: boolean;
}

interface Result {
  success: boolean;
  data?: object;
  error?: string;
  source?: "cache" | "api";
}

export async function validateAndProcess(data: InputData): Promise<Result> {
  if (!data.id || data.id.length === 0) {
    return { success: false, error: "Invalid ID" };
  }
  if (!/^[A-Z0-9]{8}$/.test(data.id)) {
    return { success: false, error: "ID must be 8 alphanumeric characters" };
  }
  if (data.forceRefresh) {
    const fresh = await fetchFromAPI(data.id);
    await cache.set(`data:${data.id}`, fresh, 3600);
    return { success: true, data: fresh, source: "api" };
  }
  const cached = await cache.get(`data:${data.id}`);
  if (cached) {
    return { success: true, data: cached, source: "cache" };
  }
  try {
    const fresh = await fetchFromAPI(data.id);
    await cache.set(`data:${data.id}`, fresh, 3600);
    return { success: true, data: fresh, source: "api" };
  } catch (error) {
    return { success: false, error: `API fetch failed: ${error.message}` };
  }
}
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "data-lookup.id-required",
      "path": "src/data-service.ts#L17-L19",
      "statement": "A lookup with a missing or empty id returns success false with error \"Invalid ID\" before anything is read."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.id-format",
      "path": "src/data-service.ts#L20-L22",
      "statement": "A lookup whose id is not exactly eight uppercase alphanumeric characters returns success false with error \"ID must be 8 alphanumeric characters\"."
    },
    {
      "kind": "criterion",
      "id": "data-lookup.id-format.pattern",
      "path": "src/data-service.ts#L20",
      "criterion": "The id matches ^[A-Z0-9]{8}$."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.force-refresh",
      "path": "src/data-service.ts#L23-L27",
      "statement": "A lookup with forceRefresh set fetches from the API without reading the cache, caches the result under data:{id} for 3600 seconds, and returns it with source \"api\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-hit",
      "path": "src/data-service.ts#L28-L31",
      "statement": "A lookup that finds a value under data:{id} returns it with source \"cache\" without calling the API."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-miss",
      "path": "src/data-service.ts#L32-L35",
      "statement": "On a cache miss the lookup fetches from the API, caches the result under data:{id} for 3600 seconds, and returns it with source \"api\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.cache-miss-failure",
      "path": "src/data-service.ts#L32-L38",
      "statement": "On the cache-miss path a failure of the API fetch or of the cache write returns success false with error \"API fetch failed: <message>\"."
    },
    {
      "kind": "requirement",
      "id": "data-lookup.force-refresh-failure",
      "path": "src/data-service.ts#L23-L28",
      "statement": "A failure of the API fetch or the cache write on the forceRefresh path, or of the cache read, propagates to the caller as an error rather than a result."
    },
    {
      "kind": "excerpt",
      "path": "src/data-service.ts#L17-L22",
      "excerpt": "Two guards before any I/O: an empty id, then an id failing ^[A-Z0-9]{8}$, each returning a failure Result and ending the call."
    },
    {
      "kind": "excerpt",
      "path": "src/data-service.ts#L23-L38",
      "excerpt": "Cache-aside over key data:{id} with TTL 3600: forceRefresh skips the read and writes through; otherwise a hit returns source \"cache\" and a miss fetches, writes, and returns source \"api\". Only the miss path sits inside the try, so only its fetch and write become a failure Result."
    },
    {
      "kind": "type",
      "path": "src/data-service.ts#L4-L7",
      "signature": "interface InputData { id: string; forceRefresh?: boolean }"
    },
    {
      "kind": "type",
      "path": "src/data-service.ts#L9-L14",
      "signature": "interface Result { success: boolean; data?: object; error?: string; source?: \"cache\" | \"api\" }"
    },
    {
      "kind": "call",
      "path": "src/data-service.ts#L24",
      "callee": "src/api-client.ts:fetchFromAPI",
      "synopsis": "fetch by id on the forceRefresh path"
    },
    {
      "kind": "call",
      "path": "src/data-service.ts#L25",
      "callee": "src/cache.ts:cache.set",
      "synopsis": "write data:{id} with TTL 3600 on the forceRefresh path"
    },
    {
      "kind": "call",
      "path": "src/data-service.ts#L28",
      "callee": "src/cache.ts:cache.get",
      "synopsis": "read data:{id}"
    },
    {
      "kind": "call",
      "path": "src/data-service.ts#L33",
      "callee": "src/api-client.ts:fetchFromAPI",
      "synopsis": "fetch by id on a cache miss"
    },
    {
      "kind": "call",
      "path": "src/data-service.ts#L34",
      "callee": "src/cache.ts:cache.set",
      "synopsis": "write data:{id} with TTL 3600 on a cache miss"
    }
  ]
}
```

## What to notice

- Five paths through the function, one `requirement` each, then the two failure behaviours. The extent of the `try` decides which failures become a `Result` and which propagate, and the statements follow the code: a cache write failing inside the `try` is reported as `API fetch failed`.
- The regex is the one explicit boundary the source encodes, so it is the one `criterion`, with an id extending `data-lookup.id-format`. The TTL and the key pattern are values inside the statements, not criteria.
- `cache` and `fetchFromAPI` live in other modules of the tree. They are `call` claims for what this surface does with them; the extract call follows them and claims what they reach, but never claims them as surfaces of their own.
- `Result.source` is carried as the union the declaration spells, not flattened to `string`.
