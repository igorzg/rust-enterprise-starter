# Rust enterprise starter v1.0

A production-ready Rust REST API starter with hexagonal architecture (a standalone core plus HTTP and persistence adapters), 
PostgreSQL persistence, Redis cache-aside, OpenAPI/Swagger documentation, Docker Compose DEV deployment, and a dedicated one-shot migration service.

## Contents

- [Overview](#overview)
- [Quick Start](#quick-start) — requirements and the commands to run it
- [Tech Stack](#tech-stack)
- [Project Structure](#project-structure)
- [Commands](#commands)
- [Configuration](#configuration)
- [Development](#development)
- [Production Deployment](#production-deployment)
- [Migrations](#migrations)
- [API](#api)
- [Swagger / OpenAPI](#swagger--openapi)
- [Testing](#testing)
- [Logging](#logging)
- [Metrics](#metrics)
- [Internationalization](#internationalization)
- [Security Defaults](#security-defaults)
- [Graceful Shutdown](#graceful-shutdown)
- [Architecture](#architecture)

## Overview

A structured backend that can be deployed to production.

It provides:

- A **User domain** with `POST /api/v1/users`, `GET /api/v1/users/{id}`, and `DELETE /api/v1/users/{id}`
- **Cache-aside reads** (Redis first, PostgreSQL on miss, TTL-based) with cache invalidation on delete
- **Dedicated migration service** (SQLx CLI) that runs before the app starts — the app never runs migrations
- **Structured logging** (pretty or JSON), health/readiness probes, CORS, body limits, gzip, graceful shutdown
- A **hermetic test suite** (in-memory fakes) that runs without PostgreSQL or Redis
- **Production-like Docker Compose** stacks (dev and prod) with named volumes and healthchecks

## Quick Start

### Requirements

- **Rust** (stable, `rust-version = "1.85"`) + **Cargo**
- **Docker** + **Docker Compose** (v2)
- **Make**

Optional: **SQLx CLI** (ad-hoc local migrations) and **cargo-watch** (for `make run-dev` live reload). See [Prerequisites](#prerequisites) for the full list.

### Run the full stack with Docker

```bash
make up        # build + start postgres, redis, migrate (one-shot), and the app
make smoke     # end-to-end check against http://localhost:8080
```

No `.env` is required — the Compose file supplies defaults and points `DATABASE_URL`/`REDIS_URL` at the service names. The API is served on `http://localhost:8080`, with Swagger UI at `http://localhost:8080/swagger-ui`.

### Run natively (development)

```bash
cp .env.bak .env        # DATABASE_URL / REDIS_URL point at localhost (the defaults already do)
make up-dev             # start PostgreSQL + Redis + migrate (infrastructure only)
make run                # run the app natively, loading .env  (or `make run-dev` for watch)
```

See [Development](#development) and [Configuration](#configuration) for details.

## Tech Stack

| Concern            | Choice                                          |
|--------------------|-------------------------------------------------|
| Language           | Rust (stable, edition 2024)                     |
| Async runtime      | Tokio                                           |
| HTTP framework     | Axum (tower / tower-http middleware)            |
| Database           | PostgreSQL via SQLx (no ORM)                    |
| Cache              | Redis via the `redis` crate (async)             |
| Serialization      | Serde / serde_json                              |
| OpenAPI / Swagger  | utoipa + utoipa-swagger-ui                      |
| Errors             | thiserror (core) / anyhow (composition)         |
| Logging            | tracing + tracing-subscriber (pretty or JSON)   |
| Metrics            | `metrics` + `metrics-exporter-prometheus`       |
| Internationalization | rust-i18n (compile-time locales, per-request `Accept-Language`) |
| Containers         | Docker multi-stage build + Docker Compose       |
| Migrations         | SQLx CLI, dedicated one-shot `migrate` service  |

## Project Structure

```
.
├── Cargo.toml
├── Dockerfile
├── Makefile                  # cargo, docker, migration, and smoke targets
├── docker-compose.yml          # production-like stack (app + PostgreSQL + Redis + migrate)
├── docker-compose.dev.yml      # dev infrastructure: PostgreSQL + Redis + migrate (app runs natively)
├── .env.bak
├── migrations/
│   └── 0001_create_users.sql   # applied by the `migrate` service only
├── locales/
│   ├── en.yml                  # English (fallback) error messages
│   └── de.yml                  # German example
├── src/
│   ├── main.rs                 # entrypoint: config → logging → state → serve
│   ├── lib.rs                  # crate root (enables tests/ integration tests)
│   ├── config/mod.rs           # env-based configuration, fail-fast validation
│   ├── state.rs                # composition root
│   ├── services/
│   │   ├── domain/
│   │   │   └── user.rs         # User domain model + CreateUserCommand
│   │   ├── ports.rs              # UserRepository, UserCache, HealthCheck
│   │   ├── errors.rs             # AppError + error enums
│   │   └── user_service.rs
│   ├── persistence/
│   │   ├── database/
│   │   │   ├── domain/
│   │   │   │   └── user.rs     # User entity (adapter-internal)
│   │   │   └── postgres/       # helpers + PostgresUserRepository
│   │   ├── cache/
│   │   │   ├── domain/
│   │   │   │   └── user.rs     # CachedUser (cache serialization shape)
│   │   │   └── redis/          # RedisUserCache
│   │   └── health.rs           # SystemHealthCheck (PG + Redis probes)
│   └── api/
│       ├── routes.rs           # router, CORS, limits, compression, metrics, 404
│       ├── error.rs            # ApiError → HTTP status + localized JSON envelope
│       ├── i18n.rs             # Accept-Language locale resolution
│       ├── metrics.rs          # Prometheus recorder, /metrics, RED middleware
│       ├── openapi.rs          # ApiDoc (utoipa)
│       ├── dto/                # request/response types (requests/ + responses/)
│       └── handlers/           # user_handler, health
└── tests/
    ├── common/mod.rs           # in-memory fakes with counters
    ├── user_service.rs         # service tests (business rules, cache-aside)
    ├── users_api.rs            # full-router API tests
    ├── config.rs               # env parsing (CORS origin validation)
    ├── error_logging.rs        # 5xx log chain + sanitized envelope
    ├── i18n.rs                 # locale resolution + localization tests
    └── integration.rs          # real PostgreSQL + Redis (skipped when env is unset)
```

## Commands

All common operations are Makefile targets; `make help` lists them with descriptions.

- **Cargo** — `build`, `build-release`, `run` (loads `.env`), `run-dev` (watch mode), `check`, `test`, `fmt`, `fmt-check`, `clippy` (warnings denied), `doc`, `clean`, and `verify` (the full quality gate: format, check, clippy, test)
- **Docker** — `up` / `up-dev` (build + start), `down` / `down-dev`, `reset-dev` (down + remove volumes), `logs` / `logs-dev`, `ps` / `ps-dev`, `build-images`
- **Migrations & validation** — `migrate` / `migrate-dev` (run the one-shot `migrate` service), `smoke` (end-to-end test against a running stack, `BASE_URL` overridable)

## Configuration

All configuration comes from environment variables (see `.env.bak`). The app **fails fast** at startup when required variables are missing or invalid.

| Variable               | Required | Default     | Description                                          |
|------------------------|----------|-------------|------------------------------------------------------|
| `APP_HOST`             | no       | `0.0.0.0`   | Bind address                                         |
| `APP_PORT`             | no       | `8080`      | Bind port                                            |
| `DATABASE_URL`         | **yes**  | —           | PostgreSQL connection string                         |
| `REDIS_URL`            | **yes**  | —           | Redis connection string                              |
| `RUST_LOG`             | no       | `info`      | Log filter (`info`, `debug`, `warn`, per-target…)    |
| `LOG_FORMAT`           | no       | `pretty`    | `pretty` or `json`                                   |
| `CORS_ALLOWED_ORIGINS` | no       | *(empty)*   | Comma-separated origins; empty = CORS disabled, `*` = any; invalid entries fail at startup |
| `CACHE_TTL_SECS`       | no       | `300`       | Cache-aside TTL in seconds                           |
| `POSTGRES_USER`        | no       | `app`       | Used by the Compose files                            |
| `POSTGRES_PASSWORD`    | no       | `app`       | Used by the Compose files                            |
| `POSTGRES_DB`          | no       | `app`       | Used by the Compose files                            |
| `POSTGRES_PORT`        | no       | `5432`      | Host-published port (dev compose only)               |
| `REDIS_PORT`           | no       | `6379`      | Host-published port (dev compose only)               |

Inside Docker, `DATABASE_URL` and `REDIS_URL` point at the **service names** (`postgres`, `redis`) — never `localhost`, which would refer to the container itself.

## Development

```bash
# 1. Configure local environment
cp .env.bak .env

# 2. Start the dev infrastructure stack (PostgreSQL + Redis + migrate)
make up-dev

# 3. Run the app natively in a second terminal (loads .env)
make run
```

For live reload, run `make run-dev` instead — it wraps `cargo run` in `cargo-watch`, rebuilding and restarting on every file save.

The dev stack is **infrastructure only** — there is no app container. You develop against a natively running `cargo run` (fast iteration, direct debugger/REPL access) while Compose provides PostgreSQL, Redis, and the one-shot migrations. PostgreSQL/Redis ports are published to the host so you can inspect them directly:

```bash
psql "postgres://app:app@localhost:5432/app"
redis-cli -p 6379
```

`.env` points `DATABASE_URL` and `REDIS_URL` at `localhost` for this native workflow; the Compose files always override them with service-name URLs inside Docker. For debug logs, use `RUST_LOG=debug make run`.

Useful commands:

```bash
make logs-dev      # follow the dev stack logs
make migrate-dev   # run migrations manually
make smoke         # end-to-end smoke test
```

The dev stack is fully reset with:

```bash
make reset-dev     # removes the postgres_data/redis_data volumes
```

## Production Deployment

```bash
make up            # = docker compose up --build -d
```

```mermaid
flowchart LR
    subgraph COMPOSE["docker compose — production-like stack"]
        APP["app (Axum, rstarter:latest)"]
        MIG["migrate (sqlx CLI, one-shot)"]
        PG[("postgres:16-alpine")]
        RD[("redis:7-alpine")]
        APP -->|"SQLx"| PG
        APP -->|"redis"| RD
        MIG -->|"sqlx migrate run"| PG
    end
    C(["clients"])
    C -->|"HTTP :8080"| APP
```

Startup sequence (enforced by Compose `depends_on` conditions):

1. `postgres` starts and becomes healthy (`pg_isready`)
2. `redis` starts and becomes healthy (`redis-cli ping`)
3. `migrate` runs `sqlx migrate run` **once** and exits `0` (non-zero on failure)
4. `app` starts — only after `migrate` completed successfully (`condition: service_completed_successfully`) — and binds `0.0.0.0:8080`

If any migration fails, `migrate` exits non-zero and the app **never starts**. There are no partial-schema half-states for the app to run against.

### Kubernetes / orchestrator notes

- Liveness probe: `GET /health` (process alive, no dependency checks — safe during DB outages)
- Readiness probe: `GET /ready` (200 only when PostgreSQL **and** Redis respond; 503 otherwise — traffic is drained automatically)
- The container runs as non-root and handles `SIGTERM` for graceful drain.

## Migrations

- **Location:** `migrations/000N_verb_noun.sql` (e.g. `0001_create_users.sql`)
- **Owner:** the dedicated one-shot `migrate` service (SQLx CLI image), present in **both** `docker-compose.yml` and `docker-compose.dev.yml`. The application binary does **not** run migrations at startup and does **not** embed them.
- **Immutability:** never edit an applied migration. Add a new numbered file instead. SQLx records applied migrations in the `_sqlx_migrations` history table.
- **Column bookkeeping:** `users.updated_at` is maintained by the `users_set_updated_at` trigger (defined in `0001_create_users.sql`) — application code never sets it.
- **Applying manually:** `make migrate` / `make migrate-dev`, or `docker compose -f <file> run --rm migrate`, or the SQLx CLI directly against a local PostgreSQL.
- **Failure behavior:** a failing migration makes the `migrate` service exit non-zero, which blocks the app from starting (`service_completed_successfully`).
- **Production strategy:** in a multi-replica deployment, run migrations as a dedicated step (a job/step in CI-CD, or a single `migrate` container) **before** rolling out new app replicas — never inside app containers.

## API

Base path: `/api/v1`

| Method   | Path             | Description                        | Success | Errors          |
|----------|------------------|------------------------------------|---------|-----------------|
| `POST`   | `/api/v1/users`  | Create a user                      | 201     | 400, 409, 500   |
| `GET`    | `/api/v1/users/{id}` | Fetch a user (cache-aside)    | 200     | 400, 404, 500   |
| `DELETE` | `/api/v1/users/{id}` | Delete a user (+ cache invalidation) | 204 | 400, 404, 500   |

### Examples

```bash
# Create
curl -X POST http://localhost:8080/api/v1/users \
  -H 'content-type: application/json' \
  -d '{"name": "Jane Doe", "email": "jane.doe@example.com"}'
# → 201 {"id":"550e8400-e29b-41d4-a716-446655440000","name":"Jane Doe","email":"jane.doe@example.com"}

# Read (first call: DB + cache fill; second call: Redis)
curl http://localhost:8080/api/v1/users/550e8400-e29b-41d4-a716-446655440000

# Delete (invalidates the Redis entry)
curl -X DELETE http://localhost:8080/api/v1/users/550e8400-e29b-41d4-a716-446655440000
# → 204

# Read after delete
curl http://localhost:8080/api/v1/users/550e8400-e29b-41d4-a716-446655440000
# → 404
```

### Error envelope

All errors share the same JSON shape. `5xx` responses never leak internal details (SQL errors, connection strings); the full detail is only in server logs.

```json
{"error": {"code": "NOT_FOUND", "message": "user with id 550e8400-e29b-41d4-a716-446655440000 was not found"}}
```

Codes: `VALIDATION_ERROR` (400), `NOT_FOUND` (404), `CONFLICT` (409, duplicate email), `DATABASE_ERROR` / `CACHE_ERROR` (500).

## Swagger / OpenAPI

- **Swagger UI:** `http://localhost:8080/swagger-ui`
- **OpenAPI JSON:** `http://localhost:8080/openapi.json`

Both are generated from the code via utoipa (sourced from the handler attributes), so they stay in sync with the API.

## Testing

```bash
make test        # = cargo test
```

The test suite is **hermetic**: it runs against in-memory fakes of `UserRepository`, `UserCache`, and `HealthCheck` (with hit/miss counters), so `cargo test` needs **no local PostgreSQL or Redis**.

- `tests/user_service.rs` — business rules (uniqueness conflicts), cache-aside (first GET reads the DB exactly once and fills the cache; second GET is a cache hit), invalidation on delete.
- `tests/users_api.rs` — the full Axum router: request validation (email format, blank names, case/whitespace normalization), health/ready, create/get/delete flows, error envelopes (including sanitized 500s), non-UUID ids, unknown routes.
- `tests/config.rs` — environment parsing: `CORS_ALLOWED_ORIGINS` accepts lists and `*`, rejects invalid entries at startup.
- `tests/error_logging.rs` — a database failure logs the full error source chain (root cause included) while the HTTP envelope stays sanitized.
- `tests/integration.rs` — the real PostgreSQL and Redis adapters: concurrent inserts of the same email (the `ON CONFLICT` race) and the create→read→delete round trip with a TTL check. **Skipped automatically** when `DATABASE_URL` / `REDIS_URL` are unset, so local `cargo test` stays hermetic; CI provides both services and applies the migrations first.

### Integration / migration validation (Docker)

End-to-end validation — migrations on a clean database, real PostgreSQL, real Redis caching — is done against the Docker stack:

```bash
make up                             # fresh stack; migrate runs on a clean DB
make smoke                          # exercises every endpoint + Redis cache behavior
docker compose logs migrate         # migration output
docker compose exec postgres psql -U app -d app -c 'select * from _sqlx_migrations'
docker compose exec redis redis-cli GET user:550e8400-e29b-41d4-a716-446655440000   # cached user JSON + TTL
```

## Logging

- **Filter** via `RUST_LOG` (`info`, `debug`, `warn`, or per-target like `rstarter=debug,tower_http=info`).
- **Format** via `LOG_FORMAT`:
  - `pretty` — single-line human-readable (default)
  - `json` — one JSON object per line, for aggregators (Loki, CloudWatch, ELK)
- HTTP requests are traced (method, URI) by the router's `TraceLayer`.
- Cache failures are logged as `warn` and degrade gracefully (reads fall through to PostgreSQL; write/invalidation failures do not fail the request).

## Metrics

Prometheus metrics are exposed at `GET /metrics` in the text exposition format. The recorder is installed once at startup (`api::metrics::init`, wired in `main.rs`); recording is driven by the [`metrics`](https://docs.rs/metrics) facade and rendered by [`metrics-exporter-prometheus`](https://docs.rs/metrics-exporter-prometheus).

```bash
curl http://localhost:8080/metrics
```

The core (`services`) stays metrics-free — instrumentation lives only in the adapters:

| Metric                            | Type      | Labels                     | Where                       |
|-----------------------------------|-----------|----------------------------|-----------------------------|
| `http_requests_total`             | counter   | `method`, `path`, `status` | `api/metrics.rs` middleware |
| `http_request_duration_seconds`   | histogram | `method`, `path`, `status` | `api/metrics.rs` middleware |
| `http_requests_in_flight`         | gauge     | `method`, `path`           | `api/metrics.rs` middleware |
| `db_queries_total`                | counter   | `operation`                | Postgres adapter            |
| `db_query_duration_seconds`       | histogram | `operation`                | Postgres adapter            |
| `db_errors_total`                 | counter   | `operation`                | Postgres adapter            |
| `cache_operations_total`          | counter   | `operation`, `result`      | Redis adapter               |
| `cache_operation_duration_seconds`| histogram | `operation`                | Redis adapter               |
| `cache_hits_total` / `cache_misses_total` | counter | —                   | Redis adapter               |

The `path` label uses the **route template** (`/api/v1/users/{id}`), not the raw URL, so label cardinality stays bounded as ids change. The `/metrics` scrape itself is excluded from the HTTP counters. Requests matching no route (the JSON 404 fallback) are counted under the bounded label `path="<unmatched>"` — never the raw path, so error probes cannot inflate cardinality.

### Using metrics in code

Instrument any non-core code with the three `metrics` macros — a metric is auto-registered on first use:

```rust
// Counter — count events.
metrics::counter!("orders_created_total", "channel" => "web").increment(1);

// Histogram — observe durations in seconds.
metrics::histogram!("order_processing_seconds", "channel" => "web")
    .record(elapsed.as_secs_f64());

// Gauge — track a current value (in-flight, queue depth, …).
metrics::gauge!("queue_depth").set(42.0);
```

### Extending

**Add a domain metric.** The core must not import `metrics` or Prometheus, so expose a port (like `UserRepository` / `UserCache`) and implement it in an adapter:

```rust
// services/ports.rs — a core-owned port in core vocabulary.
pub trait Metrics {
    fn user_deleted(&self);
}

// persistence — an adapter writing to the recorder.
#[derive(Clone, Default)]
pub struct PrometheusMetrics;

impl Metrics for PrometheusMetrics {
    fn user_deleted(&self) {
        metrics::counter!("users_deleted_total").increment(1);
    }
}
```

Inject `Arc<dyn Metrics>` into `UserService` and call `self.metrics.user_deleted()` in `delete`. The service still depends only on core types.

**Add process/runtime metrics** (CPU, memory, file descriptors) with [`metrics-process`](https://docs.rs/metrics-process): build a `metrics_process::Collector`, call `.describe()` once after `init()`, and call `.collect()` periodically from a spawned task.

**Custom histogram buckets.** Replace the defaults in `api::metrics::init` with `PrometheusBuilder::new().set_buckets(&[…])` (or `set_buckets_for_metric`).

**Separate scrape port.** `/metrics` currently shares the main router with `/health`. To serve it on a dedicated internal-only port (so it is never reachable via the `/api/v1` ingress), bind a second `axum::serve` listener serving only `metrics_handler`.

## Internationalization

Error messages are localized per request with [`rust-i18n`](https://docs.rs/rust-i18n). Locale files in `locales/` are compiled into the binary at build time — no runtime file IO.

The request locale is resolved from the `Accept-Language` header (quality-weighted, most-specific tag first, then its language subtag), falling back to `en`:

```bash
curl -H 'accept-language: de' http://localhost:8080/api/v1/users/550e8400-e29b-41d4-a716-446655440000
# → {"error":{"code":"NOT_FOUND","message":"Benutzer mit ID 550e8400-e29b-41d4-a716-446655440000 wurde nicht gefunden"}}
```

Only `error.message` is localized. `error.code` (`NOT_FOUND`, `CONFLICT`, …) is the stable machine-readable contract and is language-independent.

### Architecture

The core (`services`) never sees a locale. `AppError` carries typed data — `NotFoundError::User { id }`, `ConflictError::EmailAlreadyExists { email }`, `ValidationError::NameLength { max }` — not strings. Translation happens only at the API edge: `api/error.rs` maps each error to a `t!` key and renders it in the request locale (resolved by `api/i18n.rs`).

### Locale files

One YAML file per locale, with `%{name}` placeholders for interpolation:

```yaml
# locales/en.yml
_version: 1
errors:
  user_not_found: "user with id %{id} was not found"
validation:
  name_length: "name must be 1 to %{max} characters"
```

### Adding a locale or a key

- **New locale** — add `locales/<lang>.yml` with the same keys; the loader in `lib.rs` (`rust_i18n::i18n!("locales", fallback = "en")`) picks it up automatically.
- **New key** — add the entry to every locale file, then reference it in `api/error.rs`'s `localized_message` with `t!("key", locale = locale, param = value)`.
- **New typed error** — add a data-only variant to `services/errors.rs`, then a matching arm in `localized_message`.

## Security Defaults

- Non-root container user (uid 10001), minimal Debian runtime image, no Rust tooling shipped
- 1 MiB request body limit; JSON 404 fallback (no stack traces or HTML to clients)
- CORS is **disabled by default**; opt in explicitly with `CORS_ALLOWED_ORIGINS`
- Email uniqueness enforced in the database (`UNIQUE` constraint) as the final backstop
- Secrets (DB/Redis credentials) are supplied via the environment, never baked into images

## Graceful Shutdown

The server listens for `SIGINT` and `SIGTERM`, stops accepting new connections, and drains in-flight requests before exiting — compatible with Docker and k8s stop signals.

## Architecture

The codebase follows hexagonal architecture: a standalone core surrounded by adapters, with every dependency pointing inward at the core. No adapter may depend on another adapter.

```mermaid
flowchart TB
    Client(["HTTP clients"])

    subgraph API["api/ — input adapter"]
        direction TB
        R["Axum routes & middleware<br/>(CORS · body limits · gzip · OpenAPI)"]
        D["Request validation:<br/>XxxRequest → XxxCommand"]
        Z["XxxResponse · error envelope<br/>(ApiError → HTTP status)"]
        R --> D
    end

    subgraph CORE["services/ — the core (the hexagon)"]
        direction TB
        U["UserService — business rules only<br/>(email uniqueness), cache-aside reads"]
        M["Domain model: User<br/>Command: CreateUserCommand<br/>Errors: AppError, CacheError, DatabaseError"]
        P["Output ports (traits owned by the core):<br/>UserRepository · UserCache · HealthCheck"]
    end

    subgraph PERS["persistence/ — output adapters"]
        direction TB
        PG["PostgresUserRepository (SQLx)<br/>User entity ↔ domain mapping at this edge"]
        RD["RedisUserCache (JSON values, TTL)"]
        HC["SystemHealthCheck (PG + Redis probes)"]
    end

    PGD[("PostgreSQL")]
    RDS[("Redis")]

    COMPOSE["state.rs — composition root<br/>wires Arc&lt;dyn Trait&gt; (in-memory fakes in tests)"]

    Client -->|"REST /api/v1 (JSON)"| R
    Z -->|"200/201/204 · 4xx/5xx JSON envelope"| Client
    D -->|"CreateUserCommand"| U
    U -->|"User"| Z
    U --- M
    U -.->|"calls"| P
    P -.->|"expressed in"| M
    PG -- "implements" --> P
    RD -- "implements" --> P
    HC -- "implements" --> P
    PG --> PGD
    RD --> RDS
    HC -.-> PGD
    HC -.-> RDS
    COMPOSE -.->|"wires"| U
    COMPOSE -.->|"wires"| PG
    COMPOSE -.->|"wires"| RD
    COMPOSE -.->|"wires"| HC
```

Model separation — each layer owns its types; mapping happens only at the layer edges:

| Layer          | Models                              | Responsibility                  |
|----------------|-------------------------------------|---------------------------------|
| `api/`         | `XxxRequest` / `XxxResponse`        | HTTP contracts                  |
| `services/`    | `XxxCommand` + domain model         | Business logic & use-cases      |
| `persistence/` | `XxxEntity`                         | Database mapping                |

Key rules:

- **The core is standalone.** `services/` never imports from `api/` or `persistence/` — no adapter types, no infrastructure crates. Both adapters depend only on the core, never on each other.
- **The ports are owned by the core.** `UserRepository`, `UserCache`, and `HealthCheck` are defined in `services/ports.rs` in the core's vocabulary (the domain model, the core error types); the persistence adapters implement them and map entity ↔ domain internally.
- **Handlers contain no SQL and no business logic.** They decode requests, call a service, and encode responses.
- **Services never know the cache is Redis or the DB is PostgreSQL.** They only see `UserCache` and `UserRepository` traits. Redis/SQLx types never leave `persistence/`.
- **Request validation lives in the API layer.** Format, length, and normalization checks run when a request is mapped to its command (`CreateUserRequest::to_command`); services apply business rules only (e.g. email uniqueness).
- **No DI container.** Dependencies are explicit: `Arc<dyn Trait>` fields constructed once in the composition root (`src/state.rs`) and shared via Axum `State`.
- **Entity purity.** `persistence/database/domain/` and `persistence/cache/domain/` have no Axum, SQLx, Redis, or HTTP imports.

### Error handling

Errors are split along the same boundary as models:

| Layer       | Error type                     | Variants                                                     |
|-------------|--------------------------------|--------------------------------------------------------------|
| `api/`      | `ApiError` (HTTP boundary)     | `InvalidBody`, `InvalidId`, `RouteNotFound`, `App(AppError)` |
| `services/` | `AppError` (core error model)  | `Validation`, `NotFound`, `Conflict`, `Database`, `Cache`    |

- **Handlers return `Result<_, ApiError>`.** Body/path rejections map to `InvalidBody` / `InvalidId`; core `AppError` is wrapped as `ApiError::App` via `#[from]`.
- **Request validation lives in the API layer** (`CreateUserRequest::to_command`) but raises the core `AppError::Validation(ValidationError)`: the *check* sits at the edge, the *error type* is core-owned (`services/errors.rs`) and carries locale-agnostic data only.
- **HTTP mapping is API-only.** `AppError::status_code()`, `code()`, and `localized_message()` live in `api/error.rs`; the core never references HTTP status codes or locales.

### Request flow (cache-aside)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant A as api (Axum handler)
    participant S as UserService (core)
    participant R as Redis (UserCache)
    participant P as PostgreSQL (UserRepository)

    Note over C,P: POST /api/v1/users
    C->>A: {"name": ..., "email": ...}
    A->>A: validate + normalize → CreateUserCommand
    A->>S: create(command)
    S->>P: insert(name, email) — uniqueness enforced via ON CONFLICT
    P-->>S: User (id generated) or Conflict
    S->>R: set(User)
    A-->>C: 201 User

    Note over C,P: GET /api/v1/users/{id} — first read (cache miss)
    C->>A: GET
    A->>S: get(id)
    S->>R: get(id)
    R-->>S: None
    S->>P: find_by_id(id)
    P-->>S: User
    S->>R: set(User, TTL)
    A-->>C: 200 User

    Note over C,P: GET /api/v1/users/{id} — second read (cache hit)
    C->>A: GET
    A->>S: get(id)
    S->>R: get(id)
    R-->>S: User
    A-->>C: 200 User (no DB read)

    Note over C,P: DELETE /api/v1/users/{id}
    C->>A: DELETE
    A->>S: delete(id)
    S->>P: delete(id)
    P-->>S: true
    S->>R: delete(id) — invalidate
    A-->>C: 204
```
