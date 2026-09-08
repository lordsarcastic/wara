SHELL := /bin/bash
.DEFAULT_GOAL := help

DOCKER_COMPOSE ?= docker compose
FRONTEND_DIR := frontend
CLI_DIR := cli
MCP_DIR := mcp
QUEUE ?= default
WARA_TEST_SERVER_KEY_DIR ?= .wara-test-server
WARA_TEST_SERVER_HOST_ROOT ?= /tmp/wara-docker-server
WARA_TEST_SERVER_REMOTE_ROOT ?= /tmp/wara-docker-server/services
WARA_TEST_SERVER_SSH_PORT ?= 2222

.PHONY: help
help: ## Show available make targets.
	@awk 'BEGIN {FS = ":.*## "; printf "\nWara commands:\n\n"} /^[a-zA-Z0-9_.-]+:.*## / {printf "  %-24s %s\n", $$1, $$2} END {printf "\n"}' $(MAKEFILE_LIST)

.PHONY: setup
setup: frontend-install ## Install local development dependencies.
	cargo fetch --locked

.PHONY: up
up: ## Start the full local stack with Docker Compose.
	$(DOCKER_COMPOSE) up --build

.PHONY: up-detached
up-detached: ## Start the full local stack in the background.
	$(DOCKER_COMPOSE) up -d --build

.PHONY: deps-up
deps-up: ## Start local backend dependencies.
	$(DOCKER_COMPOSE) up -d postgres temporal

.PHONY: down
down: ## Stop the local Docker Compose stack.
	$(DOCKER_COMPOSE) down

.PHONY: logs
logs: ## Tail Docker Compose logs.
	$(DOCKER_COMPOSE) logs -f

.PHONY: ps
ps: ## Show Docker Compose service status.
	$(DOCKER_COMPOSE) ps

.PHONY: compose-config
compose-config: ## Validate the root Docker Compose config.
	$(DOCKER_COMPOSE) config --quiet

.PHONY: docker-server-key
docker-server-key: ## Create the local SSH key for the Docker-backed test server.
	@mkdir -p $(WARA_TEST_SERVER_KEY_DIR)
	@if [ ! -f "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519" ]; then \
		ssh-keygen -t ed25519 -N "" -f "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519" -C "wara-docker-server" >/dev/null; \
	fi
	@chmod 600 "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519"
	@chmod 644 "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519.pub"

.PHONY: docker-server-up
docker-server-up: docker-server-key ## Start the Docker-backed SSH test server.
	@mkdir -p "$(WARA_TEST_SERVER_HOST_ROOT)/services"
	COMPOSE_PROFILES=docker-server \
	WARA_TEST_SERVER_HOST_ROOT="$(WARA_TEST_SERVER_HOST_ROOT)" \
	WARA_TEST_SERVER_REMOTE_ROOT="$(WARA_TEST_SERVER_REMOTE_ROOT)" \
	WARA_TEST_SERVER_SSH_PORT="$(WARA_TEST_SERVER_SSH_PORT)" \
	$(DOCKER_COMPOSE) up -d --build --wait docker-server

.PHONY: docker-server-down
docker-server-down: ## Stop the Docker-backed SSH test server.
	COMPOSE_PROFILES=docker-server \
	WARA_TEST_SERVER_HOST_ROOT="$(WARA_TEST_SERVER_HOST_ROOT)" \
	WARA_TEST_SERVER_REMOTE_ROOT="$(WARA_TEST_SERVER_REMOTE_ROOT)" \
	WARA_TEST_SERVER_SSH_PORT="$(WARA_TEST_SERVER_SSH_PORT)" \
	$(DOCKER_COMPOSE) stop docker-server
	COMPOSE_PROFILES=docker-server $(DOCKER_COMPOSE) rm -f docker-server

.PHONY: docker-server-logs
docker-server-logs: ## Tail Docker-backed SSH test server logs.
	COMPOSE_PROFILES=docker-server $(DOCKER_COMPOSE) logs -f docker-server

