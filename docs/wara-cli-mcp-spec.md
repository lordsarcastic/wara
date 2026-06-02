# Agent-Friendly Rust CLI And MCP Server

## Summary

Extend Wara with a Rust API client library, Rust CLI, and Rust MCP server. The CLI must be agent-friendly: every operation is fully controllable with flags/args, supports machine-readable output, has clear progress reporting, includes a `--tree` command overview, exposes direct API access, and returns friendly actionable errors.

The MCP server supports both `stdio` and Streamable HTTP per the 2025-06-18 MCP transport spec.

## Key Changes

- Add workspace crates:
  - `crates/wara-api-client`: shared typed HTTP client for safe `/api/v1` APIs, bearer API-token auth, and consistent error mapping.
  - `crates/wara-cli`: `wara` binary built with `clap`, using the shared client.
  - `crates/wara-mcp`: MCP server binary using `rmcp`.
- Add backend support:
  - Scoped API token CRUD with redacted token display after creation.
  - Artifact upload for local Compose/Dockerfile directories.
  - Deploy endpoint accepting image refs or uploaded artifact IDs.
  - Pollable deployment status endpoint.
- CLI design:
  - All prompts are optional; every input must be available as an arg, flag, env var, or config value.
  - Add `--no-interactive` to fail fast if required input is missing.
  - Add `--output text|json|ndjson`, with `text` default.
  - Add `--quiet`, `--verbose`, `--trace-id`, `--timeout`, and `--wait/--no-wait`.
  - Add `wara --tree` to print the full command/subcommand tree in one call.
  - Add `wara api METHOD PATH [--body JSON|--body-file FILE] [--query key=value]` for direct safe API calls.
  - Progress output must use plain line-oriented output in non-TTY/agent mode, not spinner-only UI.
  - TTY mode may use nicer progress, but must never require select boxes or stdin prompts.
- MCP behavior:
  - `stdio` mode for local subprocess clients.
  - Streamable HTTP mode on `/mcp`, binding to `127.0.0.1` by default.
  - Validate `Origin`, require API-token auth, and honor `MCP-Protocol-Version`.
  - Expose only safe tools: list workspaces/services, deploy image, deploy local artifact, get deployment, get logs, restart service, and preview proxy config.

## CLI Commands

- `wara auth token set --url URL --token TOKEN`
- `wara auth token create --name NAME --scopes ...`
- `wara workspaces list|get|create`
- `wara services list|get|create`
- `wara deploy --workspace ... --environment ... --service ... --image ...`
- `wara deploy --workspace ... --environment ... --service ... --path ...`
- `wara deploy status DEPLOYMENT_ID`
- `wara logs --service ... [--follow]`
- `wara restart --service ...`
- `wara proxy preview --domain ...`
- `wara api GET /api/v1/workspaces`

## Agent-Friendly Interfaces

Examples:

```bash
wara --tree
wara workspaces list --output json --no-interactive
wara deploy --workspace api --environment production --service web --image ghcr.io/acme/web:latest --wait --output ndjson
wara deploy --workspace api --environment production --service web --path . --no-interactive --output json
wara api POST /api/v1/services/018f6a1e-7b0d-74f8-9b7d-4f23e6d0c4ab/deployments --body '{}' --output json
```

Progress output:

- `text`: concise human lines like `queued deployment 018f6a21-0f32-7b0f-86db-2af4d969f8c9`, `running workflow ...`, `succeeded in 42s`.
- `ndjson`: one JSON event per line, suitable for agents and CI logs.
- `json`: final result only unless `--wait` is used, then include event history.

Error behavior:

- Errors include what failed, likely cause, and next action.
- Exit codes are stable:
  - validation error
  - auth error
  - forbidden
  - not found
  - server error
  - network error
  - timeout
- JSON output returns structured errors with `code`, `message`, `hint`, `request_id`, and optional `details`.
- Text output avoids raw stack traces by default and suggests exact retry commands where possible.

## MCP Tools

- `wara_list_workspaces`
- `wara_list_services`
- `wara_deploy_image`
- `wara_deploy_local_artifact`
- `wara_get_deployment`
- `wara_get_logs`
- `wara_restart_service`
- `wara_preview_proxy_config`

## Safety Requirements

The CLI and MCP server must not expose unsafe platform capabilities.

Do not add:

- raw shell execution
- raw SSH command execution
- Docker socket or Docker CLI proxying
- direct database query execution
- decrypted secret retrieval
- unrestricted local filesystem reads
- unrestricted remote filesystem reads

Local artifact deploys may only read the target path supplied by the user and files included by the packaging rules.

## Test Plan

Backend:

- API token CRUD, scope enforcement, and redaction.
- Artifact upload and deploy variants.
- Unsafe API negative tests.

CLI:

- Snapshot test for `wara --tree`.
- Arg-only tests for every command with `--no-interactive`.
- JSON and NDJSON output tests.
- Friendly error tests for missing config, auth failure, not found, timeout, and invalid artifact path.
- `wara api` tests for GET, POST, body, and query handling.

MCP:

- Tool schema tests.
- `stdio` smoke test.
- Streamable HTTP tests for `POST`, `GET`, `Origin`, auth, session, and protocol version.

End-to-end:

- Create token, configure CLI, deploy image via CLI, deploy local Dockerfile/Compose via CLI, then repeat the core deploy/status flow through MCP.

## Assumptions

- API token auth is the v1 auth method for CLI and MCP.
- Local deploy supports image refs and local Compose/Dockerfile directories, not Git clone deploys yet.
- CLI may offer interactive conveniences later, but v1 must be fully usable without prompts.
- MCP Streamable HTTP is local-first and binds to `127.0.0.1` by default.
