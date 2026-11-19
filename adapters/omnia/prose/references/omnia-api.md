# The omnia API

The signatures a part compiles against, as `omnia-sdk` and `omnia-test` spell them at the version the [layout](layout.md)'s root manifest pins. Read them here rather than probing the crates through the shell: every path, generic, and bound below is the pin's.

## Routing — `Handlers::route`

One `.route(PATH, method(handler))` per route, chained on `Handlers::new()` in a part's `handlers()`, the method function axum's:

```rust
// axum::routing
pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
pub fn post<H, T, S>(handler: H) -> MethodRouter<S>
pub fn put<H, T, S>(handler: H) -> MethodRouter<S>
pub fn patch<H, T, S>(handler: H) -> MethodRouter<S>
pub fn delete<H, T, S>(handler: H) -> MethodRouter<S>
// `get(read).post(create)` serves two methods at one path

// omnia_sdk::api
impl<P: Send + Sync + 'static> Handlers<P> {
    pub fn new() -> Self
    pub fn route(self, path: &str, method_router: MethodRouter<Arc<P>>) -> Self
        // panics where `path` is already routed under a method of `method_router`
    pub fn merge(self, other: Self) -> Self
        // panics where the two serve one path under one method or one topic
    pub fn with_state(self, provider: P) -> Router<P>
}
impl<P: Send + Sync + 'static, const N: usize> From<[Handlers<P>; N]> for Handlers<P>
    // the parts merged in order; `Handlers::from([])` is `Handlers::new()`
```