.PHONY: docker-server-config
docker-server-config: ## Validate the Docker-backed SSH test server Compose config.
	COMPOSE_PROFILES=docker-server \
	WARA_TEST_SERVER_HOST_ROOT="$(WARA_TEST_SERVER_HOST_ROOT)" \
	WARA_TEST_SERVER_REMOTE_ROOT="$(WARA_TEST_SERVER_REMOTE_ROOT)" \
	WARA_TEST_SERVER_SSH_PORT="$(WARA_TEST_SERVER_SSH_PORT)" \
	$(DOCKER_COMPOSE) config --quiet

.PHONY: docker-server-ssh
docker-server-ssh: docker-server-key ## Open SSH into the Docker-backed test server.
	ssh -i "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519" \
		-p "$(WARA_TEST_SERVER_SSH_PORT)" \
		-o UserKnownHostsFile=/dev/null \
		-o StrictHostKeyChecking=no \
		deploy@localhost

.PHONY: docker-server-check
docker-server-check: docker-server-key ## Check SSH, Docker CLI, and Docker Compose on the test server.
	ssh -i "$(WARA_TEST_SERVER_KEY_DIR)/id_ed25519" \
		-p "$(WARA_TEST_SERVER_SSH_PORT)" \
		-o UserKnownHostsFile=/dev/null \
		-o StrictHostKeyChecking=no \
		deploy@localhost \
		'docker version && docker compose version && test -w "$(WARA_TEST_SERVER_REMOTE_ROOT)"'

.PHONY: backend-run
backend-run: ## Run the backend API locally.
	cargo run -p wara-backend

.PHONY: backend-worker
backend-worker: ## Run a Temporal worker locally. Override with QUEUE=high.
	cargo run -p wara-backend --bin temporal_worker -- $(QUEUE)

.PHONY: backend-migrate
backend-migrate: ## Apply pending database schema migrations.
	cd backend && cargo run --bin wara-migrate -- migration apply

.PHONY: backend-migrate-generate
backend-migrate-generate: ## Generate a migration from model changes. Pass NAME=describe_change.
	cd backend && cargo run --bin wara-migrate -- migration generate --name $(NAME)

.PHONY: backend-fmt
backend-fmt: ## Format Rust code.
	cargo fmt --all

.PHONY: backend-fmt-check
backend-fmt-check: ## Check Rust formatting.
	cargo fmt --all -- --check

.PHONY: backend-lint
backend-lint: ## Run Rust clippy with CI settings.
	cargo clippy --workspace --all-targets --locked -- -D warnings

.PHONY: backend-test
backend-test: ## Run Rust tests.
	cargo test --workspace --locked

.PHONY: backend-build
backend-build: ## Build Rust binaries.
	cargo build --workspace --bins --locked

.PHONY: frontend-install
frontend-install: ## Install frontend dependencies.
	cd $(FRONTEND_DIR) && npm install

.PHONY: frontend-ci-install
frontend-ci-install: ## Install frontend dependencies from package-lock.
	cd $(FRONTEND_DIR) && npm ci

.PHONY: frontend-start
frontend-start: ## Start the Angular dev server.
	cd $(FRONTEND_DIR) && npm start

.PHONY: frontend-format
frontend-format: ## Format frontend files with Biome.
	cd $(FRONTEND_DIR) && npm run format

.PHONY: frontend-format-check
frontend-format-check: ## Check frontend formatting with Biome.
	cd $(FRONTEND_DIR) && npm run format:check

.PHONY: frontend-lint
frontend-lint: ## Run frontend Biome lint and Angular typecheck.
	cd $(FRONTEND_DIR) && npm run lint

.PHONY: frontend-test
frontend-test: ## Run frontend tests.
	cd $(FRONTEND_DIR) && npm test

.PHONY: frontend-build
frontend-build: ## Build the frontend.
	cd $(FRONTEND_DIR) && npm run build

