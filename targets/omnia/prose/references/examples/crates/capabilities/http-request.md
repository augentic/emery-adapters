# Example HTTP Operations

This document combines the example HTTP operation implementation from `crates/ex-http/src/` in the augentic/context repository.

**Demonstrates:** `HttpRequest` and `Config` capability traits

## lib.rs

```rust
//! Operations and provider for the HTTP example.
//!
//! This crate is defined separately to the core example so it can be tested.
//! Tests cannot run under the `wasm32-wasip2` target, so this allows us to
//! use configuration flags for this target in the main example crate.
mod handlers;

pub use handlers::*;
```

## handlers.rs

```rust
//! HTTP request operations demonstrating the operation pattern.
//!
//! Operations are domain-layer business logic that:
//! - Are WASM-agnostic (can run in native or WASM)
//! - Depend on provider traits, not concrete implementations
//! - Use strongly typed request/response types
//! - Implement the `Operation<P>` trait for uniform invocation

use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;
use anyhow::Context as _;
use percent_encoding::percent_decode_str;
use omnia_guest::{Config, Error, Result, bad_request};
use serde::{Deserialize, Serialize};

/// Example of a strongly typed request, expected to be serialized as query
/// parameters of an HTTP GET request.
///
/// Axum extracts this from URL query string: `/?a=hello&b=world`
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EchoRequest {
    pub a: String,
    pub b: String,
}

/// Response from an operation for an `EchoRequest`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EchoResponse {
    pub a: String,
    pub b: String,
}

/// An example operation that takes a strongly typed request and returns a
/// strongly typed response.
///
/// This operation demonstrates:
/// - URL percent-decoding of query parameters
/// - Error handling with descriptive messages
/// - Minimal provider dependencies (no Config needed)
///
/// # Errors
/// * Returns `bad_request` if URL decoding fails for query parameters.
#[allow(clippy::unused_async)]
async fn echo(_owner: &str, _provider: &impl Config, req: EchoRequest) -> Result<EchoResponse> {
    let EchoRequest { a, b } = req;

    // Helper to decode percent-encoded query parameters
    let decode = |value: String, field: &str| -> Result<String> {
        percent_decode_str(&value)
            .decode_utf8()
            .map(std::borrow::Cow::into_owned)
            .map_err(|err| bad_request!("failed to decode '{field}': {err}"))
    };

    Ok(EchoResponse { a: decode(a, "a")?, b: decode(b, "b")? })
}

/// Common operation implementation for a consistent API.
///
/// This trait implementation allows the operation to be invoked uniformly via
/// `Invoker::invoke()` regardless of the specific request/response types.
pub struct EchoRequestOperation;

impl<P: omnia_guest::api::Provider + Config> Operation<P> for EchoRequestOperation
{
    type Error = Error;
    type Input = EchoRequest;
    type Output = EchoResponse;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> Result<Self::Output> {
        // Structural validation is the first step; omit only when no checks apply.
        echo(context.owner, context.provider, input).await
    }
}

/// Example of a strongly typed request, expected to be serialized as the body
/// of an HTTP request.
///
/// Axum extracts this from JSON request body via `Json<GreetingRequest>`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GreetingRequest {
    pub message: String,
}

/// Example of a strongly typed response, expected to be serialized as the body
/// of an HTTP response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GreetingResponse {
    /// Name of the respondent (fetched from configuration)
    pub respondent: String,
    /// Echo of the original message
    pub reply: String,
}

/// An example operation that takes a strongly typed request and returns a
/// strongly typed response.
///
/// This operation demonstrates:
/// - Using the Config provider to fetch configuration values
/// - Composing response from both config and request data
///
/// There is a dependency on a provider that implements the `Config` trait for
/// configuration information.
///
/// # Errors
/// * The provider fails to retrieve the configuration value.
async fn greeting<P>(_owner: &str, provider: &P, req: GreetingRequest) -> Result<GreetingResponse>
where
    P: Config,
{
    // Fetch the respondent name from configuration
    let name = Config::get(provider, "name").await?;
    Ok(GreetingResponse { respondent: name, reply: req.message })
}

/// Common operation implementation for a consistent API.
pub struct GreetingRequestOperation;

impl<P: omnia_guest::api::Provider + Config> Operation<P> for GreetingRequestOperation
{
    type Error = Error;
    type Input = GreetingRequest;
    type Output = GreetingResponse;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> Result<Self::Output> {
        // Structural validation is the first step; omit only when no checks apply.
        greeting(context.owner, context.provider, input).await
    }
}
```

## Key Patterns Demonstrated

1. **Strongly Typed Requests/Responses**: Both `EchoRequest` and `GreetingRequest` are concrete types
2. **Operation Trait Implementation**: Both implement `Operation<P>` for uniform invocation
3. **Provider Dependencies**: Operations depend on provider traits (e.g., `Config`)
4. **Error Handling**: Using `anyhow::Context` and `omnia_guest::Error`
5. **Separation of Concerns**: Business logic is in separate functions (`echo`, `greeting`)
6. **WASM-Compatible**: No OS-specific dependencies, all async

## References

- See [../../references/sdk-api.md](../../../sdk-api.md) for the `Operation<P>` trait pattern
- See [../../references/capabilities.md](../../../capabilities.md) for trait definitions
- See [../../references/providers.md](../../../providers/README.md) for provider bound composition
