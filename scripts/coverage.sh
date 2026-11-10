#!/usr/bin/env bash
# Optional local coverage reports. Coverage percentage is not a merge or push gate.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

IGNORE_FILE="${ROOT}/scripts/coverage-ignore.regex"
# One pattern per non-comment line → join with `|` (never strip newlines only;
# that would glue `foo` + `bar` into `foobar` if the file grows to multi-line).
IGNORE_REGEX="$(
  grep -v '^\s*#' "$IGNORE_FILE" | grep -v '^\s*$' | paste -sd '|' -
)"

usage() {
  cat <<USAGE
Usage: $(basename "$0") [html|lcov|report]

  html    HTML report under target/llvm-cov/html (default)
  lcov    Write lcov.info
  report  Text summary with missing lines

Ignore regex (from scripts/coverage-ignore.regex):
  ${IGNORE_REGEX}
USAGE
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
