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
    pub fn route(self, path: &str, method_router: MethodRouter<Client<P>>) -> Self
        // panics where `path` is already routed under a method of `method_router`
    pub fn merge(self, other: Self) -> Self
        // panics where the two serve one path under one method or one topic
    pub fn with_state(self, client: Client<P>) -> Router<P>
}
impl<P: Send + Sync + 'static, const N: usize> From<[Handlers<P>; N]> for Handlers<P>
    // the parts merged in order; `Handlers::from([])` is `Handlers::new()`
```

- A handler is a plain axum handler whose first extractor is `Context<P>`: `pub(crate) async fn read<P>(context: Context<P>, Path(id): Path<String>) -> Result<Json<ReadOutput>>`.
- Its other extractors are axum's, in axum's order: `Path<T>` for the path parameters (`Path(id): Path<String>`, `Path((id, line)): Path<(String, u32)>`, typed so a malformed one is axum's `400`), `Query<T>` for the query string, and last `Json<T>` for a JSON body or `Bytes` for any other.
- Its `Ok` is what axum answers: `Json<O>` is `200` with the JSON body, `(StatusCode, Json<O>)` the status named, `StatusCode` alone an empty body. Its `Err` is anything `IntoResponse`, which [`omnia_sdk::Error`](#errors--omnia_sdkerror) is.
- Paths are axum's: `/orders/{id}` binds `id`, `/files/{*path}` the rest.

The stem's entry folds its parts' `handlers()` with `Handlers::from([..])`, the root's `router()` folds the stems' the same way and closes the whole with `.with_state(Client::new(provider))`, which is the one `Router<P>` the two exports serve from and a test drives. `omnia_sdk` re-exports `axum`, `http`, `bytes`, `http_body`, `anyhow`, and `tracing` at its root, so a part names them through its own stem's manifest or as `omnia_sdk::axum::..`.

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
        D: Fn(&Delivery) -> Result<I, DecodeError> + Send + Sync + 'static,
}
impl<P: Send + Sync + 'static> Router<P> {
    pub async fn handle(&self, delivery: Delivery) -> Result<(), DeliveryError>
    pub fn topics(&self) -> impl Iterator<Item = &str>
}

// omnia_sdk::api::messaging
pub struct Delivery {
    pub topic: Option<String>,
    pub payload: Vec<u8>,
    pub content_type: Option<String>,
    pub metadata: Vec<(String, String)>,
}
impl Default for Delivery

pub enum DeliveryError {
    MissingTopic,
    UnhandledTopic(String),
    Rejected(String),   // the decoder's or the handler's error, displayed
}
impl std::error::Error for DeliveryError

// omnia_sdk::api
pub struct DecodeError { .. }
impl DecodeError {
    pub fn new(description: impl Into<String>) -> Self
    pub fn description(&self) -> &str
}
```

- A consumer is a `Handler<P, I>` as a route's is, its input the decoded payload rather than an extractor: `pub(crate) async fn on_placed<P>(context: Context<P>, input: Placed) -> Result<()>`, its output dropped once it answers `Ok`. A failure is `DeliveryError::Rejected` with the error displayed, which the host reads as a failed delivery and a test matches on.
- `handle` matches the delivery's topic exactly against the constant the part registered, so a publisher and its consumer name one constant of `common::topic` and agree by construction.
- A payload that is not JSON is decoded by the part's own `fn decode(delivery: &Delivery) -> std::result::Result<Input, DecodeError>` under `.topic_with(topic::NAME, handler, decode)`, as the fixture's `dispatch.rs` decodes XML through `quick_xml::de::from_reader(delivery.payload.as_slice())`.
- The delivery's `metadata` fills the handler's `Metadata`: `request-id`, `correlation-id`, `causation-id`, and `tenant`, matched case-insensitively, a request id minted where none arrived.

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
impl<P: Broadcast> Context<P> { pub fn broadcast(&self) -> &impl Broadcast }
impl<P: Config> Context<P> { pub fn config(&self) -> &impl Config }
impl<P: DocumentStore> Context<P> { pub fn docs(&self) -> &impl DocumentStore }
impl<P: HttpRequest> Context<P> { pub fn http(&self) -> &impl HttpRequest }
impl<P: Identity> Context<P> { pub fn identity(&self) -> &impl Identity }
impl<P: Publish> Context<P> { pub fn publish(&self) -> &impl Publish }
impl<P: StateStore> Context<P> { pub fn state(&self) -> &impl StateStore }
impl<P: TableStore> Context<P> { pub fn tables(&self) -> &impl TableStore }
impl<P: Vault> Context<P> { pub fn vault(&self) -> &impl Vault }

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
}
impl Default for Metadata

