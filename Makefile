# Rowmance — developer convenience targets.
# Requires: bun, cargo (with rustfmt + clippy components).
# Run `make help` to list all targets.

.PHONY: help dev build check lint format test test-watch test-coverage \
        rust-check rust-lint rust-test rust-fmt rust-doc \
        clean install update ci test-db-up test-db-down

## Show all available targets
help:
	@grep -E '^## ' Makefile | sed 's/## //'

## Install all dependencies (frontend + Rust toolchain components)
install:
	bun install
	rustup component add rustfmt clippy

## Start the app in development mode (hot reload)
dev:
	bunx tauri dev

## Build a production release for the current platform
## Mirrors the release CI: injects the real Apple Team ID into entitlements.plist
## before building (restoring the __APPLE_TEAM_ID__ placeholder afterwards so the
## tracked file never stays dirty) and requires a real embedded.provisionprofile.
build:
	@test -f ~/.tauri/rowmance.key || { echo "Missing ~/.tauri/rowmance.key (Tauri updater signing private key). See 'Local Release Builds' in README.md."; exit 1; }
	@test -f ~/.tauri/rowmance.password || { echo "Missing ~/.tauri/rowmance.password (password for rowmance.key). See 'Local Release Builds' in README.md."; exit 1; }
	@test -f ~/.tauri/rowmance.team_id || { echo "Missing ~/.tauri/rowmance.team_id (your Apple Team ID, e.g. WEEZR2L997). See 'Local Release Builds' in README.md."; exit 1; }
	@test -f ~/.tauri/rowmance.signing_identity || { echo "Missing ~/.tauri/rowmance.signing_identity (your codesign identity, e.g. 'Developer ID Application: Name (TEAMID)'). See 'Local Release Builds' in README.md."; exit 1; }
	@test -s src-tauri/embedded.provisionprofile || { echo "Missing/empty src-tauri/embedded.provisionprofile. See 'Local Release Builds' in README.md."; exit 1; }
	@cp src-tauri/entitlements.plist src-tauri/entitlements.plist.bak
	@trap 'mv src-tauri/entitlements.plist.bak src-tauri/entitlements.plist' EXIT; \
	sed -i '' "s/__APPLE_TEAM_ID__/$$(cat ~/.tauri/rowmance.team_id)/" src-tauri/entitlements.plist; \
	TAURI_SIGNING_PRIVATE_KEY=$$(cat ~/.tauri/rowmance.key) \
	TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$$(cat ~/.tauri/rowmance.password) \
	APPLE_TEAM_ID=$$(cat ~/.tauri/rowmance.team_id) \
	APPLE_SIGNING_IDENTITY=$$(cat ~/.tauri/rowmance.signing_identity) \
	KEYCHAIN_ACCESS_GROUP=$$(cat ~/.tauri/rowmance.team_id).com.jherbert.rowmance \
	bunx tauri build

# ── Frontend ──────────────────────────────────────────────────────────────────

## Run all frontend checks: svelte-check, tsc, eslint, prettier
check:
	bun run check
	bunx tsc --noEmit
	bunx eslint src/
	bunx prettier --check src/

## Run ESLint and Prettier checks only
lint:
	bunx eslint src/
	bunx prettier --check src/

## Auto-fix formatting with Prettier
format:
	bunx prettier --write src/

## Run frontend tests once
test:
	bun run test

## Run frontend tests in watch mode
test-watch:
	bun run test:watch

## Run frontend tests with coverage report
test-coverage:
	bun run test:coverage

# ── Rust ──────────────────────────────────────────────────────────────────────

## Run rustfmt check, clippy, and cargo test
rust-check:
	SQLX_OFFLINE=true cargo fmt --check --manifest-path src-tauri/Cargo.toml
	SQLX_OFFLINE=true cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
	SQLX_OFFLINE=true cargo test --manifest-path src-tauri/Cargo.toml

## Run Clippy linter (warnings as errors)
rust-lint:
	SQLX_OFFLINE=true cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

## Run Rust test suite
rust-test:
	SQLX_OFFLINE=true cargo test --manifest-path src-tauri/Cargo.toml

## Auto-fix Rust formatting
rust-fmt:
	cargo fmt --manifest-path src-tauri/Cargo.toml

## Build and verify Rust documentation
rust-doc:
	SQLX_OFFLINE=true cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# ── Test databases ────────────────────────────────────────────────────────────

## Start the Postgres/SQL Server/MySQL/MariaDB test databases (docker compose)
test-db-up:
	docker compose -f docker-compose.yml up -d

## Stop the test databases and remove their containers
test-db-down:
	docker compose -f docker-compose.yml down

# ── Combined ──────────────────────────────────────────────────────────────────

## Run all checks — frontend and Rust (equivalent to CI)
ci: check rust-check

## Remove all build artefacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	rm -rf node_modules .svelte-kit src-tauri/target

## Update all dependencies to their latest compatible versions
update:
	bun update
	cargo update --manifest-path src-tauri/Cargo.toml
