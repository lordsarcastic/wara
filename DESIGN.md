# Wara Design And Style Guide

This guide defines how Wara should look, feel, and be implemented. It is written
for human contributors and AI coding agents. Follow it unless a maintainer
explicitly approves a different direction.

## Product Principles

Wara is an operational tool. The interface should feel calm, precise, and fast.
It should help users repeatedly manage servers, projects, services, deployments,
domains, credentials, and logs without visual noise.

Core product principles:

- Docker-native: services run from Docker images, Compose files, or Dockerfiles.
- Self-hosted: users own their infrastructure, secrets, data, and telemetry.
- Safe automation: APIs, CLI, and MCP expose scoped platform actions, not raw
  shell, raw SSH, Docker socket proxying, or database access.
- Platform-only telemetry: OpenTelemetry monitors Wara itself, never hosted app
  telemetry.
- Secret discipline: secrets are encrypted or hashed at rest, redacted in
  responses, and never placed in JWT claims, logs, spans, or screenshots.

## Architecture Boundaries

Backend structure:

- `routes/`: Axum handlers. They authenticate, validate, call services, and
  return DTOs. Keep them thin.
- `services/`: business logic, persistence orchestration, auth, deploy planning,
  proxy generation, and workflow starters.
- `entities/`: API-facing structs and Toasty record structs.
- `libs/`: config, database setup, crypto, telemetry, SSH, Docker helpers.
- `middleware/`: cross-cutting Axum/Tower layers.
- `services/workflows/` and `services/activities/`: Temporal code.
- `openapi.rs`: Utoipa registration.

Frontend structure:

- Use Angular standalone components unless a feature clearly needs a module.
- Keep API calls in services once the frontend grows beyond the current shell.
- Use route guards for authenticated areas.
- Use ZardUI components where they exist.
- Use Tailwind utilities for layout and styling.

## Backend Style

Rust conventions:

- Use Rust 1.95 and edition 2024.
- Keep route handlers small and readable.
- Prefer typed structs over maps or stringly typed state.
- Use `uuid::Uuid` for persistent product IDs.
- Use `chrono` for externally visible timestamps unless a local model requires
  another format.
- Use `WaraError` for internal process and infrastructure errors. Use `ApiError`
  only for errors exposed through HTTP API responses.
- Add comments only when they explain non-obvious behavior or a security
  decision.

Axum conventions:

- Public APIs live under `/api/v1`.
- Use `axum-valid::Valid<Json<T>>` for request body validation.
- DTOs should derive `utoipa::ToSchema` when exposed through OpenAPI.
- All authenticated routes use `CurrentUser` or `AdminUser`.
- Admin-only APIs live under `/api/v1/admin`.

Toasty conventions:

- API structs and DB record structs may be separate when Toasty cannot store a
  public enum or when API redaction differs from storage.
- Store enums as strings in DB records when needed.
- Convert DB records into API structs with `From<Record> for PublicType`.
- Do not expose DB-only secret fields in API schemas.
- Add integration tests for each new persisted resource.

Auth conventions:

- Passwords use Argon2 hashes.
- JWT access tokens are RS256 signed, not encrypted.
- JWT claims must contain only identity and authorization metadata.
- Do not put secrets, API tokens, SSH material, env vars, or app logs in JWTs.
- Opaque tokens, such as invites and future API tokens, must be hashed at rest.

Security conventions:

- No arbitrary shell execution endpoints.
- No raw SSH command endpoints.
- No raw Docker socket or Docker CLI proxy endpoints.
- No direct database query endpoint.
- No endpoint returns decrypted secrets.
- Always add negative tests when implementing security-sensitive behavior.

OpenAPI conventions:

- Every public route gets a `#[utoipa::path]` annotation.
- Request/response bodies should be schema-registered.
- Authenticated paths include the bearer auth security scheme.
- Error responses should use the standard `ErrorResponse` envelope.

## Frontend Design Style

The visual direction is Apple-inspired in the broad sense: clean, minimal,
smooth, restrained, and highly legible. Do not copy Apple assets, icons,
trademarks, product layouts, or proprietary visuals.

Visual qualities:

- Quiet, operational, and work-focused.
- Dense enough for repeated use.
- Clear hierarchy without marketing-style hero sections.
- Smooth transitions only where they improve comprehension.
- Light surfaces with subtle borders and restrained shadows.
- Avoid visual decoration that does not serve a task.