pub struct Client<P> { .. }
impl<P: Send + Sync + 'static> Client<P> {
    pub fn new(provider: P) -> Self
    pub fn provider(&self) -> &P
}
impl<P: Send + Sync + 'static> Clone for Client<P>

pub struct Router<P: Send + Sync + 'static = ()> { .. }
impl<P: Send + Sync + 'static> Clone for Router<P>
impl<P, B> tower::Service<http::Request<B>> for Router<P>   // the HTTP side, which `send` drives
```

A handler reaches a capability through the context's accessor for it, `context.state().get(..)`, `context.config().get(..)`, and bounds its `P` on the traits it reaches, `where P: StateStore + Config`; the `Capabilities` bound is the `handlers()` fn's and the root's. A helper several capabilities serve takes `context.provider()` under its own bound. A test reaches a handler through the stem's `handlers().with_state(Client::new(provider))`, since the handler is `pub(crate)`, as [testing](testing.md) spells it.

## Errors — `omnia_sdk::Error`

```rust
pub type Result<T> = std::result::Result<T, Error>;

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
    pub fn code(&self) -> String
    pub fn description(&self) -> String
}
impl IntoResponse for Error   // the status, a JSON body `{"error": <code>, "message": <description>}`
impl From<anyhow::Error> for Error      // the chain joined into a server_error, or the Error it wraps
impl From<serde_json::Error> for Error  // a bad_request
impl From<sql::ColumnError> for Error   // a server_error

