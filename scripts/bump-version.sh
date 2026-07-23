#!/usr/bin/env bash
# Bump the project version across package.json, src-tauri/tauri.conf.json and
# src-tauri/Cargo.toml. See `make help` / CONTRIBUTING.md for usage.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PACKAGE_JSON="$ROOT/package.json"
TAURI_CONF="$ROOT/src-tauri/tauri.conf.json"
CARGO_TOML="$ROOT/src-tauri/Cargo.toml"
CARGO_LOCK="$ROOT/src-tauri/Cargo.lock"

usage() {
	cat <<EOF
Usage: $(basename "$0") [version] [--commit] [--tag]

  version    Explicit version to set (e.g. 2.4.0). If omitted, the last
             dot-separated field of the current version is incremented
             by 1 (e.g. 2.3.4 -> 2.3.5).
  --commit   Commit the version bump with message "Bump to v<version>".
  --tag      Create an annotated tag "v<version>" (implies --commit).

Updates package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml and
the matching entry in src-tauri/Cargo.lock.
EOF
}

NEW_VERSION=""
DO_COMMIT=0
DO_TAG=0

for arg in "$@"; do
	case "$arg" in
	--commit)
		DO_COMMIT=1
		;;
	--tag)
		DO_TAG=1
		DO_COMMIT=1
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*)
		echo "Unknown option: $arg" >&2
		usage >&2
		exit 1
		;;
	*)
		if [ -n "$NEW_VERSION" ]; then
			echo "Unexpected extra argument: $arg" >&2
			usage >&2
			exit 1
		fi
		NEW_VERSION="$arg"
		;;
	esac
done

CURRENT_VERSION="$(grep -m1 '"version"' "$PACKAGE_JSON" | sed -E 's/.*"version": *"([^"]+)".*/\1/')"

if [ -z "$CURRENT_VERSION" ]; then
	echo "Could not determine current version from $PACKAGE_JSON" >&2
	exit 1
fi

if [ -z "$NEW_VERSION" ]; then
	if ! [[ "$CURRENT_VERSION" =~ ^([0-9]+\.[0-9]+\.)([0-9]+)$ ]]; then
		echo "Current version '$CURRENT_VERSION' is not in X.Y.Z form; pass an explicit version." >&2
		exit 1
	fi
	PREFIX="${BASH_REMATCH[1]}"
	PATCH="${BASH_REMATCH[2]}"
	NEW_VERSION="${PREFIX}$((PATCH + 1))"
else
	if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
		echo "Version '$NEW_VERSION' is not in X.Y.Z form." >&2
		exit 1
	fi
fi

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

# package.json / tauri.conf.json: replace only the first "version" field.
set_json_version() {
	local file="$1"
	awk -v new="$NEW_VERSION" '
		!done && /"version"[[:space:]]*:/ {
			sub(/"version"[[:space:]]*:[[:space:]]*"[^"]*"/, "\"version\": \"" new "\"")
			done = 1
		}
		{ print }
	' "$file" >"$file.tmp" && mv "$file.tmp" "$file"
}

# Cargo.toml: replace only the version field within the [package] section.
set_cargo_version() {
	local file="$1"
	awk -v new="$NEW_VERSION" '
		/^\[/ { in_package = ($0 == "[package]") }
		in_package && !done && /^version[[:space:]]*=/ {
			sub(/^version[[:space:]]*=[[:space:]]*"[^"]*"/, "version = \"" new "\"")
			done = 1
		}
		{ print }
	' "$file" >"$file.tmp" && mv "$file.tmp" "$file"
}

# Cargo.lock: replace the version field in the [[package]] block for our own
# crate (identified by name from Cargo.toml), which `cargo build`/`check`
# would otherwise update on the next run.
set_cargo_lock_version() {
	local file="$1"
	local pkg_name="$2"
	awk -v new="$NEW_VERSION" -v pkg="$pkg_name" '
		/^name = / { in_pkg = ($0 == "name = \"" pkg "\"") }
		in_pkg && !done && /^version[[:space:]]*=/ {
			sub(/^version[[:space:]]*=[[:space:]]*"[^"]*"/, "version = \"" new "\"")
			done = 1
			in_pkg = 0
		}
		{ print }
	' "$file" >"$file.tmp" && mv "$file.tmp" "$file"
}

CARGO_PACKAGE_NAME="$(grep -m1 '^name = ' "$CARGO_TOML" | sed -E 's/name = "([^"]+)"/\1/')"

set_json_version "$PACKAGE_JSON"
set_json_version "$TAURI_CONF"
set_cargo_version "$CARGO_TOML"

UPDATED_FILES=("$PACKAGE_JSON" "$TAURI_CONF" "$CARGO_TOML")

if [ -f "$CARGO_LOCK" ]; then
	set_cargo_lock_version "$CARGO_LOCK" "$CARGO_PACKAGE_NAME"
	UPDATED_FILES+=("$CARGO_LOCK")
fi

echo "Updated:"
printf '  %s\n' "${UPDATED_FILES[@]}"

if [ "$DO_COMMIT" -eq 1 ]; then
	git -C "$ROOT" add "${UPDATED_FILES[@]}"
	git -C "$ROOT" commit -m "Bump to v$NEW_VERSION"
	echo "Committed: Bump to v$NEW_VERSION"
fi

if [ "$DO_TAG" -eq 1 ]; then
	git -C "$ROOT" tag -a "v$NEW_VERSION" -m "v$NEW_VERSION"
	echo "Tagged: v$NEW_VERSION"
fi
