#!/usr/bin/env python3
"""Fail when the diff vs --base adds explanatory comments to Rust code (ADR 0002).

Scans added lines in *.rs files. Allowed comment shapes: doc comments (///,
//!), SAFETY contracts, section markers / divider lines, and reference
citations (ADR numbers, firmware date stamps, spec/source citations).
Anything else is treated as an explanatory comment and rejected.
"""

import argparse
import re
import subprocess
import sys

ALLOWED = [
    re.compile(p)
    for p in (
        r"^\s*(///|//!)",
        r"^\s*// SAFETY\b",
        r"^\s*// ?[=\-─═]{3,}",
        r"^\s*// ── .+",
        r"^\s*// === .+ ===\s*$",
        r"//.*\bADR \d{4}\b",
        r"//.*\bdate \d{4}-\d{2}-\d{2}\b",
        r"//.*\(from .+\)",
        r"//.*\bspecs?/",
        r"//.*\.(cpp|cc|c|h|hpp)\b",
        r"//\s*$",
    )
]

CANDIDATE = re.compile(r"(?<!:)//")


def added_lines(base):
    diff = subprocess.run(
        ["git", "diff", "--unified=0", f"{base}...HEAD", "--", "*.rs"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    path = None
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            path = line[6:]
        elif line.startswith("+") and not line.startswith("+++"):
            yield path, line[1:]


def has_comment(line):
    return any(
        line.count('"', 0, m.start()) % 2 == 0 for m in CANDIDATE.finditer(line)
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="git ref to diff against")
    base = parser.parse_args().base

    bad = [
        (path, line)
        for path, line in added_lines(base)
        if has_comment(line) and not any(p.search(line) for p in ALLOWED)
    ]
    for path, line in bad:
        print(f"{path}: {line.strip()}", file=sys.stderr)
    if bad:
        print(
            f"\nerror: {len(bad)} added explanatory comment line(s)."
            " Per ADR 0002: express intent through naming/structure/tests,"
            " or record the constraint in docs/adr/ and cite it"
            " (see docs/adr/0002-self-explanatory-code-no-explanatory-comments.md).",
            file=sys.stderr,
        )
        return 1
    print("comment-gate: OK (no explanatory comments added)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
