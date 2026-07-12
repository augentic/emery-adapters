# Omnia Operation API Reference

The generated domain surface is the transport-neutral operation kernel from `omnia_guest::api`. Transport adapters decode wire input, invoke an operation, and project its plain output or typed error.

## Stateless operation

```rust
use std::future::Future;
use omnia_guest::api::Provider;
use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;

pub trait Operation<P: Provider>: 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}
```

Use a zero-sized operation type and a typed input DTO. `Operation::call` is static: do not construct a stateful operation object.

```rust
use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;
pub struct CreateUser;

impl<P> Operation<P> for CreateUser
where
    P: omnia_guest::api::Provider + Config + DocumentStore,
{
    type Input = CreateUserInput;
    type Output = User;
    type Error = omnia_guest::Error;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> omnia_guest::Result<Self::Output> {
        validate_structural(&input)?;
        let settings = load_settings(context.provider).await?;
        validate_contextual(&input, &settings)?;
        create(context.owner, input, context.provider).await
    }
}
```

Preserve exact provider capability bounds. Structural checks are the first statements in `call`; checks requiring time, configuration, identity, or persisted state run only after that context is loaded.

## Invocation

`Invoker<P>` owns one provider and supplies owner plus transport-neutral metadata:

```rust
use omnia_guest::api::invocation::Invocation;
use omnia_guest::api::invoke::Invoker;

let invoker = Invoker::new("owner", provider);
let output = invoker
    .invoke::<CreateUser>(Invocation::new(input).metadata(metadata))
    .await?;
```

`CallContext` contains `owner`, `provider`, and `metadata`. Correlation IDs are transport metadata, not domain input fields.

## HTTP router and projectors

`omnia_guest::api::http::Router` binds typed operations. `get` decodes path/query fields into `O::Input`; `post` merges path fields into a JSON object and deserializes it into `O::Input`.

```rust
use omnia_guest::api::http::{Router, get, post};

Router::new(invoker)
    .route("/users/{user_id}", get::<GetUser, Provider>())
    .route("/users", post::<CreateUser, Provider>());
```

The default `Json` projector serializes plain outputs and maps operation errors through `HttpError`. Use `get_with` / `post_with` and implement `http::Projector<O, P>` when status, headers, non-JSON bytes, or a custom error envelope differ. Serialization belongs to the projector, never the domain output type.

## Messaging router and projectors

`omnia_guest::api::messaging::Router` routes exact topics to operations:

```rust
use omnia_guest::api::messaging::{Router, consume};

Router::new(invoker)
    .route("orders.created.v1", consume::<ProcessOrder>());
```

The default decoder deserializes JSON. The default `Acknowledge` projector acknowledges outputs and rejects decode or operation errors. Use `decode_with` and `project_with` for another payload format or retry/rejection policy. Missing and unhandled topics are errors.

## Command router

When a component exports WASI command, assemble `omnia_guest::api::command::RouterBuilder::new(clap_command, invoker)` and `build()` it into the executable `command::Router`. Parse CLI args into the operation input, register `run::<Args, Operation>()`, and project `Outcome::{Output, Operation, Decode}` into stdout, stderr, and exit status. Do not hand-match argument vectors or call domain functions around the operation kernel.

## Explicit exports

Export every transport explicitly:

```rust
struct HttpGuest;
wasip3::http::service::export!(HttpGuest);

struct MessagingGuest;
omnia_wasi_messaging::export!(MessagingGuest with_types_in omnia_wasi_messaging);

struct CliGuest;
wasip3::cli::command::export!(CliGuest);
```

Each WIT implementation assembles its typed router and delegates to the transport adapter. There is no aggregate guest macro.
