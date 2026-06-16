# Rowmance

A native desktop database client for MySQL, MariaDB, and PostgreSQL — built with [Tauri v2](https://tauri.app/) (Rust backend) and [Svelte 5](https://svelte.dev/) (frontend).

Rowmance provides a split-panel workspace with a query editor, table browser, schema explorer, ERD viewer, and rich inline data editing — all in a single, keyboard-navigable interface with no Electron overhead.

---

## Screenshot

> Screenshot coming soon — run `make dev` to see it in action.

---

## Prerequisites

| Requirement | Version |
|---|---|
| [Rust toolchain](https://rustup.rs/) | stable (1.78+) |
| [Bun](https://bun.sh/) | v1.0+ |
| macOS | 12+, or Linux / Windows 10+ |

Additional Rust components (installed automatically by `make install`):

```
rustup component add rustfmt clippy
```

---

## Quick Start

```bash
git clone https://github.com/your-org/rowmance.git
cd rowmance
make install   # install frontend deps + Rust toolchain components
make dev       # start the app with hot reload
```

---

## Make Target Reference

Run `make help` to list all targets. The most useful ones:

| Target | Description |
|---|---|
| `make install` | Install all dependencies (frontend + Rust toolchain components) |
| `make dev` | Start the app in development mode (hot reload) |
| `make build` | Build a production release for the current platform |
| `make check` | Run all frontend checks: svelte-check, tsc, eslint, prettier |
| `make lint` | Run ESLint and Prettier checks only |
| `make format` | Auto-fix formatting with Prettier |
| `make test` | Run frontend tests once |
| `make test-watch` | Run frontend tests in watch mode |
| `make test-coverage` | Run frontend tests with coverage report |
| `make rust-check` | Run rustfmt check, clippy, and cargo test |
| `make rust-lint` | Run Clippy linter (warnings as errors) |
| `make rust-test` | Run Rust test suite |
| `make rust-fmt` | Auto-fix Rust formatting |
| `make rust-doc` | Build and verify Rust documentation |
| `make ci` | Run all checks — frontend and Rust (equivalent to CI) |
| `make clean` | Remove all build artefacts |
| `make update` | Update all dependencies to their latest compatible versions |

---

## Configuration

User configuration is stored in `~/.config/rowmance/` (Linux/macOS) or the platform-appropriate AppData folder on Windows:

- `config.sqlite` — connection profiles, groups, saved queries, query history
- `settings.json` — application settings (theme, editor preferences, etc.)
- `themes/` — custom CSS-variable theme files

Credentials (passwords, SSH passphrases) are never written to disk — they are stored in the OS keychain via the Tauri keychain plugin.

---

## Supported Databases

| Database | Minimum Version |
|---|---|
| MySQL | 5.7+ |
| MariaDB | 10.5+ |
| PostgreSQL | 13+ |

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for branch conventions, PR process, and testing guide.

---

## License

MIT