- A handler is a plain axum handler whose first extractor is `Context<P>`: `pub(crate) async fn read<P>(context: Context<P>, Path(id): Path<String>) -> Result<Json<ReadOutput>>`.
- Its other extractors are axum's, in axum's order: `Path<T>` for the path parameters (`Path(id): Path<String>`, `Path((id, line)): Path<(String, u32)>`, typed so a malformed one is axum's `400`), `Query<T>` for the query string, and last `Json<T>` for a JSON body or `axum::body::Bytes` for any other.
- Its `Ok` is what axum answers: `Json<O>` is `200` with the JSON body, `(StatusCode, Json<O>)` the status named, `StatusCode` alone an empty body. Its `Err` is anything `IntoResponse`, which [`omnia_sdk::Error`](#errors--omnia_sdkerror) is.
- Paths are axum's: `/orders/{id}` binds `id`, `/files/{*path}` the rest.

The stem's entry folds its parts' `handlers()` with `Handlers::from([..])`, the root's `router()` folds the stems' the same way and closes the whole with `.with_state(provider)`, which is the one `Router<P>` the two exports serve from and a test drives. `axum` re-exports `http` (`axum::http`) and `Bytes` (`axum::body::Bytes`); any other crate a part uses (`anyhow`, `bytes`, `http`, `thiserror`, `tracing`) is named in its own stem's `[dependencies]`.

## Messaging — `Handlers::topic`

One `.topic(topic::NAME, handler)` per topic a part consumes, on the same `Handlers`, the constant the shared table's:

```rust
// omnia_sdk::api
impl<P: Send + Sync + 'static> Handlers<P> {
    pub fn topic<F, I>(self, topic: impl Into<String>, handler: F) -> Self
    where
        F: Handler<P, I>,
        I: DeserializeOwned + Send + 'static,
        // the payload decoded as JSON into `I`; panics on an empty topic or one already registered
    pub fn topic_with<F, I, D>(self, topic: impl Into<String>, handler: F, decode: D) -> Self
    where
        F: Handler<P, I>,
        I: Send + 'static,
        D: Fn(&Delivery) -> omnia_sdk::Result<I> + Send + Sync + 'static,
        // a decoder failure becomes `Error::bad_request("undecodable", error.description())`
}
impl<P: Send + Sync + 'static> Router<P> {
    pub async fn handle(&self, delivery: Delivery) -> omnia_sdk::Result<()>
    pub fn topics(&self) -> impl Iterator<Item = &str>
}

// omnia_sdk::api::messaging
pub struct Delivery {
    pub topic: Option<String>,
    pub payload: Bytes,
    pub key: Option<String>,
    pub content_type: Option<String>,
    pub metadata: Vec<(String, String)>,
}
impl Delivery {
    pub fn new(topic: impl Into<String>, payload: &[u8]) -> Self
    pub fn with_key(self, key: impl Into<String>) -> Self
    pub fn with_content_type(self, content_type: impl Into<String>) -> Self
    pub fn with_metadata(self, name: impl Into<String>, value: impl Into<String>) -> Self
}
impl Default for Delivery
```

- A consumer is a `Handler<P, I>` as a route's is, its input the decoded payload rather than an extractor: `pub(crate) async fn on_placed<P>(context: Context<P>, input: Placed) -> Result<()>`, its output dropped once it answers `Ok`. `handle` answers `omnia_sdk::Result<()>`: `Error::bad_request("missing-topic", ..)` when `topic` is `None`, `Error::not_found("unhandled-topic", ..)` when unregistered, `Error::bad_request("undecodable", ..)` when decoding fails, or the handler's error, which the host reads by variant and code and a test matches on.
- `handle` matches the delivery's topic exactly against the constant the part registered, so a publisher and its consumer name one constant of `common::topic` and agree by construction.
- A payload that is not JSON is decoded by the part's own `fn decode(delivery: &Delivery) -> Result<Input>` under `.topic_with(topic::NAME, handler, decode)`, as the fixture's `dispatch.rs` decodes XML through `quick_xml::de::from_reader(&delivery.payload[..])` mapped with `bad_request!`.
- The delivery's `metadata` fills the handler's `Metadata`: `request-id`, `correlation-id`, `causation-id`, `tenant`, and `deadline`, matched case-insensitively, a request id minted where none arrived.

## The handler — `omnia_sdk::api`

```rust
pub struct Context<P> {
    pub metadata: Metadata,
    ..
}
impl<P> Context<P> {
    pub fn new(provider: P, metadata: Metadata) -> Self   // outside a router, to unit-test a handler
    pub fn provider(&self) -> &P
}
// one accessor per capability, each present where `P` implements the trait,
// so a handler bounded on two traits that both define a method names the one it means
impl<P: BlobStore> Context<P> { pub fn blobs(&self) -> &impl BlobStore }
impl<P: Config> Context<P> { pub fn config(&self) -> &impl Config }
impl<P: DocumentDb> Context<P> { pub fn docs(&self) -> &impl DocumentDb }
impl<P: HttpClient> Context<P> { pub fn http(&self) -> &impl HttpClient }
impl<P: Identity> Context<P> { pub fn identity(&self) -> &impl Identity }
impl<P: Publisher> Context<P> { pub fn publisher(&self) -> &impl Publisher }
impl<P: SecretStore> Context<P> { pub fn secrets(&self) -> &impl SecretStore }
impl<P: SqlDb> Context<P> { pub fn sql(&self) -> &impl SqlDb }
impl<P: StateStore> Context<P> { pub fn state(&self) -> &impl StateStore }
impl<P: WebSockets> Context<P> { pub fn websockets(&self) -> &impl WebSockets }

pub trait Handler<P, I>: Clone + Send + Sync + 'static {
    type Output: Send;
    type Error: std::error::Error + Send + Sync + 'static;
    fn call(self, context: Context<P>, input: I)
        -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}
// the blanket impl: every `async fn(Context<P>, I) -> Result<O, E>` is a Handler
impl<F, Fut, P, I, O, E> Handler<P, I> for F
where
    F: FnOnce(Context<P>, I) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<O, E>> + Send,
    O: Send,
    E: std::error::Error + Send + Sync + 'static,

pub struct Metadata {
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub tenant: Option<String>,
    pub deadline: Option<SystemTime>,
}
impl Metadata {
    pub fn minted(request_id: String) -> Self
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self
    pub fn from_headers(headers: &HeaderMap) -> Self
}
impl Default for Metadata

pub struct Router<P: Send + Sync + 'static = ()> { .. }
impl<P: Send + Sync + 'static> Clone for Router<P>
impl<P, B> tower::Service<http::Request<B>> for Router<P>   // the HTTP side, which `send` drives
```

A handler reaches a capability through the context's accessor for it, `context.state().get(..)`, `context.config().get(..)`, and bounds its `P` on the traits it reaches (`where P: StateStore + Config`, or no `where` clause when it reaches none); the stem's `pub trait Requires` in `crates/<stem>/src/lib.rs` collects the traits the stem's handlers reach, `P: crate::Requires` bounds the part's `handlers()`, `P: Requires` bounds the stem's `handlers()`, and the root's `router()` lists `P: <stem>::Requires,` per stem. A helper several capabilities serve takes `context.provider()` under its own bound. A test reaches a handler through the stem's `handlers().with_state(provider.clone())`, since the handler is `pub(crate)`, as [testing](testing.md) spells it.

## Errors — `omnia_sdk::Error`

```rust
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub enum Error {
    BadRequest { code: String, description: String },   // 400
    NotFound { code: String, description: String },     // 404
    ServerError { code: String, description: String },  // 500
    BadGateway { code: String, description: String },   // 502
}
impl Error {
    pub fn bad_request(code: impl Into<String>, description: impl Into<String>) -> Self
    pub fn not_found(code: impl Into<String>, description: impl Into<String>) -> Self
    pub fn server_error(code: impl Into<String>, description: impl Into<String>) -> Self
    pub fn bad_gateway(code: impl Into<String>, description: impl Into<String>) -> Self
    pub const fn status(&self) -> StatusCode
    pub const fn code(&self) -> &str
    pub const fn description(&self) -> &str
}
impl IntoResponse for Error             // the status, a JSON body `{"error": <code>, "message": <description>}`
impl From<anyhow::Error> for Error      // the first domain Error in the chain, else a server_error
impl From<serde_json::Error> for Error  // a bad_request

// each takes a format string and arguments, or one expression spelled with `to_string`
bad_request!("..")    // code "bad_request"
not_found!("..")      // code "not_found"
server_error!("..")   // code "server_error"
bad_gateway!("..")    // code "bad_gateway"
```

- The code is the `error` discriminant the wire body carries, so where the specification names a reason code the handler spells it, `Error::bad_request("segment_unknown", ..)`; the macro is the same error under the default code.
- Every capability call answers `omnia_sdk::Result<T>` directly (and `anyhow::Error` converts through `From<anyhow::Error>`), so `?` propagates it inside a `Result<_>` handler.
- `Error` has **no** `409` variant (`Error::bad_request("conflict", ..)` still answers `400`) and serialises only `{"error": <code>, "message": <description>}`, while `bad_gateway!` and `HttpClientExt::fetch_json` hardcode `"error": "bad_gateway"` with no extra fields. Any status outside the four (`409` for state or concurrency conflicts) or any error carrying extra wire fields (`"state"`, `"upstream"`, `"issues"`) is the part's own `IntoResponse` error enum (wrapping `Host(#[from] omnia_sdk::Error)`, and `Gate(#[from] GateError)` where gated, with `thiserror` and `anyhow` in `[dependencies]`), returned from both the part's helper functions and its handlers as [handlers](handlers.md) shows under "A status of its own".

## The capabilities — `omnia_sdk`

Every trait is re-exported at the crate root, value types live in their capability module (`keyvalue`, `blobstore`, `docstore`, `sql`, `identity`, `vault`, `messaging`, plus `omnia_sdk::Page<T>`), every method answers `omnia_sdk::Result` through `impl Future<Output = ..> + Send`, and `WasiProvider` implements each on `wasm32`. Each stem's `Requires` trait lists the capability traits its handlers reach, and a handler bounds its `P` on the ones its body uses. The `*Ext` traits are blanket-implemented over every implementor and imported `as _` for their methods:

```rust
pub trait Config: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<String>>> + Send;
}
pub trait ConfigExt: Config {   // blanket, over every Config
    fn require<T: FromStr>(&self, key: &str) -> impl Future<Output = Result<T>> + Send
    where T::Err: fmt::Display;   // unset -> server_error("not-found", ..); bad parse -> server_error("invalid", ..)
    fn parse<T: FromStr>(&self, key: &str) -> impl Future<Output = Result<Option<T>>> + Send
    where T::Err: fmt::Display;
}

pub trait StateStore: Send + Sync {
    type Named: StateStore<Named = Self::Named>;
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;
    fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>)
        -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    fn compare_and_swap(&self, key: &str, current: Option<&[u8]>, new: &[u8], ttl: Option<Duration>)
        -> impl Future<Output = Result<std::result::Result<(), keyvalue::Conflict>>> + Send;
    fn increment(&self, key: &str, delta: i64, ttl: Option<Duration>)
        -> impl Future<Output = Result<i64>> + Send;
    fn named(&self, name: &str) -> Self::Named;
}
pub trait StateStoreExt: StateStore {   // blanket, over every StateStore
    fn get_json<T: DeserializeOwned>(&self, key: &str) -> impl Future<Output = Result<Option<T>>> + Send;
    fn set_json<T: Serialize + Sync + ?Sized>(&self, key: &str, value: &T, ttl: Option<Duration>)
        -> impl Future<Output = Result<()>> + Send;
}
// omnia_sdk::keyvalue — portable keys: non-empty ASCII `[A-Za-z0-9._-]`, no leading/trailing `.` (never `:` or `/`); non-zero TTL
pub struct Conflict { pub current: Option<Bytes> }   // None when the key was absent

pub trait BlobStore: Send + Sync {
    fn get(&self, container: &str, name: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;
    fn get_range(&self, container: &str, name: &str, range: Range<u64>)
        -> impl Future<Output = Result<Option<Bytes>>> + Send;
    fn put(&self, container: &str, name: &str, data: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, container: &str, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn stat(&self, container: &str, name: &str)
        -> impl Future<Output = Result<Option<blobstore::ObjectMetadata>>> + Send;
    fn list(&self, container: &str, prefix: &str, after: Option<&str>)
        -> impl Future<Output = Result<Page<blobstore::ObjectMetadata>>> + Send;
    fn create_container(&self, container: &str) -> impl Future<Output = Result<()>> + Send;
    fn delete_container(&self, container: &str) -> impl Future<Output = Result<()>> + Send;
    fn container_exists(&self, container: &str) -> impl Future<Output = Result<bool>> + Send;
    fn container_info(&self, container: &str)
        -> impl Future<Output = Result<Option<blobstore::ContainerMetadata>>> + Send;
    fn exists(&self, container: &str, name: &str) -> impl Future<Output = Result<bool>> + Send;
    fn delete_many(&self, container: &str, names: &[&str]) -> impl Future<Output = Result<()>> + Send;
    fn clear(&self, container: &str) -> impl Future<Output = Result<()>> + Send;
    fn copy(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
    fn rename(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
}
// omnia_sdk::Page<T> { pub items: Vec<T>, pub next: Option<String> }; omnia_sdk::blobstore:
pub struct ObjectMetadata { pub name: String, pub container: String, pub created_at: SystemTime, pub size: u64 }
pub struct ContainerMetadata { pub name: String, pub created_at: SystemTime }

pub trait DocumentDb: Send + Sync {
    fn get(&self, collection: &str, id: &str) -> impl Future<Output = Result<Option<docstore::Document>>> + Send;
    fn insert(&self, collection: &str, document: &docstore::Document)
        -> impl Future<Output = Result<std::result::Result<(), docstore::Exists>>> + Send;
    fn put(&self, collection: &str, document: &docstore::Document) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, collection: &str, id: &str) -> impl Future<Output = Result<bool>> + Send;
    fn query(&self, collection: &str, query: &docstore::Query)
        -> impl Future<Output = Result<Page<docstore::Document>>> + Send;
}
pub trait DocumentDbExt: DocumentDb {   // blanket, over every DocumentDb
    fn get_as<T: DeserializeOwned>(&self, collection: &str, id: &str)
        -> impl Future<Output = Result<Option<T>>> + Send;
}
// omnia_sdk::Page<T> { pub items: Vec<T>, pub next: Option<String> }; omnia_sdk::docstore:
pub struct Document { pub id: String, pub data: Bytes }
impl Document {
    pub fn new(id: impl Into<String>, data: &[u8]) -> Self
    pub fn json<T: Serialize + ?Sized>(id: impl Into<String>, value: &T) -> Result<Self>
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T>
}
pub struct Exists;
pub struct Query {
    pub filter: Option<Filter>,
    pub order_by: Vec<SortField>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub after: Option<String>,
}
impl Default for Query
pub struct SortField { pub field: String, pub descending: bool }
pub enum Filter {
    Compare { field: String, op: ComparisonOp, value: ScalarValue },
    InList { field: String, values: Vec<ScalarValue> },
    NotInList { field: String, values: Vec<ScalarValue> },
    IsNull(String),
    IsNotNull(String),
    Contains { field: String, pattern: String },
    StartsWith { field: String, pattern: String },
    EndsWith { field: String, pattern: String },
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}
// Filter shorthands: Filter::{eq, ne, gt, gte, lt, lte, in_list, not_in_list, is_null, is_not_null, contains, starts_with, ends_with, and, or, on_date}
pub enum ComparisonOp { Eq, Ne, Gt, Gte, Lt, Lte }
pub enum ScalarValue { Null, Bool(bool), Int32(i32), Int64(i64), Float64(f64), Str(String), Binary(Vec<u8>), Timestamp(String) }

pub trait SqlDb: Send + Sync {
    type Named: SqlDb<Named = Self::Named>;
    fn query(&self, sql: &str, params: &[sql::Value]) -> impl Future<Output = Result<Vec<sql::Row>>> + Send;
    fn execute(&self, sql: &str, params: &[sql::Value]) -> impl Future<Output = Result<u64>> + Send;   // rows changed
    fn batch(&self, statements: &[sql::Statement]) -> impl Future<Output = Result<Vec<u64>>> + Send;
    fn named(&self, name: &str) -> Self::Named;
}
pub trait SqlDbExt: SqlDb {   // blanket, over every SqlDb
    fn query_as<T: sql::FromRow>(&self, sql: &str, params: &[sql::Value])
        -> impl Future<Output = Result<Vec<T>>> + Send;
}
// omnia_sdk::sql — `$1, $2, ..` parameters; provider handle uses `"default"`, `.named(..)` selects another
pub struct Row { pub fields: Vec<Field> }
impl Row {
    pub fn get<'a, T: FromColumn<'a>>(&'a self, column: &str) -> Result<T>
        // T: i32, i64, u32, u64, f32, f64, bool, String, &str, Vec<u8>, Bytes, or Option of one for a nullable column
}
pub struct Field { pub name: String, pub value: Value }
pub struct Statement { pub sql: String, pub params: Vec<Value> }
impl Statement { pub fn new(sql: impl Into<String>, params: impl Into<Vec<Value>>) -> Self }
pub enum Value {   // `From` impls for String, &str, i32, i64, u32, u64, f32, f64, bool, Vec<u8>, Bytes, and Option<T>
    Int32(Option<i32>), Int64(Option<i64>), Uint32(Option<u32>), Uint64(Option<u64>),
    Float(Option<f32>), Double(Option<f64>), Str(Option<String>), Boolean(Option<bool>),
    Date(Option<String>), Time(Option<String>), Timestamp(Option<String>), Binary(Option<Bytes>),
}
pub trait FromRow: Sized {
    fn from_row(row: &Row) -> Result<Self>;
}
#[derive(FromRow)]   // omnia_sdk::sql::FromRow: each field read from the column of its name through `Row::get`

pub trait HttpClient: Send + Sync {
    fn fetch(&self, request: http::Request<Bytes>)
        -> impl Future<Output = Result<http::Response<Bytes>>> + Send;
}
pub trait HttpClientExt: HttpClient {   // blanket, over every HttpClient
    fn fetch_json<O: DeserializeOwned>(
        &self, request: http::Request<Bytes>,
    ) -> impl Future<Output = Result<O>> + Send;
        // a non-2xx status or an unreadable JSON body is a `bad_gateway!`
}
// `axum::body::Bytes` is `bytes::Bytes`: `Request::get(url).body(Bytes::new())`, `Bytes::from(vec)`

pub trait Identity: Send + Sync {
    type Named: Identity<Named = Self::Named>;
    fn token(&self, scopes: &[&str]) -> impl Future<Output = Result<identity::AccessToken>> + Send;
    fn named(&self, name: &str) -> Self::Named;
}
pub trait IdentityExt: Identity {   // blanket, over every Identity
    fn access_token(&self) -> impl Future<Output = Result<String>> + Send;   // default scopes (`&[]`)
}
// omnia_sdk::identity — `context.identity().named(&settings.upstream_identity).access_token().await?`
pub struct AccessToken { pub token: String, pub expires_in: Duration }

pub trait SecretStore: Send + Sync {
    type Named: SecretStore<Named = Self::Named>;
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Bytes>>> + Send;
    fn set(&self, key: &str, value: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    fn keys(&self, after: Option<&str>) -> impl Future<Output = Result<Page<String>>> + Send;
    fn named(&self, name: &str) -> Self::Named;
    fn exists(&self, key: &str) -> impl Future<Output = Result<bool>> + Send;
}
// omnia_sdk::Page<T> { pub items: Vec<T>, pub next: Option<String> }

pub trait Publisher: Send + Sync {
    fn publish(&self, topic: &str, message: &messaging::Message) -> impl Future<Output = Result<()>> + Send;
}
// omnia_sdk::messaging
pub struct Message {
    pub payload: Bytes,
    pub key: Option<String>,
    pub content_type: Option<String>,
    pub headers: HashMap<String, String>,
}
impl Message {
    pub fn new(payload: &[u8]) -> Self
    pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<Self>
    pub fn with_key(self, key: impl Into<String>) -> Self
    pub fn with_content_type(self, content_type: impl Into<String>) -> Self
    pub fn with_header(self, key: impl Into<String>, value: impl Into<String>) -> Self
}

pub trait WebSockets: Send + Sync {
    fn broadcast(&self, data: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn send_to(&self, sockets: &[&str], data: &[u8]) -> impl Future<Output = Result<()>> + Send;
}
```

`compare_and_swap` answers `Result<Result<(), keyvalue::Conflict>>` (and `DocumentDb::insert` answers `Result<Result<(), docstore::Exists>>`), so `?` propagates a store failure and the handler matches on `Ok(())` vs `Err(Conflict { current })` for a retry or a `409`.

## Settings — `omnia_sdk::Settings`

A part that reads required configuration declares one struct deriving `Settings`, private fields, one per key, and loads it where a body needs one:

```rust
// The settings the part reads: `ORDERS_LIMIT`.
#[derive(Debug, Settings)]
#[setting(prefix = "ORDERS_")]
pub(crate) struct PlacingSettings {
    limit: usize,
}

// what the derive emits
impl PlacingSettings {
    pub const KEYS: &'static [&'static str];   // ["ORDERS_LIMIT"]
    pub async fn load<C: Config + ?Sized>(config: &C) -> omnia_sdk::Result<Self>
        // each field through `ConfigExt::require::<T>`, an unset key ("not-found") or unparseable value ("invalid") a server_error
}
```

- A field's key is its name upper-cased behind the struct's `#[setting(prefix = "..")]`, or the literal in its own `#[setting(key = "..")]`; a field is typed as its value parses, `usize`, `u64`, `String`, `Url`.
- `let settings = PlacingSettings::load(context.config()).await?;` in the handler, under `P: Config` among its bounds.
- A setting no scenario depends on is a direct `context.config().get("KEY").await?` read (or `.parse::<T>("KEY").await?`), which answers `None` when unset.

## The doubles — `omnia_test::guest`

`Provider` answers every capability natively and is `Clone` and `Default`; each double is set with the builder method of its name and read back through the field of the same name:

```rust
pub struct Provider {
    pub config: MapConfig,
    pub http: MatchedHttp,
    pub identity: FixedIdentity,
    pub publisher: Sink,
    pub websockets: Sink,
    pub storage: Memory,       // StateStore, BlobStore, and SecretStore
    pub docs: MemoryDocs,
    pub sql: ScriptedTables,
    ..
}
impl Provider {
    pub fn config(self, config: MapConfig) -> Self
    pub fn http(self, http: MatchedHttp) -> Self
    pub fn identity(self, identity: FixedIdentity) -> Self
    pub fn publisher(self, publisher: Sink) -> Self
    pub fn websockets(self, websockets: Sink) -> Self
    pub fn storage(self, storage: Memory) -> Self
    pub fn docs(self, docs: MemoryDocs) -> Self
    pub fn sql(self, sql: ScriptedTables) -> Self
}

impl MapConfig {   // Default: empty, so `Config::get` answers `Ok(None)` and `ConfigExt::require` fails
    pub fn with<K: Into<String>, V: Into<String>>(self, pairs: impl IntoIterator<Item = (K, V)>) -> Self
}

impl MatchedHttp {   // Default: no response, so a fetch panics naming the method and URL
    pub fn on(self, method: http::Method, url: impl Into<String>, response: http::Response<Bytes>) -> Self
    pub fn on_matching(
        self, method: http::Method, url: impl Into<String>,
        body: impl Fn(&[u8]) -> bool + Send + Sync + 'static, response: http::Response<Bytes>,
    ) -> Self
    pub fn requests(&self) -> Vec<Recorded>
}
pub struct Recorded { pub method: http::Method, pub uri: http::Uri, pub headers: HeaderMap, pub body: Vec<u8> }
impl Recorded {
    pub fn text(&self) -> String   // the body as UTF-8
}

impl FixedIdentity {   // Default: an empty token; `named("svc")` answers `"<token>:svc"` and records `"svc"`
    pub fn new(token: impl Into<String>) -> Self
    pub fn asked(&self) -> Vec<String>   // every identity a token was asked for
}

impl Sink {   // Default: records every publish and WebSocket send
    pub fn sent(&self) -> Vec<(String, Message)>   // Publisher: (topic, message)
    pub fn broadcasts(&self) -> Vec<Broadcasted>   // WebSockets
}
pub struct Broadcasted { pub data: Vec<u8>, pub sockets: Option<Vec<String>> }

impl Memory {   // Default: empty `"default"` state bucket; an entry set with a `Duration` TTL expires after it
    pub fn bucket(self, name: &str) -> Self
    pub fn named(&self, name: &str) -> Self
    pub fn state(&self, key: &str) -> Option<Vec<u8>>
    pub fn secret(&self, key: &str) -> Option<Vec<u8>>
    pub fn object(&self, container: &str, name: &str) -> Option<Vec<u8>>
    pub fn objects(&self, container: &str) -> Vec<String>
    pub fn insert_state(&self, key: &str, bytes: &[u8])
    pub fn insert_secret(&self, key: &str, bytes: &[u8])
    pub fn insert_object(&self, container: &str, name: &str, bytes: &[u8])
    pub fn insert_container(&self, name: &str)
    pub fn has_container(&self, name: &str) -> bool
    pub fn is_empty(&self) -> bool
}

impl MemoryDocs {   // Default: empty
    pub fn documents(&self, collection: &str) -> impl Future<Output = omnia_sdk::Result<Vec<Document>>> + Send
}

impl ScriptedTables {   // Default: no rule, so a statement panics naming its SQL and parameters
    pub fn on_query(self, matches: impl Fn(&str, &[sql::Value]) -> bool + Send + Sync + 'static, rows: Vec<sql::Row>) -> Self
    pub fn on_exec(self, matches: impl Fn(&str, &[sql::Value]) -> bool + Send + Sync + 'static, affected: u64) -> Self
    pub fn statements(&self) -> Vec<Executed>
}
pub struct Executed { pub binding: String, pub sql: String, pub params: Vec<sql::Value> }

pub async fn send<S, B, RB>(service: &S, request: http::Request<B>) -> http::Response<Bytes>
    // one request through the router, cloned for the call, its body collected; `Router<P>` is such an `S`

pub async fn deliver<P>(router: &Router<P>, topic: &str, payload: impl Into<Vec<u8>>) -> omnia_sdk::Result<()>
    // one delivery on `topic` through the router, with no key, content type, or metadata
```

`Provider` and its doubles share their records across clones, so a test clones the provider into the router and reads the original back after the call. How a scenario arranges them is in [testing](testing.md).
