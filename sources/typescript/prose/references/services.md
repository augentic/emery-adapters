# External services and publications

The stores, brokers, caches, and identity providers a surface reaches, and what it publishes. Each is reached through a client; each use is a `call` claim at the call site, and what is written, published, or read, and when, is a `requirement`.

## Classify the service

Name the technology and the kind of service, from the client the source uses:

- **database** — SQL through an ORM, a query builder, or raw SQL (`pg`, `mysql2`, `typeorm`, `prisma`, `knex`).
- **managed table store** — a cloud table store through its SDK (`@azure/data-tables` `TableClient`, DynamoDB `DocumentClient`, Cosmos DB's table API). A store reached over HTTP by its SDK is a store, not an outbound API.
- **document store** — a document database through its client (`mongodb` `MongoClient`, Cosmos DB's document API: `find`, `insertOne`, `updateOne`).
- **blob store** — object storage through its client (`@azure/storage-blob` `BlobServiceClient` / `ContainerClient`, `@aws-sdk/client-s3` `S3Client` with `PutObject` / `GetObject`); record the container or bucket and the key pattern as constructed.
- **cache** — Redis, Memcached, an in-memory cache with a TTL.
- **message broker** — Kafka, RabbitMQ, Azure Service Bus, SQS, a pub/sub client.
- **identity provider** — Azure AD, Auth0, an OAuth token endpoint.
- **API** — another service over HTTP; see [Outbound HTTP calls](external-api.md).
- **WebSocket** — a socket the surface opens or serves.

Record what the source shows and no more: connection configuration by its exact key (`DATABASE_URL`, `KAFKA_BROKER`), the operations performed (select, insert, update, delete; get, set, delete; publish, subscribe; token acquisition), the entity or table and its key columns, the key pattern, the topic.

## Publications

For each publish, read from the code:

- **Topic or queue** as constructed — `${env}-${TOPIC}` is not `TOPIC`.
- **Count** — from the loop bounds: `for (let i = 0; i < 2; i++)` publishes 2 times.
- **Delay placement** — `sleep(5s); publish all`, repeated, is not `publish; sleep(5s); publish`.
- **Payload** — its type (a `type` claim), and whether it is identical each round or changes (a timestamp, a sequence number).
- **Metadata** — the partition or routing key (`message.key = externalId`), headers, content type.
- **Ordering with the response** — whether the caller's response waits for the publish.

Each fact a consumer or an operator observes is a requirement: `After enrichment the processor publishes the enriched event to events-topic, waits 5 seconds, and publishes an audit record carrying the metadata to audit-topic.`
