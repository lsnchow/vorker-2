#!/bin/sh
set -eu

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found. Install Rust or source ~/.cargo/env first." >&2
  exit 1
fi

exec cargo run --quiet -p vorker-cli -- tui "$@"