.PHONY: cli-fmt
cli-fmt: ## Format CLI Rust code.
	cargo fmt --manifest-path $(CLI_DIR)/Cargo.toml

.PHONY: cli-fmt-check
cli-fmt-check: ## Check CLI Rust formatting.
	cargo fmt --manifest-path $(CLI_DIR)/Cargo.toml -- --check

.PHONY: cli-lint
cli-lint: ## Run CLI clippy with CI settings.
	cargo clippy --manifest-path $(CLI_DIR)/Cargo.toml --all-targets --locked -- -D warnings

.PHONY: cli-test
cli-test: ## Run CLI tests.
	cargo test --manifest-path $(CLI_DIR)/Cargo.toml --locked

.PHONY: cli-build
cli-build: ## Build the CLI.
	cargo build --manifest-path $(CLI_DIR)/Cargo.toml --locked

.PHONY: mcp-fmt
mcp-fmt: ## Format MCP Rust code.
	cargo fmt --manifest-path $(MCP_DIR)/Cargo.toml

.PHONY: mcp-fmt-check
mcp-fmt-check: ## Check MCP Rust formatting.
	cargo fmt --manifest-path $(MCP_DIR)/Cargo.toml -- --check

.PHONY: mcp-lint
mcp-lint: ## Run MCP clippy with CI settings.
	cargo clippy --manifest-path $(MCP_DIR)/Cargo.toml --all-targets --locked -- -D warnings

.PHONY: mcp-test
mcp-test: ## Run MCP tests.
	cargo test --manifest-path $(MCP_DIR)/Cargo.toml --locked

.PHONY: mcp-build
mcp-build: ## Build the MCP server.
	cargo build --manifest-path $(MCP_DIR)/Cargo.toml --locked

.PHONY: temporal-config
temporal-config: ## Validate Temporal Docker Compose config.
	$(DOCKER_COMPOSE) -f temporal/docker-compose.yml config --quiet

.PHONY: observability-config
observability-config: ## Validate observability Docker Compose config.
	$(DOCKER_COMPOSE) -f observability/docker-compose.yml config --quiet

.PHONY: docs-format-check
docs-format-check: ## Check Markdown and YAML formatting.
	npx --yes prettier@3.6.2 --check \
		"README.md" \
		"ROADMAP.md" \
		"DESIGN.md" \
		"PR_GUIDE.md" \
		"CONTRIBUTING.md" \
		"CHANGELOG.md" \
		"backend/README.md" \
		"frontend/README.md" \
		"cli/README.md" \
		"mcp/README.md" \
		"temporal/README.md" \
		"observability/README.md" \
		"deploy/README.md" \
		"docs/package.json" \
		"docs/**/*.{md,yml,yaml}" \
		".github/workflows/*.yml"

.PHONY: check-version
check-version: ## Check core and component version policy.
	./scripts/check-version.sh

.PHONY: cli-validate
cli-validate: check-version cli-fmt-check cli-lint cli-test cli-build ## Run CLI validation.

.PHONY: mcp-validate
mcp-validate: check-version mcp-fmt-check mcp-lint mcp-test mcp-build ## Run MCP validation.

.PHONY: docs-validate
docs-validate: check-version docs-format-check ## Run docs validation.

.PHONY: format
format: backend-fmt frontend-format cli-fmt mcp-fmt ## Format Rust and frontend files.

.PHONY: format-check
format-check: backend-fmt-check frontend-format-check cli-fmt-check mcp-fmt-check docs-format-check ## Check formatting.

.PHONY: lint
lint: backend-lint frontend-lint cli-lint mcp-lint ## Run backend, frontend, CLI, and MCP linters.

.PHONY: test
test: backend-test ## Run stable automated tests.

.PHONY: build
build: backend-build frontend-build ## Build backend binaries and frontend.

.PHONY: validate
validate: check-version format-check lint backend-test frontend-build cli-test cli-build mcp-test mcp-build compose-config temporal-config observability-config ## Run the main local validation suite.
