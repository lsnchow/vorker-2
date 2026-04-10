#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROMPT_FILE="$ROOT_DIR/docs/codex-style-tui-agent.md"
DRY_RUN=0
PROMPT_FILE_SET=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      ;;
    -*)
      echo "unsupported option: $1" >&2
      exit 1
      ;;
    *)
      if [ "$PROMPT_FILE_SET" -eq 1 ]; then
        echo "unexpected argument: $1" >&2
        exit 1
      fi
      PROMPT_FILE=$1
      PROMPT_FILE_SET=1
      ;;
  esac
  shift
done

if ! command -v codex >/dev/null 2>&1; then
  echo "codex not found. Install the Codex CLI first." >&2
  exit 1
fi

if [ ! -f "$PROMPT_FILE" ]; then
  echo "prompt file not found: $PROMPT_FILE" >&2
  exit 1
fi

if [ "$DRY_RUN" -eq 1 ]; then
  printf 'codex exec -C "%s" - < "%s"\n' "$ROOT_DIR" "$PROMPT_FILE"
  exit 0
fi

exec codex exec -C "$ROOT_DIR" - < "$PROMPT_FILE"