// each takes a format string and arguments, or one expression spelled with `to_string`
bad_request!("..")    // code "bad_request"
not_found!("..")      // code "not_found"
server_error!("..")   // code "server_error"
bad_gateway!("..")    // code "bad_gateway"
```

- The code is the `error` discriminant the wire body carries, so where the specification names a reason code the handler spells it, `Error::bad_request("segment_unknown", ..)`; the macro is the same error under the default code.
- A provider call answers `anyhow::Result<T>`; `?` on it inside a `Result<_>` handler converts through `From<anyhow::Error>`.
- A status the four do not cover, a `409`, is the part's own error type: an enum over its cases with a `Host(#[from] omnia_sdk::Error)` variant, implementing `IntoResponse`, as [handlers](handlers.md) shows under "A status of its own".

## The capabilities — `omnia_sdk`

Every trait is re-exported at the crate root, every method answers `anyhow::Result` through `impl Future<Output = ..> + Send`, and the production provider implements each over WASI on `wasm32`. `omnia_sdk::Capabilities` is every trait at once, the bound a `handlers()` takes; a handler bounds its `P` on the traits it reaches. The `*Ext` traits are blanket-implemented over every implementor and imported `as _` for their methods:

```rust
pub trait Config: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<String>> + Send;
}
pub trait ConfigExt: Config {   // blanket, over every Config
    fn get_parsed<T: FromStr>(&self, key: &str) -> impl Future<Output = Result<T>> + Send;
}

pub trait StateStore: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;
    fn set(&self, key: &str, value: &[u8], ttl_secs: Option<u64>)
        -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    fn cas(&self, key: &str, expected: Option<&[u8]>, value: &[u8])
        -> impl Future<Output = Result<(), CasError>> + Send;
    fn increment(&self, key: &str, delta: i64) -> impl Future<Output = Result<i64>> + Send;
}
pub trait StateStoreExt: StateStore {   // blanket, over every StateStore
    fn get_json<T: DeserializeOwned>(&self, key: &str) -> impl Future<Output = Result<Option<T>>> + Send;
    fn set_json<T: Serialize + ?Sized>(&self, key: &str, value: &T, ttl_secs: Option<u64>)
        -> impl Future<Output = Result<()>> + Send;
}
pub enum CasError {
    Conflict(Option<Vec<u8>>),   // the value found; None when the key was absent
    Store(String),
}

pub trait BlobStore: Send + Sync {
    fn get(&self, container: &str, name: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;
    fn put(&self, container: &str, name: &str, data: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, container: &str, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn list(&self, container: &str) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn get_range(&self, container: &str, name: &str, start: u64, end: u64)
        -> impl Future<Output = Result<Vec<u8>>> + Send;
    fn object_info(&self, container: &str, name: &str) -> impl Future<Output = Result<ObjectMetadata>> + Send;
    fn create_container(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn delete_container(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn container_exists(&self, name: &str) -> impl Future<Output = Result<bool>> + Send;
    fn container_info(&self, container: &str) -> impl Future<Output = Result<ContainerMetadata>> + Send;
    fn has(&self, container: &str, name: &str) -> impl Future<Output = Result<bool>> + Send;
    fn delete_objects(&self, container: &str, names: &[String]) -> impl Future<Output = Result<()>> + Send;
    fn clear(&self, container: &str) -> impl Future<Output = Result<()>> + Send;
    fn copy_object(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
    fn move_object(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
}
pub struct ObjectMetadata { pub name: String, pub container: String, pub created_at: u64, pub size: u64 }
pub struct ContainerMetadata { pub name: String, pub created_at: u64 }

pub trait DocumentStore: Send + Sync {
    fn get(&self, store: &str, id: &str) -> impl Future<Output = Result<Option<Document>>> + Send;
    fn insert(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> + Send;
    fn put(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, store: &str, id: &str) -> impl Future<Output = Result<bool>> + Send;
    fn query(&self, store: &str, options: QueryOptions) -> impl Future<Output = Result<QueryResult>> + Send;
}
pub trait DocumentStoreExt: DocumentStore {   // blanket, over every DocumentStore
    fn get_as<T: DeserializeOwned>(&self, store: &str, id: &str) -> impl Future<Output = Result<Option<T>>> + Send;
}
// omnia_sdk::docstore
pub struct Document { pub id: String, pub data: Vec<u8> }
pub struct QueryOptions {
    pub filter: Option<Filter>,
    pub order_by: Vec<SortField>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub continuation: Option<String>,
}
impl Default for QueryOptions
pub struct QueryResult { pub documents: Vec<Document>, pub continuation: Option<String> }
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
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
}
pub enum ComparisonOp { Eq, Ne, Gt, Gte, Lt, Lte }
pub enum ScalarValue { Null, Bool(bool), Int32(i32), Int64(i64), Float64(f64), Str(String), Binary(Vec<u8>), Timestamp(String) }

pub trait TableStore: Send + Sync {
    fn query(&self, conn_name: &str, query: &str, params: &[DataType])
        -> impl Future<Output = Result<Vec<Row>>> + Send;
    fn exec(&self, conn_name: &str, query: &str, params: &[DataType])
        -> impl Future<Output = Result<u32>> + Send;   // rows affected
}
pub trait TableStoreExt: TableStore {   // blanket, over every TableStore
    fn query_as<T: FromRow>(&self, conn_name: &str, query: &str, params: &[DataType])
        -> impl Future<Output = Result<Vec<T>>> + Send;
}
// omnia_sdk::sql
pub struct Row { pub index: String, pub fields: Vec<Field> }
impl Row {
    pub fn get<'a, T: FromColumn<'a>>(&'a self, column: &str) -> Result<T, ColumnError>
        // T: i32, i64, u32, u64, f32, f64, bool, String, &str, Vec<u8>, or Option of one for a nullable column
}
pub struct Field { pub name: String, pub value: DataType }
pub enum DataType {
    Int32(Option<i32>), Int64(Option<i64>), Uint32(Option<u32>), Uint64(Option<u64>),
    Float(Option<f32>), Double(Option<f64>), Str(Option<String>), Boolean(Option<bool>),
    Date(Option<String>), Time(Option<String>), Timestamp(Option<String>), Binary(Option<Vec<u8>>),
}
pub trait FromRow: Sized {
    fn from_row(row: &Row) -> anyhow::Result<Self>;
}
#[derive(FromRow)]   // omnia_sdk::sql::FromRow: each field read from the column of its name through `Row::get`
pub struct ColumnError { .. }   // a missing column, a NULL for a non-Option field, or an incompatible DataType

pub trait HttpRequest: Send + Sync {
    fn fetch<T>(&self, request: http::Request<T>) -> impl Future<Output = Result<http::Response<Bytes>>> + Send
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;
}
pub trait HttpRequestExt: HttpRequest {   // blanket, over every HttpRequest
    fn fetch_json<T, O: DeserializeOwned>(&self, request: http::Request<T>)
        -> impl Future<Output = omnia_sdk::Result<O>> + Send;
        // a non-2xx status or an unreadable JSON body is a `bad_gateway!`
}
// `axum::body::Body` is such a `T`: `Request::get(url).body(Body::empty())`, `Body::from(bytes)`

pub trait Identity: Send + Sync {
    fn token(&self, identity: &str, scopes: &[String]) -> impl Future<Output = Result<AccessToken>> + Send;
}
pub trait IdentityExt: Identity {   // blanket, over every Identity
    fn access_token(&self, identity: &str) -> impl Future<Output = Result<String>> + Send;   // default scopes
}
pub struct AccessToken { pub token: String, pub expires_in: u64 }

pub trait Vault: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;
    fn set(&self, key: &str, value: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    fn exists(&self, key: &str) -> impl Future<Output = Result<bool>> + Send;
    fn list_ids(&self) -> impl Future<Output = Result<Vec<String>>> + Send;
}

pub trait Publish: Send + Sync {
    fn send(&self, topic: &str, message: &Message) -> impl Future<Output = Result<()>> + Send;
}
pub struct Message { pub payload: Vec<u8>, pub headers: HashMap<String, String> }
impl Message {
    pub fn new(payload: &[u8]) -> Self
    pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<Self>
    pub fn with_header(self, key: impl Into<String>, value: impl Into<String>) -> Self
}

pub trait Broadcast: Send + Sync {
    fn send(&self, name: &str, data: &[u8], sockets: Option<Vec<String>>)
        -> impl Future<Output = Result<()>> + Send;   // None reaches every subscriber
}
```

A `cas` answers `Result<(), CasError>` rather than `anyhow::Result`, so a handler matches on `CasError::Conflict` for a retry or a `409` and maps `CasError::Store` to a `server_error!`.

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
        // each field through `ConfigExt::get_parsed::<T>`, a missing key or an unparseable value a server_error
}
```

- A field's key is its name upper-cased behind the struct's `#[setting(prefix = "..")]`, or the literal in its own `#[setting(key = "..")]`; a field is typed as its value parses, `usize`, `u64`, `String`, `Url`.
- `let settings = PlacingSettings::load(context.config()).await?;` in the handler, under `P: Config` among its bounds.
- A setting no scenario depends on is a direct `context.config().get("KEY").await` read, its absence handled where it is read.

