#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
RELEASE="$ROOT/target/release/ara-cli"
DEBUG="$ROOT/target/debug/ara-cli"
CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"

if [ -x "$RELEASE" ]; then
  exec "$RELEASE" "$@"
fi

if [ -x "$DEBUG" ]; then
  exec "$DEBUG" "$@"
fi

cd "$ROOT"
exec "$CARGO" run -p ara-cli -- "$@"
