# daily-quote Specification

## Purpose

Single-screen Daily Quote feature: display one quote with attribution and refresh it on demand.

### Requirement: Daily Quote View

ID: REQ-001
Sources: [intent]
Status: agreed

The system SHALL render a single screen titled "Daily Quote" showing the current quote's text and its author attribution.

#### Scenario: Quote renders

- **WHEN** the user opens the app and a quote has been loaded
- **THEN** the screen titled "Daily Quote" renders the quote text and the author attribution beneath it

#### Scenario: Loading state

- **WHEN** the app is fetching a quote and none has been loaded yet
- **THEN** the screen renders a loading indicator in place of the quote text

### Requirement: Refresh Quote

ID: REQ-002
Sources: [intent]
Status: agreed

The system SHALL fetch a new quote when the user activates the refresh action, replacing the displayed quote on success.

#### Scenario: Refresh replaces the quote

- **WHEN** the user activates the refresh action and the fetch succeeds
- **THEN** the screen re-renders with the newly fetched quote text and attribution

#### Scenario: Refresh failure preserves the quote

- **WHEN** the user activates the refresh action and the fetch fails
- **THEN** the previously displayed quote remains and a "Could not refresh" message is surfaced
