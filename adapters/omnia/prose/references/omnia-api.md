# The omnia API

The signatures a module compiles against, as `omnia-sdk`, `omnia-test`, and `omnia-wasi-sql` spell them at the version the [layout](layout.md)'s root manifest pins. Read them here rather than probing the crates through the shell: every path, generic, and bound below is the pin's.

## Routing — `omnia_sdk::api::http`

One route function per method, each over a handler and answering the `MethodRouter` a `route` takes:

```rust
pub fn get<F, P, I>(handler: F) -> MethodRouter<Client<P>>
pub fn delete<F, P, I>(handler: F) -> MethodRouter<Client<P>>
pub fn post<F, P, I>(handler: F) -> MethodRouter<Client<P>>
pub fn put<F, P, I>(handler: F) -> MethodRouter<Client<P>>
pub fn patch<F, P, I>(handler: F) -> MethodRouter<Client<P>>
where
    F: Handler<P, I>,
    F::Output: Serialize,
    F::Error: Into<HttpError>,
    I: DeserializeOwned + Send + 'static,
    P: Send + Sync + 'static,
```

- `get` and `delete` decode the input from the path parameters and the query string together, as one URL-encoded form; `post`, `put`, and `patch` decode it from the JSON body with the path parameters inserted as string fields (an empty body is an empty object, a body that is not a JSON object is a `400`).
- A path parameter arrives as a string: in a body route the field it fills is a `String`, or deserialises from one.
- The output is answered as JSON with status `200`; a `Result<(), _>` handler answers `null`.
- Paths are axum's: `/orders/{id}` binds `id`.

The general constructor, for a method set, a non-JSON body, or a response that is not JSON:

```rust
pub fn handle_with<F, P, I, D, DE, E>(
    filter: MethodFilter, handler: F, decode: D, encode: E,
) -> MethodRouter<Client<P>>
where
    F: Handler<P, I>,
    F::Error: Into<HttpError>,
    I: Send + 'static,
    D: Fn(RawRequest<'_>) -> Result<I, DE> + Clone + Send + Sync + 'static,
    DE: Into<HttpError>,
    E: Fn(F::Output) -> axum::response::Response + Clone + Send + Sync + 'static,
    P: Send + Sync + 'static,

pub use axum::routing::MethodFilter; // MethodFilter::POST.or(MethodFilter::PUT)

pub struct RawRequest<'a> {
    pub path_params: &'a [(String, String)],
    pub query: Option<&'a str>,
    pub headers: &'a HeaderMap,
    pub body: &'a [u8],
}
```

A decoder's error is anything `Into<HttpError>`; a closure using `?` names its return type (`|raw: RawRequest<'_>| -> Result<Input, DecodeError> { .. }`) so the error type is not left to inference.

```rust
// omnia_sdk::api
pub struct DecodeError { .. }
impl DecodeError {
    pub fn new(description: impl Into<String>) -> Self
    pub fn description(&self) -> &str
}
// omnia_sdk::api::http, re-exported as omnia_sdk::{HttpError, HttpResult}
pub type HttpResult<T, E = HttpError> = Result<T, E>;
pub struct HttpError { .. }
impl HttpError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self
    pub const fn with_body(status: StatusCode, content_type: HeaderValue, body: Vec<u8>) -> Self
}
impl From<DecodeError> for HttpError   // 400, code `invalid_request`
impl From<omnia_sdk::Error> for HttpError   // the error's status, a JSON body
impl From<anyhow::Error> for HttpError   // 500, or the omnia_sdk::Error it wraps
impl IntoResponse for HttpError
```

The router is axum's over the client state: `axum::Router<Client<P>>`, built with `Router::new()`, `.route(path, method_router)`, and finished once by `.with_state(Client::new(owner, provider))`, which the root's route list does; a module builds no router of its own. The crate re-exports `axum`, `http`, `bytes`, `http_body`, `anyhow`, and `tracing` at its root, so a module names them as `omnia_sdk::axum::..` or through its own stem's manifest.

## Messaging — `omnia_sdk::api::messaging`

An exact-topic router over the same handlers, which the root's topic list builds and the guest's messaging export dispatches each delivery through:

```rust
pub struct Router<P: Send + Sync + 'static> { .. }
impl<P: Send + Sync + 'static> Router<P> {
    pub fn new(client: Client<P>) -> Self
    pub fn route<F, I, D>(self, topic: impl Into<String>, consume: Consume<F, I, D>) -> Self
    where
        F: Handler<P, I>,
        I: Send + 'static,
        D: Fn(&Delivery) -> Result<I, DecodeError> + Send + Sync + 'static,
    // panics on an empty topic, or one already registered
    pub async fn handle(&self, delivery: Delivery) -> Result<(), DeliveryError>
}

