#!/usr/bin/env bash
# Local coverage helpers — same ignore set as CI / Codecov.
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
  cat <<EOF
Usage: $(basename "$0") [html|lcov|report|gate]

  html    HTML report under target/llvm-cov/html (default)
  lcov    Write lcov.info (same as CI upload)
  report  Text summary with missing lines
  gate    Run full suite coverage + enforce gates (same as codecov.yml floors):
            - project line coverage >= 99% (absolute; not “no drop vs main”)
            - patch: 100% of changed executable lines under src/ (non-ignored)

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
  gate)
    # Same generator CI uses (Ubuntu job uploads this lcov to Codecov).
    cargo llvm-cov --lcov --output-path lcov.info --ignore-filename-regex "${IGNORE_REGEX}"
    python3 "${ROOT}/scripts/coverage-gate.py" --lcov "${ROOT}/lcov.info"
    ;;
  *)
    usage
    exit 1
    ;;
esac
