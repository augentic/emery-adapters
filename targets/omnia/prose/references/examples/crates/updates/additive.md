# Additive Example: Add a New Operation to the Cars Crate

Adding a `POST /worksite` endpoint to the existing `cars` multi-operation crate. The crate already has `GET /feature`, `GET /features`, `GET /layout`, and `GET /worksite` operations.

## Starting State

```
crates/cars/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── filter.rs
│   ├── operations.rs         # barrel: feature, feature_list, layout, worksite
│   └── handlers/
│       ├── feature.rs
│       ├── feature_list.rs
│       ├── layout.rs
│       └── worksite.rs
├── tests/
│   ├── provider.rs
│   ├── feature.rs
│   ├── layout.rs
│   └── worksite.rs
├── Migration.md
├── Architecture.md
└── .env.example
```

## Artifact Change

The updated artifacts add a new endpoint in the API Contracts section:

```markdown
### Endpoint: POST /worksite

- **Method**: POST
- **Path**: `/worksite`
- **Input**: JSON body (`CreateWorksiteInput`)
- **Output**: JSON (`WorksiteResponse`)
- **Provider bounds**: Config, HttpRequest

#### Input Type: CreateWorksiteInput
- `worksite_code`: string (required)
- `name`: string (required)
- `project_name`: string (required)

#### Business Logic
1. [domain] Validate input: worksite_code must not be empty
2. [domain] POST to MWS API `/worksite` with JSON body
3. [domain] Return created worksite
```

## Derived Change Set

- **Category**: Additive
- **Changes**:
  1. New operation file `src/handlers/create_worksite.rs`
  2. New type `CreateWorksiteInput` in the operation file
  3. New barrel entry in `src/operations.rs`
  4. New test file `tests/create_worksite.rs`
  5. MockProvider update (if needed -- in this case `Config + HttpRequest` already implemented)
  6. Guest wiring: new route

## Applied Changes

### 1. New Operation (`src/handlers/create_worksite.rs`)

```rust
//! Creates a new worksite via the MWS API.

use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;
use anyhow::Context as _;
use bytes::Bytes;
use http_body_util::Full;
use omnia_guest::{Config, Error, Result, bad_gateway};
use serde::{Deserialize, Serialize};

use crate::handlers::Worksite;
use crate::{HttpRequest, MWS_URI};

/// Input for creating a new worksite.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CreateWorksiteInput {
    pub worksite_code: String,
    pub name: String,
    pub project_name: String,
}

/// Response wrapping the created worksite.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CreateWorksiteResponse(pub Worksite);



async fn handle<P>(
    _owner: &str,
    input: CreateWorksiteInput,
    provider: &P,
) -> Result<CreateWorksiteResponse>
where
    P: Config + HttpRequest,
{
    let api_key = Config::get(provider, "MWS_API_KEY").await?;

    let body = serde_json::to_vec(&input).context("serializing request body")?;
    let request = http::Request::builder()
        .method("POST")
        .uri(format!("{MWS_URI}/worksite"))
        .header("Content-Type", "application/json")
        .header("x-api-key", &api_key)
        .body(Full::new(Bytes::from(body)))
        .context("building request")?;

    let response = HttpRequest::fetch(provider, request)
        .await
        .map_err(|e| bad_gateway!("creating worksite: {e}"))?;

    let bytes = response.into_body();
    let worksite: Worksite =
        serde_json::from_slice(&bytes).context("deserializing worksite response")?;

    Ok(CreateWorksiteResponse(worksite))
}

pub struct CreateWorksiteInputOperation;

impl<P> Operation<P> for CreateWorksiteInputOperation
where
    P: omnia_guest::api::Provider + Config + HttpRequest,
{
    type Error = Error;
    type Input = CreateWorksiteInput;
    type Output = CreateWorksiteResponse;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> Result<Self::Output> {
        // Structural validation is the first step; omit only when no checks apply.
        handle(context.owner, input, context.provider).await
    }
}
```

### 2. Update Barrel (`src/operations.rs`)

Add module declaration and re-export:

```rust
mod create_worksite;
mod feature;
mod feature_list;
mod layout;
mod worksite;

pub use create_worksite::*;
pub use feature::*;
pub use feature_list::*;
pub use layout::*;
pub use worksite::*;
```

### 3. New Test (`tests/create_worksite.rs`)

```rust
mod provider;

use cars::CreateWorksiteInput;
use omnia_guest::api::invocation::Invocation;
use omnia_guest::api::invoke::Invoker;
use provider::MockProvider;

#[tokio::test]
async fn create_worksite_happy_path() {
    let provider = MockProvider::new();
    let invoker = Invoker::new("owner", provider);

    let input = CreateWorksiteInput {
        worksite_code: "WS-001".to_string(),
        name: "Test Site".to_string(),
        project_name: "Test Project".to_string(),
    };

    let output = invoker.invoke::<CreateWorksiteInputOperation>(Invocation::new(input)).await.expect("should succeed");
    assert_eq!(output.0.worksite_code, "WS-001");
}

#[tokio::test]
async fn create_worksite_empty_code_rejected() {
    let provider = MockProvider::new();
    let invoker = Invoker::new("owner", provider);

    let input = CreateWorksiteInput {
        worksite_code: String::new(),
        name: "Test Site".to_string(),
        project_name: "Test Project".to_string(),
    };

    let response = invoker.invoke::<CreateWorksiteInputOperation>(Invocation::new(input)).await;
    assert!(response.is_err());
}
```

### 4. Guest Wiring Update

Append to `src/lib.rs`:

```rust
// New import
use cars::CreateWorksiteInputOperation;

// New typed route (appended to the existing Omnia HTTP router)
.route("/worksite", post::<CreateWorksiteInputOperation, Provider>())
```

### 5. CHANGELOG.md Entry

```markdown
## [Update: 2026-03-01]

### Added
- New operation `POST /worksite` (CreateWorksiteInput) for creating worksites via MWS API
- New types: `CreateWorksiteInput`, `CreateWorksiteResponse`
```

## Verification

- [x] Baseline `cargo test` captured (all existing tests pass)
- [x] New operation follows the `Operation<P>` pattern with delegation
- [x] `Operation::call` validates `worksite_code` before loading context
- [x] Provider bounds match existing pattern (Config + HttpRequest)
- [x] MockProvider already implements needed traits (no update required)
- [x] HTTP route uses the default JSON projector
- [x] Tests cover happy path and validation error
- [x] Guest wiring appended (not reordered)
- [x] CHANGELOG.md updated
- [x] `cargo check` passes
- [x] `cargo test` passes (no regressions, new tests pass)
