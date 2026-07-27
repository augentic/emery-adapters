# Spec-to-Test Mapping (Omnia)

Omnia-specific deltas on top of the shared [spec-to-test mapping discipline](emery-runtime/spec-to-test-mapping.md). The shared base owns the target-neutral rules: scenario → one test function, the `test_<unit_snake>_<scenario_snake>` naming convention, REQ-ID traceability comments, requirement coverage (N scenarios → N tests), and drift-detection mechanics. This file carries only what is specific to Omnia services.

## Test location and attribute

Each spec file maps to a primary test file in a separate `tests/` directory, named as the snake_case of the spec directory:

```text
specs/worksite/spec.md  →  tests/worksite.rs
specs/order/spec.md     →  tests/order.rs
```

Omnia tests are async and run on the Tokio runtime:

```text
#### Scenario: Successful worksite retrieval
  →  #[tokio::test] async fn test_worksite_successful_retrieval()
```

## WHEN clause to test setup

The WHEN clause determines typed operation input construction against the invoker and its mock provider:

| WHEN Pattern | Test Setup |
| --- | --- |
| WHEN user sends valid request with field X = Y | `let input = MyInput { x: "Y".to_string(), .. };` |
| WHEN request is missing required field | `let input = MyInput { field: "".to_string(), .. };` |
| WHEN external API returns error | Configure MockProvider to return error for that path |
| WHEN message arrives on topic T | `let message = build_message(/* topic T payload */);` |

## THEN clause to assertions

The THEN clause determines test assertions against the response, provider, and state store:

| THEN Pattern | Assertion |
| --- | --- |
| THEN operation succeeds with data | Assert fields on the plain operation output |
| THEN system returns error CODE | `let err = invoker.invoke::<MyOperation>(Invocation::new(input)).await.expect_err("...");` + `assert_eq!(err.code(), "CODE");` |
| THEN system publishes event to topic T | `let events = provider.events();` + topic and payload assertions |
| THEN system caches result for N seconds | Assert StateStore was called with expected TTL |
| THEN system calls external API at path P | `let calls = provider.requests_for("P");` + `assert_eq!(calls.len(), 1);` |

## Worked example — validation requirement

Validation requirements construct invalid typed input and assert the resulting error through `MockProvider` + `Invoker`:

```markdown
### Requirement: Input validation
ID: REQ-002
#### Scenario: Missing worksite code
- WHEN request has empty worksite_code
- THEN system returns BadRequest with code "missing_worksite_code"
```

```rust
/// Spec: specs/fleet-api/spec.md > REQ-002 > Scenario: Missing worksite code
#[tokio::test]
async fn test_fleet_api_missing_worksite_code() {
    let provider = MockProvider::new();
    let invoker = Invoker::new("owner", provider.clone());

    let request = GetWorksiteRequest { worksite_code: "".to_string(), .. };
    let error = invoker
        .invoke::<GetWorksite>(Invocation::new(request))
        .await
        .expect_err("should reject empty code");
    assert_eq!(error.code(), "missing_worksite_code");
}
```

## Drift detection (Omnia specifics)

The drift-detection mechanics live in the [shared base](emery-runtime/spec-to-test-mapping.md#drift-detection-mechanics). For Omnia, the test side of the comparison parses `#[tokio::test]` functions (and their `/// Spec:` comments) from `tests/*.rs`; assertion-drift comparison focuses on HTTP status codes, error codes, published-event topics, and cache TTLs.
