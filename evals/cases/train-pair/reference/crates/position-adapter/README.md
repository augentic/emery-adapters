# Train Position Adapter

Transforms train updates for specific stations into vehicle location
events. The transformed events are published to the
`{env}-realtime-train-positions.v1` topic for downstream processing.

Each event is published `PUBLISH_REPEATS` times because the downstream
schedule-adherence process relies on the repeat to detect departures — see
the note on `PUBLISH_REPEATS` in `src/handler.rs` before copying that
behaviour.

## Tests

- `tests/static.rs` — hand-authored scenarios exercising arrival/departure
  handling, unmapped stations, and validation failures.
- `tests/replay.rs` — snapshot fixtures captured from a live system, replayed
  through the adapter with the captured HTTP answers seeded into
  `omnia_test::guest::MatchedHttp` (`tests/fixture/` is the loader; each
  fixture names the exact request it answers, query string included).