pub fn consume<F, I: DeserializeOwned>(handler: F) -> Consume<F, I, impl Fn(&Delivery) -> Result<I, DecodeError>>
    // the payload decoded as JSON into `I`
pub fn consume_with<F, I, D>(handler: F, decode: D) -> Consume<F, I, D>
where
    D: Fn(&Delivery) -> Result<I, DecodeError> + Send + Sync + 'static,

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
```

- The handler of a topic is a `Handler<P, I>` as a route's is: `pub async fn on_placed<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()>`, its output dropped once it answers `Ok`. A failure is `DeliveryError::Rejected` with the error displayed, which the host reads as a failed delivery.
- `handle` matches the delivery's topic exactly, so the list registers each topic under its qualified name, `config::topic_for(&env, <suffix>)`, and a delivery from another environment is `UnhandledTopic`.
- The delivery's `metadata` fills the handler's `Metadata`: `request-id`, `correlation-id`, and `causation-id`, matched case-insensitively, a request id minted where none arrived.
- `omnia_sdk::api::messaging::handle(&router, message)` is the `wasm32` adapter from the WIT message to a `Delivery`, which the guest's export calls and nothing else does.

## The handler — `omnia_sdk::api`

```rust
pub trait Handler<P, I>: Clone + Send + Sync + 'static {
    type Output: Send;
    type Error: std::error::Error + Send + Sync + 'static;
    fn call(self, input: I, context: Context<P>)
        -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}
// the blanket impl: every `async fn(I, Context<P>) -> Result<O, E>` is a Handler
impl<F, Fut, P, I, O, E> Handler<P, I> for F
where
    F: FnOnce(I, Context<P>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<O, E>> + Send,
    O: Send,
    E: std::error::Error + Send + Sync + 'static,

pub struct Context<P> {
    pub metadata: Metadata,
    ..
}
impl<P> Context<P> {
    pub fn new(owner: impl Into<String>, provider: P, metadata: Metadata) -> Self
    pub fn owner(&self) -> &str
    pub fn provider(&self) -> &P
}

pub struct Metadata {
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub deadline: Option<SystemTime>,
}
impl Metadata {
    pub fn minted(request_id: String) -> Self
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self
}
impl Default for Metadata

pub struct Client<P> { .. }
impl<P: Send + Sync + 'static> Client<P> {
    pub fn new(owner: impl Into<String>, provider: P) -> Self
    pub fn owner(&self) -> &str
    pub fn provider(&self) -> &P
    pub async fn call<F, I>(&self, handler: F, input: I, metadata: &Metadata) -> Result<F::Output, F::Error>
    where
        F: Handler<P, I>,
}
impl<P: Send + Sync + 'static> Clone for Client<P>
```

`Client::new(owner, provider).call(handler, input, &Metadata::default()).await` is how a test calls a handler with no router: the call builds the handler's `Context` over the client's provider and runs it under a `handler` span, as the two routers do.

## Errors — `omnia_sdk`

```rust
pub type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    BadRequest { code: String, description: String },   // 400
    NotFound { code: String, description: String },     // 404
    ServerError { code: String, description: String },  // 500
    BadGateway { code: String, description: String },   // 502
}
impl Error {
    pub const fn status(&self) -> StatusCode
    pub fn code(&self) -> String
    pub fn description(&self) -> String
}
impl From<anyhow::Error> for Error   // the chain joined into a server_error, or the Error it wraps

// each takes a format string and arguments, or one expression spelled with `to_string`
bad_request!("..")    // code "bad_request"
not_found!("..")      // code "not_found"
server_error!("..")   // code "server_error"
bad_gateway!("..")    // code "bad_gateway"
```

A provider call answers `anyhow::Result<T>`; `?` on it inside an `omnia_sdk::Result` handler converts through `From<anyhow::Error>`.

## The capabilities — `omnia_sdk`

Every trait is re-exported at the crate root, every method answers `anyhow::Result` through `impl Future<Output = ..> + Send`, and the production provider implements each over WASI on `wasm32` with an empty `impl`. The nine the layout's `Capabilities` bound names:

```rust
pub trait Config: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<String>> + Send;
}