## The doubles — `omnia_test::guest`

`Provider` answers every capability natively and is `Clone` and `Default`; each double is set with the builder method of its name and read back through the field of the same name:

```rust
pub struct Provider {
    pub config: MapConfig,
    pub http: MatchedHttp,
    pub identity: FixedIdentity,
    pub publish: Sink,
    pub broadcast: Sink,
    pub storage: Memory,       // StateStore, BlobStore, and Vault
    pub docs: MemoryDocs,
    pub tables: ScriptedTables,
    ..
}
impl Provider {
    pub fn config(self, config: MapConfig) -> Self
    pub fn http(self, http: MatchedHttp) -> Self
    pub fn identity(self, identity: FixedIdentity) -> Self
    pub fn publish(self, publish: Sink) -> Self
    pub fn broadcast(self, broadcast: Sink) -> Self
    pub fn storage(self, storage: Memory) -> Self
    pub fn docs(self, docs: MemoryDocs) -> Self
    pub fn tables(self, tables: ScriptedTables) -> Self
}

impl MapConfig {   // Default: no setting, so `Config::get` of any key fails
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

impl FixedIdentity {   // Default: an empty token
    pub fn new(token: impl Into<String>) -> Self
    pub fn asked(&self) -> Vec<String>   // every identity a token was asked for
}

impl Sink {   // Default: records every send
    pub fn sent(&self) -> Vec<(String, Message)>   // Publish: (topic, message)
    pub fn broadcasts(&self) -> Vec<Broadcasted>   // Broadcast
}
pub struct Broadcasted { pub name: String, pub data: Vec<u8>, pub sockets: Option<Vec<String>> }

impl Memory {   // Default: empty; an entry set with `ttl_secs` expires after it
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
    pub async fn documents(&self, store: &str) -> anyhow::Result<Vec<Document>>
}

impl ScriptedTables {   // Default: no rule, so a statement panics naming its SQL and parameters
    pub fn on_query(self, matches: impl Fn(&str, &[DataType]) -> bool + Send + Sync + 'static, rows: Vec<Row>) -> Self
    pub fn on_exec(self, matches: impl Fn(&str, &[DataType]) -> bool + Send + Sync + 'static, affected: u32) -> Self
    pub fn statements(&self) -> Vec<Statement>
}
pub struct Statement { pub connection: String, pub sql: String, pub params: Vec<DataType> }

pub async fn send<S, B>(service: &S, request: http::Request<B>) -> http::Response<Bytes>
    // one request through the router, cloned for the call, its body collected; `Router<P>` is such an `S`

pub async fn deliver<P>(router: &Router<P>, topic: &str, payload: impl Into<Vec<u8>>) -> Result<(), DeliveryError>
    // one delivery on `topic` through the router, with no content type and no metadata
```

`Provider` and its doubles share their records across clones, so a test clones the provider into the router and reads the original back after the call. How a scenario arranges them is in [testing](testing.md).
