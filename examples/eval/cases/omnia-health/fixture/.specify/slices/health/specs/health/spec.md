# health Specification

## Purpose

Expose a single liveness endpoint that reports the service is up and names itself from configuration.

### Requirement: Health check response

ID: REQ-001
Sources: [intent]
Status: agreed

The system SHALL answer `GET /health` with HTTP 200 and a JSON body containing `status` set to `"ok"` and `service` set to the configured service name.

#### Scenario: Healthy response

- **GIVEN** Config key `SERVICE_NAME` is set to `"health-eval"`
- **WHEN** a client invokes `GET /health`
- **THEN** the response status is 200 and the body is `{"status":"ok","service":"health-eval"}` (camelCase JSON)

#### Scenario: Missing service name

- **GIVEN** Config key `SERVICE_NAME` is absent
- **WHEN** a client invokes `GET /health`
- **THEN** the operation fails with `omnia_guest::Error::ServerError` (stable code `config_missing`) and no success body is returned
