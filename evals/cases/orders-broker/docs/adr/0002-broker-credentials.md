# 2. Broker credentials live in the secret store

## Context

The broker is reached with a key and a secret that rotate on a schedule the
platform team sets, and the service runs in several environments, each with
credentials of its own. No credential may be committed or passed in the
environment.

## Decision

Keep the broker credentials in Azure Key Vault, one secret per environment,
read by the service when it starts.

## Consequences

The service obtains the broker credentials before any broker client is
constructed.

The service refuses to start without the broker credentials.

When the credentials rotate, the service restarts so that the next start
reads the new ones.
