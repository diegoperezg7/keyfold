#!/bin/sh
# Prints the app version and fails if package.json and src-tauri/Cargo.toml disagree.
# src-tauri/Cargo.toml is the source of truth: tauri.conf.json has no version, so Tauri takes it
# from there. package.json only mirrors it.
set -eu

cd "$(dirname "$0")/.."

CARGO_VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -1)
NPM_VERSION=$(node -p "require('./package.json').version")

if [ -z "$CARGO_VERSION" ]; then
  echo "No version found in src-tauri/Cargo.toml." >&2
  exit 1
fi
if [ "$CARGO_VERSION" != "$NPM_VERSION" ]; then
  echo "Version mismatch: src-tauri/Cargo.toml has $CARGO_VERSION, package.json has $NPM_VERSION." >&2
  exit 1
fi
echo "$CARGO_VERSION"