Layout:

- Prefer stable application shells: sidebar, top header, content area.
- Use full-width sections or constrained content regions.
- Do not nest cards inside cards.
- Cards are for repeated resource items, modals, and framed tools.
- Avoid oversized empty space in operational screens.
- Preserve readable density on desktop and mobile.

Controls:

- Use ZardUI components when available.
- Use Tailwind utility classes before custom CSS.
- Use forms for resource creation/editing.
- Use tabs for view switching.
- Use segmented controls for modes.
- Use toggles or checkboxes for binary settings.
- Use menus/selects for option sets.
- Use icon buttons only when the icon is familiar or has a tooltip.

Typography:

- Do not use viewport-scaled font sizes.
- Use clear, short labels.
- Use compact headings inside panels and operational surfaces.
- Avoid negative letter spacing.
- Ensure text never overflows buttons, cards, or tables.

Color:

- Avoid one-note palettes.
- Do not make the UI dominated by purple/blue gradients, beige/tan, dark slate,
  or brown/orange.
- Use color sparingly for status, risk, and action emphasis.
- Status colors must be understandable with text labels, not color alone.

States:

- Every async action needs loading, success, and error states.
- Empty states should tell the user what action is available.
- Destructive actions need confirmation.
- Secret fields must never re-render stored secret values.

## Tailwind And CSS Rules

- Prefer Tailwind utility classes.
- Add custom CSS only for base tokens, unavoidable third-party integration, or
  repeated patterns that Tailwind cannot express cleanly.
- Do not add decorative gradients, blobs, or bokeh backgrounds.
- Keep fixed-format controls stable with explicit sizing, grid tracks, or
  responsive constraints.
- Validate mobile layouts when changing major surfaces.

## ZardUI Rules

- Use ZardUI for buttons, inputs, dialogs, tabs, menus, forms, checkboxes,
  toggles, and similar controls when components exist.
- Keep ZardUI variants consistent across the app.
- Do not mix several component libraries for the same primitive.
- If a ZardUI component is missing, use semantic HTML plus Tailwind and document
  why.

## API UX Rules

Safe public APIs should be automation-friendly:

- Stable route names.
- Stable error envelope.
- Predictable pagination when lists grow.
- Idempotency headers for deploy/restart/template jobs where relevant.
- No hidden required fields.
- Clear validation errors.

## CLI And MCP Style

Future CLI and MCP work should be agent-friendly:

- Every interactive input must also be available through args or flags.
- No required fancy select boxes.
- Provide `--json` output for machine consumption.
- Provide `--tree` to print all commands and subcommands.
- Errors should include what failed and the next action to try.
- MCP tools should be narrow, safe, and schema-first.

## Temporal Style

- Workflows orchestrate; activities perform side effects.
- Activity inputs should be explicit typed structs.
- Activities that decrypt secrets should keep decrypted values local and short
  lived.
- Workflow IDs should be persisted where users can inspect job state.
- Long-running jobs should update deployment status.

## Observability Style

- Telemetry is disabled by default.
- Telemetry must cover Wara platform internals only.
- Redact secrets before logs, spans, metrics labels, or audit events.
- Do not emit hosted app logs to platform telemetry exporters.
- Metrics labels must be bounded cardinality.

## Testing Standards

Backend:

- Unit test pure helpers and command builders.
- Integration test Toasty-backed resource services.
- Route test auth, validation, and error behavior.
- Add negative tests for unsafe capabilities.
- Add redaction tests for every secret type.

Frontend:

- Test route guards and form validation.
- Test loading, error, and empty states.
- Test API service behavior when generated clients exist.

Manual checks:

- Confirm `/docs` renders.
- Confirm invite flow works.
- Confirm auth-protected routes reject missing/invalid tokens.
- Confirm no API response returns decrypted secrets.

## AI Contributor Checklist

Before asking an AI agent to implement a task, give it:

- The exact roadmap item.
- The expected files or feature area.
- The tests that should pass.
- Any security constraints.

Before accepting AI-generated work, verify:

- It followed existing architecture boundaries.
- It did not introduce raw shell, raw SSH, Docker socket proxy, or secret leaks.
- It did not add custom CSS where Tailwind/ZardUI would work.
- It updated OpenAPI annotations for API changes.
- It included tests at the right layer.
- It did not silently change unrelated behavior.
