# rstarter — build, test, and deployment entry points.
#
# Run `make help` for a summary of all targets.
#
#   make <cargo-target>     standard cargo commands
#   make <docker-target>    Docker Compose stacks (dev/prod), migrations, smoke test

SHELL       := /bin/bash
SHELLFLAGS  := -eu -o pipefail -c

COMPOSE_PROD := docker compose
COMPOSE_DEV  := docker compose -f docker-compose.dev.yml

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

##@ Cargo

.PHONY: build
build: ## Compile the workspace (debug)
	cargo build

.PHONY: build-release
build-release: ## Compile the workspace (release)
	cargo build --release

.PHONY: run
run: ## Run the app locally (loads .env; requires DATABASE_URL and REDIS_URL)
	cargo run

.PHONY: run-dev
run-dev: ## Run the app in watch mode (rebuilds and restarts on save)
	cargo watch -c -x run

.PHONY: check
check: ## Type-check without building
	cargo check

.PHONY: test
test: ## Run the hermetic test suite (no PostgreSQL or Redis needed)
	cargo test

.PHONY: fmt
fmt: ## Format the code
	cargo fmt

.PHONY: fmt-check
fmt-check: ## Fail if the code is not formatted
	cargo fmt --check

.PHONY: clippy
clippy: ## Lint with warnings denied
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: doc
doc: ## Build the documentation
	cargo doc --no-deps

.PHONY: clean
clean: ## Remove build artifacts
	cargo clean

.PHONY: verify
verify: ## Full quality gate: format, check, clippy (warnings denied), test
	cargo fmt && cargo fmt --check && cargo check && cargo clippy --all-targets --all-features -- -D warnings && cargo test

##@ Docker stacks

.PHONY: up
up: ## Build images and start the production-like stack (detached)
	$(COMPOSE_PROD) up -d --build

.PHONY: up-dev
up-dev: ## Start the dev stack (detached)
	$(COMPOSE_DEV) up -d --build

.PHONY: down
down: ## Stop the production-like stack
	$(COMPOSE_PROD) down

.PHONY: down-dev
down-dev: ## Stop the dev stack
	$(COMPOSE_DEV) down

.PHONY: reset-dev
reset-dev: ## Stop the dev stack and remove its volumes
	$(COMPOSE_DEV) down -v

.PHONY: logs
logs: ## Follow the production-like stack logs
	$(COMPOSE_PROD) logs -f

.PHONY: logs-dev
logs-dev: ## Follow the dev stack logs
	$(COMPOSE_DEV) logs -f

.PHONY: ps
ps: ## Show the production-like stack status
	$(COMPOSE_PROD) ps

.PHONY: ps-dev
ps-dev: ## Show the dev stack status
	$(COMPOSE_DEV) ps

.PHONY: build-images
build-images: ## Build the app and migrate images
	$(COMPOSE_PROD) build

##@ Migrations

.PHONY: migrate
migrate: ## Run migrations once through the migrate service (prod stack)
	$(COMPOSE_PROD) run --rm migrate

.PHONY: migrate-dev
migrate-dev: ## Run migrations once through the migrate service (dev stack)
	$(COMPOSE_DEV) run --rm migrate

##@ Smoke test

.PHONY: smoke
smoke: ## End-to-end smoke test against a running stack (BASE_URL overridable)
	@BASE_URL="$${BASE_URL:-http://localhost:8080}"; \
	EMAIL="smoke-$$(date +%s)@example.com"; \
	echo "== health =="; \
	curl -sf "$$BASE_URL/health"; echo; \
	echo "== ready =="; \
	curl -sf "$$BASE_URL/ready"; echo; \
	echo "== create user ($$EMAIL) =="; \
	CREATED=$$(curl -sf -X POST "$$BASE_URL/api/v1/users" \
		-H 'content-type: application/json' \
		-d "{\"name\": \"Smoke Test\", \"email\": \"$$EMAIL\"}"); \
	echo "$$CREATED"; \
	USER_ID=$$(printf '%s' "$$CREATED" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p'); \
	[ -n "$$USER_ID" ] || { echo "failed to parse user id" >&2; exit 1; }; \
	echo "== get user (1st, database-backed) =="; \
	curl -sf "$$BASE_URL/api/v1/users/$$USER_ID"; echo; \
	echo "== get user (2nd, Redis-served) =="; \
	curl -sf "$$BASE_URL/api/v1/users/$$USER_ID"; echo; \
	echo "== redis key + ttl =="; \
	docker compose exec -T redis redis-cli GET "user:$$USER_ID" || true; \
	docker compose exec -T redis redis-cli TTL "user:$$USER_ID" || true; \
	echo "== delete user =="; \
	curl -s -o /dev/null -w '%{http_code}\n' -X DELETE "$$BASE_URL/api/v1/users/$$USER_ID"; \
	echo "== get after delete (expect 404) =="; \
	curl -s -o /dev/null -w '%{http_code}\n' "$$BASE_URL/api/v1/users/$$USER_ID"; \
	echo "== smoke test passed =="
