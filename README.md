# Wara

Wara is a self-hosted, Docker-native application platform for running apps on
your own servers. The goal is to provide a practical Coolify/Heroku/Render-style
experience where users can connect Docker hosts, create workspaces and
environments, attach Docker credentials and env vars, deploy Docker images,
Compose files, or Dockerfiles, view logs, restart services, and generate reverse
proxy configuration.

The workspace is early and intentionally contribution-friendly. Backend APIs are in
Rust with Axum, Toasty, PostgreSQL, Utoipa OpenAPI, Temporal workflow scaffolding,
and opt-in platform-only telemetry. The frontend is Angular with ZardUI and
Tailwind.

## What Wara Is Aiming For

Wara should become a self-hosted platform that can:

- Manage one or more Docker servers over SSH.
- Run Docker image, Docker Compose, and Dockerfile based services.
- Support workspaces, environments, services, domains, credentials, env vars,
  templates, and workspace duplication.
- Use Temporal for deploy, restart, proxy, log collection, and template jobs.
- Expose safe, documented APIs for automation.
- Offer an agent-friendly Rust CLI and Rust MCP server for local deploy flows.
- Provide opt-in OpenTelemetry for Wara platform internals only, never hosted
  application telemetry.
- Keep secrets encrypted or hashed at rest and redacted in public responses.

## Repository Layout

- `backend/`: Rust Axum API, Toasty models, Utoipa OpenAPI, auth, services, and
  Temporal worker binary.
- `frontend/`: Angular dashboard using ZardUI and Tailwind utilities.
- `temporal/`: Local Temporal setup matching the workspace conventions.
- `observability/`: Optional local Prometheus, Jaeger, Fluent Bit,
  Elasticsearch, and Grafana stack.
- `deploy/`: Deployment examples such as Nginx config.
- `docs/`: Extended specs and configuration examples.

## Current State

Implemented foundations include:

- Rust Axum API scaffold with `/api/v1`.
- Swagger/OpenAPI docs.
- Toasty/PostgreSQL persistence for core product resources.
- Workspace and environment creation.
- Server records with SSH key-pair metadata and encrypted private key material.
- Services, credentials, env vars, domains, deployments, templates, and workspace
  duplication backed by Toasty.
- DB-backed users with Argon2 password hashes.
- RS256 JWT access tokens.
- Admin user invite flow with one-time hashed invite tokens.
- Basic Angular dashboard and invite acceptance screen.
- Platform telemetry scaffolding.

See [ROADMAP.md](./ROADMAP.md) for the next implementation slices.

## Prerequisites

- Rust 1.95, pinned by `rust-toolchain.toml`.
- Docker and Docker Compose.
- Node.js compatible with Angular 20.
- PostgreSQL for backend integration tests.

## Configuration

Wara reads configuration from environment variables and from
`~/.wara/config.yml`. The config file path can be overridden with
`WARA_CONFIG_FILE`.

Environment variables override config file values. See:

- [.env.example](./.env.example)
- [docs/config.example.yml](./docs/config.example.yml)

JWT access and refresh tokens are signed with the private key configured by
`WARA_JWT_ACTIVE_KEY_ID` and `WARA_JWT_PRIVATE_KEY_PEM`. `WARA_JWT_ACTIVE_KEY_ID`
must be a UUID and is written into each token header as `kid`.
`WARA_JWT_PUBLIC_KEYS` is public key PEM seed material keyed by UUID `id`; on
startup Wara stores public verification keys in the database as JWK components
(`kid`, `kty`, `use`, `alg`, `n`, `e`), not raw PEM.

Clients can fetch the active public key set from:

```text
/.well-known/jwks.json
```

Generate a new local keypair with:

```bash
scripts/generate-jwt-keypair.sh
```

The helper writes PEM files and environment/YAML snippets with restrictive file
permissions. It generates a UUIDv7 key id when one is not provided and does not
print private key material to stdout.

JWT signing key rotation is an operator deployment procedure, not an application
API:

1. Generate a new keypair locally with the helper script.
2. Add the new public key seed to `WARA_JWT_PUBLIC_KEYS` while keeping the old
   active signing key.
3. Restart or roll the backend so every instance stores and can verify old and
   new keys.
4. Set `WARA_JWT_ACTIVE_KEY_ID` and `WARA_JWT_PRIVATE_KEY_PEM` to the new key.
5. Restart or roll the backend again so new access tokens use the new `kid`.
6. Wait for access tokens signed by the old key to expire.
7. Revoke or remove the old public key from the DB/configured key set.
8. Restart or roll the backend one final time.

For development, the default bootstrapped admin is:

