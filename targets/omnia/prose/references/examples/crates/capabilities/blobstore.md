# BlobStore Operation Patterns

This document covers the `BlobStore` trait pattern used for binary object storage operations in Omnia business logic crates. `BlobStore` is backed by `omnia_wasi_blobstore` and covers Azure Blob Storage, AWS S3, and other object storage services.

**Demonstrates:** `BlobStore` and `Config` capability traits

## Overview

The `BlobStore` trait provides operations for storing, retrieving, and managing binary objects (blobs) organized in containers. Use `BlobStore` for file uploads/downloads, report storage, image/media assets, or any binary content addressed by key.

## Trait Definition

```rust
pub trait BlobStore: Send + Sync {
    fn get(&self, container: &str, name: &str)
        -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;
    fn put(&self, container: &str, name: &str, data: &[u8])
        -> impl Future<Output = Result<()>> + Send;
    fn delete(&self, container: &str, name: &str)
        -> impl Future<Output = Result<()>> + Send;
    fn has(&self, container: &str, name: &str)
        -> impl Future<Output = Result<bool>> + Send;
    fn list(&self, container: &str)
        -> impl Future<Output = Result<Vec<String>>> + Send;
    fn get_range(&self, container: &str, name: &str, start: u64, end: u64)
        -> impl Future<Output = Result<Vec<u8>>> + Send;
}
```

For guest code, an empty `impl BlobStore for Provider {}` is sufficient to use the default implementations that connect to WASI BlobStore resources.

## CRUD Patterns

### Write a Blob

```rust
let data = serde_json::to_vec(&report).context("serializing report")?;
BlobStore::put(provider, &container, &key, &data).await?;
```

### Read a Blob (full)

```rust
let bytes = BlobStore::get(provider, &container, &key)
    .await?
    .ok_or_else(|| bad_request!("blob not found: {key}"))?;
let report: Report = serde_json::from_slice(&bytes)
    .context("deserializing report")?;
```

### Check Existence

```rust
if BlobStore::has(provider, &container, &key).await? {
    tracing::info!("blob already exists: {key}");
}
```

### Delete a Blob

```rust
BlobStore::delete(provider, &container, &key).await?;
```

### List Objects in a Container

```rust
let keys = BlobStore::list(provider, &container).await?;
tracing::info!("container has {} objects", keys.len());
```

## Complete Operation Examples

### Upload Blob Operation

```rust
use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;
use anyhow::Context as _;
use omnia_guest::{bad_request, BlobStore, Config, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct UploadRequest {
    pub name: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadResponse {
    pub key: String,
}

async fn upload_blob<P: Config + BlobStore>(
    _owner: &str, provider: &P, req: UploadRequest,
) -> Result<UploadResponse> {
    if req.name.trim().is_empty() {
        return Err(bad_request!("name cannot be empty"));
    }
    if req.data.is_empty() {
        return Err(bad_request!("data cannot be empty"));
    }

    let container = Config::get(provider, "BLOB_CONTAINER")
        .await
        .context("getting BLOB_CONTAINER")?;

    BlobStore::put(provider, &container, &req.name, &req.data)
        .await
        .context("writing blob")?;

    Ok(UploadResponse { key: req.name })
}

pub struct UploadRequestOperation;

impl<P: omnia_guest::api::Provider + Config + BlobStore> Operation<P> for UploadRequestOperation
{
    type Error = Error;
    type Input = UploadRequest;
    type Output = UploadResponse;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> Result<Self::Output> {
        // Structural validation is the first step; omit only when no checks apply.
        upload_blob(context.owner, context.provider, input).await
    }
}
```

### Download Blob Operation

```rust
use omnia_guest::api::invoke::CallContext;
use omnia_guest::api::operation::Operation;
#[derive(Clone, Debug, Deserialize)]
pub struct DownloadRequest {
    pub name: String,
}

async fn download_blob<P: Config + BlobStore>(
    _owner: &str, provider: &P, req: DownloadRequest,
) -> Result<Vec<u8>> {
    let container = Config::get(provider, "BLOB_CONTAINER")
        .await
        .context("getting BLOB_CONTAINER")?;

    BlobStore::get(provider, &container, &req.name)
        .await
        .context("reading blob")?
        .ok_or_else(|| bad_request!("Blob not found: {}", req.name))
}

pub struct DownloadRequestOperation;

impl<P: omnia_guest::api::Provider + Config + BlobStore> Operation<P> for DownloadRequestOperation
{
    type Error = Error;
    type Input = DownloadRequest;
    type Output = Vec<u8>;

    async fn call(
        input: Self::Input,
        context: CallContext<'_, P>,
    ) -> Result<Self::Output> {
        // Structural validation is the first step; omit only when no checks apply.
        download_blob(context.owner, context.provider, input).await
    }
}
```

### List and Cleanup Operation

```rust
#[derive(Clone, Debug, Serialize)]
pub struct ListResponse {
    pub objects: Vec<String>,
    pub count: usize,
}

async fn list_blobs<P: Config + BlobStore>(
    _owner: &str, provider: &P,
) -> Result<ListResponse> {
    let container = Config::get(provider, "BLOB_CONTAINER")
        .await
        .context("getting BLOB_CONTAINER")?;

    let objects = BlobStore::list(provider, &container)
        .await
        .context("listing blobs")?;
    let count = objects.len();

    Ok(ListResponse { objects, count })
}
```

## Required Imports

```rust
// BlobStore trait
use omnia_guest::BlobStore;

// SDK types
use omnia_guest::{bad_request, Config, Error, Result};

// Other common imports
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
```

## Key Rules

1. **Target Architecture**: BlobStore operations are designed for `wasm32-wasip2` only
2. **Range reads**: Use `get` for full reads and `get_range` with inclusive `start` and `end` byte offsets for partial reads
3. **Config for container name**: Get container/bucket name from `Config` trait
4. **Validation first**: Validate input (non-empty name, non-empty data) before performing blob operations
5. **Error mapping**: Map blob errors to `omnia_guest::Error` with context
6. **Binary data**: Blob `data` is `&[u8]` / `Vec<u8>` — no serialization format is assumed; use `serde_json::to_vec` for JSON, raw bytes for images/files

## Choosing Between Storage Traits

| Data Shape | Trait | When |
|------------|-------|------|
| Binary blobs by key | `BlobStore` | Files, images, large payloads, opaque binary data |
| JSON documents by key/query | `DocumentStore` | Azure Table Storage, Cosmos DB documents, MongoDB, flexible schema |
| Tabular rows, SQL queries | `TableStore` | Relational data, SQL CRUD |
| Small key-value cache entries | `StateStore` | Redis cache, session state, TTL-based expiry |

## References

- See [../../references/capabilities.md](../../../capabilities.md) for trait definitions
- See [../../references/providers.md](../../../providers/README.md) for provider bound composition
- See [../../references/error-handling.md](../../../error-handling.md) for error conventions
