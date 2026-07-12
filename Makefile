# Rowmance — developer convenience targets.
# Requires: bun, cargo (with rustfmt + clippy components).
# Run `make help` to list all targets.

.PHONY: help dev build check lint format test test-watch test-coverage \
        rust-check rust-lint rust-test rust-fmt rust-doc \
        clean install update ci

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
build:
	TAURI_SIGNING_PRIVATE_KEY=$$(cat ~/.tauri/rowmance.key) TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$$(cat ~/.tauri/rowmance.password) bunx tauri build

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
