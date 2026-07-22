# Rowmance

A native desktop database client for MySQL, MariaDB, PostgreSQL and SQLite — built with [Tauri v2](https://tauri.app/) (Rust backend) and [Svelte 5](https://svelte.dev/) (frontend).

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
| macOS | 12+, Windows 10+ or Linux (untested) |

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

## Local Release Builds

`make build` produces a signed, notarizable macOS release the same way release CI does (`.github/workflows/release.yml`), instead of the ad-hoc/unsigned build you get from `tauri build` directly.

Signing with a stable Developer ID identity matters for more than distribution: connection passwords are stored in the macOS keychain (see [Configuration](#configuration)), and the keychain's own access-control list trusts whichever app *created* an item, identified by its code signature. An ad-hoc or inconsistently-signed build looks like a "new" app on every rebuild, so macOS re-prompts for the keychain password constantly. With a stable signing identity, that prompt happens once per stored credential and never again.

(The app also carries a `keychain-access-groups` entitlement and embedded provisioning profile for the macOS Data Protection Keychain, which would avoid that one-time prompt entirely — but DPK access proved unreliable in practice on this Team ID/entitlement combination, silently losing writes even when they read back successfully moments later. Secrets are stored via the plain legacy keychain API instead; the DPK entitlement/profile are left in place as a harmless no-op in case that changes in a future macOS release.)

This requires four files under `~/.tauri/` (outside the repo, never committed):

| File | Contents |
|---|---|
| `~/.tauri/rowmance.key` | Tauri updater signing private key |
| `~/.tauri/rowmance.password` | Password for `rowmance.key` |
| `~/.tauri/rowmance.team_id` | Your Apple Team ID (e.g. `WEEZR2L997`) |
| `~/.tauri/rowmance.signing_identity` | Your codesign identity, e.g. `Developer ID Application: Name (TEAMID)` — find it with `security find-identity -v -p codesigning` |

You'll also need a `src-tauri/embedded.provisionprofile` (gitignored) — see the note above on why this no longer needs to be a perfectly valid, matching profile for keychain access to work, though a real one is still required for notarized distribution.

`make build` substitutes your team ID into `src-tauri/entitlements.plist` before building and restores the `__APPLE_TEAM_ID__` placeholder afterward, so the tracked file never ends up dirty. If any of the files above are missing, the target fails fast with a message telling you which one.

Note: local builds skip notarization (no `APPLE_ID`/`APPLE_PASSWORD`) — that only affects Gatekeeper's online check, not the keychain behavior above.

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
| SQLite | 3+ |

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for branch conventions, PR process, and testing guide.

---

## License

MIT
