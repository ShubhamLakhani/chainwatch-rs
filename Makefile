.PHONY: setup up down run demo demo-reorg test test-integration lint fmt logs seed check security contracts-test

export PATH := $(HOME)/.cargo/bin:$(HOME)/.foundry/bin:$(PATH)

setup:
	@test -f .env || cp .env.example .env
	@echo "Created .env from .env.example (edit DATABASE_URL if needed)."
	@echo "Next: make up && make run"

up:
	@if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then \
		docker compose -f compose.yaml up -d postgres anvil; \
	else \
		echo "Docker is not available. Using local Postgres + Anvil."; \
		./scripts/dev/up.sh; \
	fi

down:
	@if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then \
		docker compose -f compose.yaml down; \
	else \
		./scripts/dev/down.sh; \
	fi

run:
	@test -f .env || cp .env.example .env
	cargo run

demo: setup
	./scripts/dev/demo.sh

demo-reorg: setup
	./scripts/dev/reorg_demo.sh

seed:
	./scripts/dev/seed.sh

test:
	cargo test --all-features

test-integration:
	@test -f .env || cp .env.example .env
	set -a && . ./.env && set +a && cargo test --all-features -- --ignored --nocapture

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all -- --check

check:
	./scripts/ci/check.sh

security:
	@if command -v cargo-deny >/dev/null 2>&1; then \
		cargo deny check; \
	else \
		echo "cargo-deny is not installed. Install with: cargo install cargo-deny --locked"; \
		exit 1; \
	fi

contracts-test:
	@cd contracts && forge test

logs:
	@if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then \
		docker compose -f compose.yaml logs -f; \
	else \
		echo "No Docker Compose logs. Indexer logs print in the make run terminal."; \
		echo "Anvil logs: tail -f /tmp/chainwatch-anvil.log"; \
	fi
