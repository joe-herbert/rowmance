# Contributing to Rowmance

Thanks for your interest in contributing. This document covers the conventions we use to keep the codebase consistent and the review process smooth.

---

## Branch Conventions

| Prefix | Use for |
|---|---|
| `feature/` | New functionality |
| `fix/` | Bug fixes |
| `chore/` | Maintenance, dependency updates, tooling |

Examples: `feature/csv-export`, `fix/connection-retry`, `chore/update-tauri`.

Always branch from `main`. Keep branches short-lived — aim to merge within a week.

---

## PR Process

1. **Open a draft PR early** if you want feedback on direction before the implementation is complete.
2. **Fill in the PR description** — include what changed, why, and how to test it. PRs without descriptions will not be merged.
3. **All CI checks must pass** before requesting review:
   - `make check` (frontend: svelte-check, tsc, eslint, prettier)
   - `make rust-check` (rustfmt, clippy, cargo test)
4. Request at least one reviewer. Self-merge is not allowed on `main`.
5. Squash or merge — no preference, but keep the history readable.

---

## Testing Guide

### Frontend tests

```bash
make test           # run once
make test-watch     # interactive watch mode
make test-coverage  # with coverage report
```

Tests live alongside the code they test (e.g. `foo.svelte.ts` has `foo.test.ts`). Smoke tests live in `src/tests/`.

All tests mock Tauri's `invoke()` via the setup in `src/tests/setup.ts` — no real backend is required to run the frontend test suite.

### Rust tests

```bash
make rust-test
```

Use `SQLX_OFFLINE=true` when running cargo commands locally if you do not have a live database available (the Makefile handles this automatically).

---

## Code Style

- **Prettier** for all frontend files — run `make format` to auto-fix.
- **rustfmt** for Rust — run `make rust-fmt` to auto-fix.
- **Clippy** must pass with no warnings (`-D warnings`).
- **Comments**: only write comments when the *why* is non-obvious. Code that explains itself needs no comment.
- **No `any` types** in TypeScript — use proper types or generics.
- **Svelte 5 runes syntax**: use `$state`, `$derived`, `$effect` — not the legacy store API.
- **CSS variables**: use tokens from `src/styles/variables.css` (e.g. `var(--color-bg-primary)`) — no hard-coded colours.