pub trait StateStore: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;
    fn set(&self, key: &str, value: &[u8], ttl_secs: Option<u64>)
        -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;   // the value replaced
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
    fn cas(&self, key: &str, expected: Option<&[u8]>, value: &[u8])
        -> impl Future<Output = Result<(), CasError>> + Send;
    fn increment(&self, key: &str, delta: i64) -> impl Future<Output = Result<i64>> + Send;
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
}
pub struct ObjectMetadata { pub name: String, pub container: String, pub created_at: u64, pub size: u64 }
pub struct ContainerMetadata { pub name: String, pub created_at: u64 }
pub trait BlobStoreExt: BlobStore {   // blanket, over every BlobStore
    fn has(&self, container: &str, name: &str) -> impl Future<Output = Result<bool>> + Send;
    fn delete_objects(&self, container: &str, names: &[String]) -> impl Future<Output = Result<()>> + Send;
    fn clear(&self, container: &str) -> impl Future<Output = Result<()>> + Send;
    fn copy_object(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
    fn move_object(&self, src_container: &str, src_name: &str, dest_container: &str, dest_name: &str)
        -> impl Future<Output = Result<()>> + Send;
}

pub trait DocumentStore: Send + Sync {
    fn get(&self, store: &str, id: &str) -> impl Future<Output = Result<Option<Document>>> + Send;
    fn insert(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> + Send;
    fn put(&self, store: &str, doc: &Document) -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, store: &str, id: &str) -> impl Future<Output = Result<bool>> + Send;
    fn query(&self, store: &str, options: QueryOptions) -> impl Future<Output = Result<QueryResult>> + Send;
}
// omnia_sdk::document_store
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
    fn query(&self, conn_name: String, query: String, params: Vec<DataType>)
        -> impl Future<Output = Result<Vec<Row>>> + Send;
    fn exec(&self, conn_name: String, query: String, params: Vec<DataType>)
        -> impl Future<Output = Result<u32>> + Send;   // rows affected
}
// omnia_wasi_sql, a crate of the stem's manifest at the pinned version
pub struct Row { pub index: String, pub fields: Vec<Field> }
pub struct Field { pub name: String, pub value: DataType }
pub enum DataType {
    Int32(Option<i32>), Int64(Option<i64>), Uint32(Option<u32>), Uint64(Option<u64>),
    Float(Option<f32>), Double(Option<f64>), Str(Option<String>), Boolean(Option<bool>),
    Date(Option<String>), Time(Option<String>), Timestamp(Option<String>), Binary(Option<Vec<u8>>),
}

pub trait HttpRequest: Send + Sync {
    fn fetch<T>(&self, request: http::Request<T>) -> impl Future<Output = Result<http::Response<Bytes>>> + Send
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;
}
// `axum::body::Body` is such a `T`: `Request::get(url).body(Body::empty())`, `Body::from(bytes)`

pub trait Identity: Send + Sync {
    fn access_token(&self, identity: String) -> impl Future<Output = Result<String>> + Send;
}

pub trait Publish: Send + Sync {
    fn send(&self, topic: &str, message: &Message) -> impl Future<Output = Result<()>> + Send;
}
pub struct Message { pub payload: Vec<u8>, pub headers: HashMap<String, String> }
impl Message {
    pub fn new(payload: &[u8]) -> Self
}

pub trait Broadcast: Send + Sync {
    fn send(&self, name: &str, data: &[u8], sockets: Option<Vec<String>>)
        -> impl Future<Output = Result<()>> + Send;   // None reaches every subscriber
}
```

A `cas` answers `Result<(), CasError>` rather than `anyhow::Result`, so a handler matches on `CasError::Conflict` for a retry or a `409` and maps `CasError::Store` to a `server_error!`.

## The doubles — `omnia_test::guest`

`Provider` answers all nine capabilities natively and is `Clone` and `Default`; each double is set with the builder method of its name and read back through the field of the same name:

```rust
pub struct Provider {
    pub config: MapConfig,
    pub http: MatchedHttp,
    pub identity: FixedIdentity,
    pub publish: Sink,
    pub broadcast: Sink,
    pub storage: Memory,       // StateStore and BlobStore
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
    pub fn asked(&self) -> Vec<String>   // every identity an access token was asked for
}

impl Sink {   // Default: records every send
    pub fn sent(&self) -> Vec<(String, Message)>   // Publish: (topic, message)
    pub fn broadcasts(&self) -> Vec<Broadcasted>   // Broadcast
}
pub struct Broadcasted { pub name: String, pub data: Vec<u8>, pub sockets: Option<Vec<String>> }

impl Memory {   // Default: empty; `set` ignores `ttl_secs`
    pub fn state(&self, key: &str) -> Option<Vec<u8>>
    pub fn object(&self, container: &str, name: &str) -> Option<Vec<u8>>
    pub fn objects(&self, container: &str) -> Vec<String>
    pub fn insert_state(&self, key: &str, bytes: &[u8])
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
```

`Provider` and its doubles share their records across clones, so a test clones the provider into the router and reads the original back after the call. How a scenario arranges them is in [testing](testing.md).
