#!/usr/bin/env python3
"""Fail if codecov.yml ignore: and scripts/coverage-ignore.regex disagree.

llvm-cov uses regex patterns; Codecov uses path globs. Mapping is intentional
and listed below — update BOTH files when changing the ignore set.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REGEX_FILE = ROOT / "scripts" / "coverage-ignore.regex"
CODECOV_YML = ROOT / "codecov.yml"

# codecov.yml path (or glob) → coverage-ignore.regex pattern
# Keep this table complete; CI runs this script.
EXPECTED: list[tuple[str, str]] = [
    ("src/main.rs", r"src/main\.rs"),
    ("src/gui/mod.rs", r"src/gui/mod\.rs"),
    ("src/gui/views/**", r"src/gui/views/"),
    ("src/drive/os.rs", r"src/drive/os\.rs"),
    ("src/gui/file_dialog.rs", r"src/gui/file_dialog\.rs"),
    ("src/process_runner.rs", r"src/process_runner\.rs"),
]


def load_regex_patterns() -> list[str]:
    out: list[str] = []
    for line in REGEX_FILE.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        out.append(line)
    return out


def load_codecov_ignore() -> list[str]:
    text = CODECOV_YML.read_text()
    # Minimal parse: lines under `ignore:` that look like `  - "path"`
    in_ignore = False
    paths: list[str] = []
    for line in text.splitlines():
        if re.match(r"^ignore:\s*$", line):
            in_ignore = True
            continue
        if in_ignore:
            if re.match(r"^[A-Za-z]", line):
                break
            m = re.match(r'^\s*-\s*["\']?([^"\'#]+?)["\']?\s*(?:#.*)?$', line)
            if m:
                paths.append(m.group(1).strip())
    return paths


def codecov_yml_nests_comment_under_coverage(text: str) -> bool:
    """Historical footgun: comment: under coverage: invalidates the whole file."""
    in_coverage = False
    coverage_indent = -1
    for line in text.splitlines():
        if re.match(r"^coverage:\s*$", line):
            in_coverage = True
            coverage_indent = 0
            continue
        if not in_coverage:
            continue
        if line.strip() and not line.startswith(" ") and not line.startswith("\t"):
            # top-level key again
            in_coverage = False
            continue
        if re.match(r"^\s+comment:\s*$", line):
            return True
    return False


def main() -> int:
    regexes = load_regex_patterns()
    codecov = load_codecov_ignore()
    expected_codecov = [c for c, _ in EXPECTED]
    expected_regex = [r for _, r in EXPECTED]
    yml = CODECOV_YML.read_text()

    ok = True
    if codecov_yml_nests_comment_under_coverage(yml):
        print(
            "FAIL: codecov.yml nests `comment:` under `coverage:` — Codecov rejects "
            "the whole file and falls back to defaults (relative drop checks).",
            file=sys.stderr,
        )
        ok = False
    if regexes != expected_regex:
        print("FAIL: scripts/coverage-ignore.regex does not match expected set:", file=sys.stderr)
        print(f"  file:     {regexes}", file=sys.stderr)
        print(f"  expected: {expected_regex}", file=sys.stderr)
        print("  Update EXPECTED in this script when changing the ignore set.", file=sys.stderr)
        ok = False
    if codecov != expected_codecov:
        print("FAIL: codecov.yml ignore: does not match expected set:", file=sys.stderr)
        print(f"  file:     {codecov}", file=sys.stderr)
        print(f"  expected: {expected_codecov}", file=sys.stderr)
        print("  Update EXPECTED in this script when changing the ignore set.", file=sys.stderr)
        ok = False

    if ok:
        print("OK: codecov.yml ignore and coverage-ignore.regex stay in sync")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
