# my-lists Specification

## Purpose

My Lists overview with a create-list floating action button. FAB presence is evidenced; FAB activation outcome is intentionally unspecified.

### Requirement: My Lists View

ID: REQ-021
Sources: [intent]
Status: agreed

The My Lists screen presents a scrollable body listing task lists and a floating action button overlay.

#### Scenario: Layout regions

- **WHEN** the user views the My Lists screen
- **THEN** the layout includes a list body and a floating action button region

### Requirement: List row display

ID: REQ-022
Sources: [intent]
Status: agreed

Each list row displays the list name and an item count.

#### Scenario: Row contents

- **WHEN** a list row is rendered on My Lists
- **THEN** the row shows the list name and item count

### Requirement: Create list floating action button

ID: REQ-024
Sources: [intent]
Status: agreed

My Lists exposes a floating action button as the primary creation action for new lists.

#### Scenario: FAB visible

- **WHEN** the user views My Lists
- **THEN** a floating action button is visible for primary list creation

### Requirement: FAB activation behaviour

ID: REQ-026
Sources: [intent]
Status: agreed

A floating action button is present on My Lists; the action triggered when the user activates it is not evidenced.

#### Scenario: WHEN the user activates the My Lists floating action button THEN the resulting navigation or state change is unspecified — operator must supply acceptance criteria
