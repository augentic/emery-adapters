# Outbound HTTP calls

Every HTTP call the surface makes is a `call` claim at the call site, and the behaviour a caller observes through it — the timeout, the retry, the error mapping — is a `requirement`. Best-effort and audit calls count: fire-and-forget is not unspecified.

## Read from the call site

- **URL** as constructed: the base (`process.env.API_URL`, a config field) and the path and query as the code builds them — the exact template, no parameter added or dropped.
- **Method** and **headers**, each header with where its value comes from: a literal, a config key, a token.
- **Request body** shape, from the object the code serialises; the type it is built from is a `type` claim.
- **Response** shape, from how the code deserialises and reads it (`await response.json()` assigned to `string[]`; `body.items[0].id`), never from a broader declared interface.
- **Authentication**: bearer, API key, basic; and where the identity comes from — a config key (`AZURE_IDENTITY`) or a literal — because renaming or hardcoding it changes behaviour.
- **Status handling**: which statuses count as success, which are mapped to what error or default, whether a non-2xx throws or returns.
- **Retries and timeouts**: attempt counts, backoff values, and timeout durations, as the source spells them.

## Shape the claims

- `call` — `path` at the call site, `callee` the client function: a global bare (`fetch`), a package function as `<package>:<symbol>` (`axios:post`, `@azure/identity:getAzureToken`), a function or method of the tree as `<file>:<symbol>` (`src/lib/http.ts:HttpClient.post`); the synopsis names the method and the URL template.
- `requirement` — one per observable behaviour: `Enrichment posts the message to ${API_URL}/data with a 5-second timeout and retries twice on a network error.`
- `type` — the request and response shapes as the code declares or reads them.
- Say what is not in the source rather than inventing it: a response shape the code never reads, an auth method behind a client the tree does not contain.
