.PHONY: build release test test-e2e lint fmt check clean install release-patch release-minor release-major

build:
	cargo build

release:
	cargo build --release

test:
	cargo nextest run --test integration
	cargo nextest run --lib --bin jira

# Run end-to-end tests against a real Jira instance.
# Requires: JIRA_E2E_HOST, JIRA_E2E_EMAIL, JIRA_E2E_TOKEN
# Optional: JIRA_E2E_PROJECT (default: TST)
test-e2e:
	cargo nextest run --test e2e

lint:
	cargo fmt -- --check
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

check: lint test

clean:
	cargo clean

install: release
	cp target/release/jira ~/.local/bin/jira

publish:
	cargo publish

# ── Local Jira (Data Center) for integration testing ──────────────────────────
jira-start:
	docker compose -f docker/docker-compose.yml up -d

jira-stop:
	docker compose -f docker/docker-compose.yml down

jira-wait:
	docker/wait-for-jira.sh

jira-logs:
	docker compose -f docker/docker-compose.yml logs -f jira

jira-reset:
	docker compose -f docker/docker-compose.yml down -v

release-patch:
	vership bump patch

release-minor:
	vership bump minor

release-major:
	vership bump major
