#!/usr/bin/env bash
# Local coverage helpers — same ignore set as CI / Codecov.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

IGNORE_FILE="${ROOT}/scripts/coverage-ignore.regex"
# Strip comments and blank lines; join into one regex.
IGNORE_REGEX="$(
  grep -v '^\s*#' "$IGNORE_FILE" | grep -v '^\s*$' | tr -d '\n'
)"

usage() {
  cat <<EOF
Usage: $(basename "$0") [html|lcov|report]

  html    HTML report under target/llvm-cov/html (default)
  lcov    Write lcov.info (same as CI upload)
  report  Text summary with missing lines

Ignore regex (from scripts/coverage-ignore.regex):
  ${IGNORE_REGEX}
EOF
}

cmd="${1:-html}"
case "$cmd" in
  -h|--help|help) usage; exit 0 ;;
  html)
    cargo llvm-cov --html --ignore-filename-regex "${IGNORE_REGEX}"
    echo "Open: ${ROOT}/target/llvm-cov/html/index.html"
    ;;
  lcov)
    cargo llvm-cov --lcov --output-path lcov.info --ignore-filename-regex "${IGNORE_REGEX}"
    echo "Wrote: ${ROOT}/lcov.info"
    ;;
  report)
    cargo llvm-cov --ignore-filename-regex "${IGNORE_REGEX}" --show-missing-lines
    ;;
  *)
    usage
    exit 1
    ;;
esac