- Email: `admin@wara.local`
- Password: `change-me`

Override this before using a shared or exposed environment.

## Run Locally With Docker Compose

```bash
cp .env.example .env
make up
```

Default URLs:

- Frontend: `http://localhost:4200`
- Backend: `http://localhost:8080`
- Swagger UI: `http://localhost:8080/docs`
- OpenAPI JSON: `http://localhost:8080/api/openapi.json`

## Run Backend Locally

Start dependencies:

```bash
make deps-up
```

Run the backend:

```bash
make backend-run
```

Run the Temporal worker:

```bash
make backend-worker
```

Run tests:

```bash
make backend-test
```

Integration tests use `WARA_TEST_DATABASE_URL`. The default in `.env.example`
points at the local compose database.

## Database Migrations

Wara uses Toasty's migration system. Configuration lives in
[backend/Toasty.toml](./backend/Toasty.toml), and the generated migrations,
snapshots, and history live in [backend/toasty/](./backend/toasty). A single
binary, `wara-migrate`, drives everything.

Apply pending migrations:

```bash
make backend-migrate
```

This runs `wara-migrate migration apply` against `DATABASE_URL`. Run it in CI and
production before starting the backend; `docker compose up` runs it automatically
via a one-shot `migrate` service. A fresh database is fully initialized by
`migration apply` — `WARA_DB_PUSH_SCHEMA` is not required.

`WARA_DB_PUSH_SCHEMA` (default `false`) is a development-only escape hatch that
lets Toasty regenerate the schema directly while iterating on models. It bypasses
migrations and is not a production workflow.

When you change a Toasty model, generate a new migration from the model diff and
commit the resulting files in `backend/toasty/`:

```bash
make backend-migrate-generate NAME=describe_change
```

Existing databases created with `WARA_DB_PUSH_SCHEMA` have no Toasty migration
history, so `migration apply` will not adopt them; recreate the database (or use
`wara-migrate migration reset`) since development databases are disposable.

## Run Frontend Locally

```bash
cd frontend
npm install
npm start
```

Or from the repository root:

```bash
make frontend-install
make frontend-start
```

Build:

```bash
cd frontend
npm run build
```

Or:

```bash
make frontend-build
```

## Make Commands

Run `make help` from the repository root to see common commands for Docker
Compose, Rust, Angular, version checks, formatting, linting, builds, and local
validation.

Focused validation targets are available for separately tracked components:

- `make cli-validate`
- `make mcp-validate`
- `make docs-validate`

## API Usage

Swagger is available at `/docs` when `WARA_DOCS_ENABLED=true`. Public API routes
are versioned under `/api/v1`.

Current auth flow:

1. Login with `POST /api/v1/auth/login`.
2. Use the returned JWT access token as `Authorization: Bearer <token>`.
3. Store the returned refresh JWT securely on the client. Its `jti` is the
   refresh-token record id.
4. Refresh sessions with `POST /api/v1/auth/refresh`; this returns a new access
   token and a new refresh token, and revokes the refresh token that was used.
5. Logout with `POST /api/v1/auth/logout` to revoke the active refresh token.
6. Admins can invite users with `POST /api/v1/admin/users`.
7. Invited users open `/accept-invite?token=...` and create a password.

Refresh token plaintext is returned only by login, invite acceptance, and
refresh responses. Listing, revocation, and error responses must not expose it.

## Contributing

Start with:

- [ROADMAP.md](./ROADMAP.md)
- [DESIGN.md](./DESIGN.md)
- [PR_GUIDE.md](./PR_GUIDE.md)

Keep contributions small and complete. A good PR should implement one roadmap
item end to end, including tests and documentation updates where relevant.

## Versioning

Wara uses separate versions for independently released components:

- Core platform images use [wara.version.toml](./wara.version.toml), and the
  backend/frontend package versions must match it.
- CLI versions live in [cli/Cargo.toml](./cli/Cargo.toml).
- MCP server versions live in [mcp/Cargo.toml](./mcp/Cargo.toml).
- Documentation site versions live in [docs/package.json](./docs/package.json).

Core platform releases use `vMAJOR.MINOR.PATCH` tags. Independent components use
namespaced tags such as `cli-vMAJOR.MINOR.PATCH`,
`mcp-vMAJOR.MINOR.PATCH`, and `docs-vMAJOR.MINOR.PATCH`.

Beta versions use `MAJOR.MINOR.PATCH-beta-N` and are allowed only on `dev`.
Stable and other prerelease types can be used on both `main` and `dev`.

Core platform version tags trigger Docker image publishing to GitHub Container
Registry.

## License

Wara is licensed under the [Apache License 2.0](./LICENSE).
