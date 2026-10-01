# Worked examples

Each example is one surface as the message names it, the module the extract call reads, and the Evidence the call answers with — claims alone, in the shape of the prompt's worked example, with no `type` among them: the caller copies the public declarations. Read the one nearest the surface in hand.

| File                                           | Read when the surface has                                                                                                                      |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| [outbound-http.md](outbound-http.md)           | A function forwarding over `requests`: the URL as constructed, the config key verbatim, a response the code never reads left unclaimed. Start here. |
| [branching-caching.md](branching-caching.md)   | Validation guards, a cache-aside flow with early returns, one explicit boundary as a `criterion`, a `try` whose extent decides what a failure becomes. |
| [parallel-execution.md](parallel-execution.md) | A token, two parallel lookups under `asyncio.gather`, two publishes 5 seconds apart, and an `except` that re-raises one error code and swallows the rest. |
