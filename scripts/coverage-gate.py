#!/usr/bin/env python3
"""Enforce the same coverage gates as codecov.yml (absolute floors).

Reads lcov.info (from `./scripts/coverage.sh lcov`) and:
  1. Project: line hit rate among non-ignored files must be >= PROJECT_MIN (99%).
     Absolute floor only — no “must not drop vs base” rule (see codecov.yml
     project.threshold: 100%).
  2. Patch: every executable changed line in non-ignored files under src/ must
     have hit count > 0 vs BASE (default: origin/main or main) → 100%.

Ignore set: scripts/coverage-ignore.regex (must match codecov.yml `ignore:` —
checked by scripts/coverage-ignore-sync-check.py in CI).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IGNORE_FILE = ROOT / "scripts" / "coverage-ignore.regex"
PROJECT_MIN = 99.0


def load_ignore_regexes() -> list[re.Pattern[str]]:
    pats: list[re.Pattern[str]] = []
    for line in IGNORE_FILE.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        pats.append(re.compile(line))
    return pats


def ignored(path: str, pats: list[re.Pattern[str]]) -> bool:
    # Normalize to repo-relative src/... style used in ignore file.
    norm = path.replace("\\", "/")
    if "/src/" in norm:
        norm = "src/" + norm.split("/src/", 1)[1]
    elif not norm.startswith("src/"):
        # Only domain sources under src/ participate in gates.
        return True
    return any(p.search(norm) for p in pats)


def parse_lcov(lcov_path: Path) -> dict[str, dict[int, int]]:
    """path -> {line_no: hit_count}"""
    data: dict[str, dict[int, int]] = defaultdict(dict)
    current: str | None = None
    for raw in lcov_path.read_text(errors="replace").splitlines():
        if raw.startswith("SF:"):
            current = raw[3:]
        elif raw.startswith("DA:") and current is not None:
            # DA:<line>,<hits>
            try:
                line_s, hits_s = raw[3:].split(",", 1)
                data[current][int(line_s)] = int(hits_s)
            except ValueError:
                continue
        elif raw == "end_of_record":
            current = None
    return data


def project_line_rate(lcov: dict[str, dict[int, int]], pats: list[re.Pattern[str]]) -> tuple[float, int, int]:
    hit = 0
    total = 0
    for path, lines in lcov.items():
        if ignored(path, pats):
            continue
        for _ln, count in lines.items():
            total += 1
            if count > 0:
                hit += 1
    rate = 100.0 * hit / total if total else 100.0
    return rate, hit, total


def git_changed_lines(base: str) -> dict[str, set[int]]:
    """Return repo-relative paths -> set of new/changed line numbers (right side)."""
    # Prefer merge-base so the gate matches PR patch semantics vs the PR base.
    try:
        merge_base = subprocess.check_output(
            ["git", "merge-base", base, "HEAD"],
            cwd=ROOT,
            text=True,
        ).strip()
    except subprocess.CalledProcessError:
        merge_base = base

    diff = subprocess.check_output(
        ["git", "diff", f"{merge_base}...HEAD", "-U0", "--", "src/"],
        cwd=ROOT,
        text=True,
    )
    changed: dict[str, set[int]] = defaultdict(set)
    path: str | None = None
    new_ln: int | None = None
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            path = line[6:]
            new_ln = None
            continue
        if line.startswith("@@") and path is not None:
            m = re.search(r"\+(\d+)(?:,(\d+))?", line)
            if not m:
                new_ln = None
                continue
            new_ln = int(m.group(1))
            # Pure deletion hunks have +N,0 — no right-side content lines follow.
            if m.group(2) == "0":
                new_ln = None
            continue
        if path is None or new_ln is None:
            continue
        if line.startswith("+") and not line.startswith("+++"):
            changed[path].add(new_ln)
            new_ln += 1
        elif line.startswith("-") and not line.startswith("---"):
            # Left-side only; right line number does not advance under -U0.
            pass
        else:
            # Context (should not appear with -U0) or empty — advance if present.
            if line.startswith(" "):
                new_ln += 1
    return changed


def resolve_lcov_path(path_key: str, lcov: dict[str, dict[int, int]]) -> str | None:
    """Map repo-relative path to an SF: key in lcov."""
    if path_key in lcov:
        return path_key
    for sf in lcov:
        if sf.endswith("/" + path_key) or sf.replace("\\", "/").endswith("/" + path_key):
            return sf
    return None


def patch_uncovered(
    lcov: dict[str, dict[int, int]],
    changed: dict[str, set[int]],
    pats: list[re.Pattern[str]],
) -> list[tuple[str, int]]:
    misses: list[tuple[str, int]] = []
    for rel, lines in changed.items():
        if ignored(rel, pats):
            continue
        sf = resolve_lcov_path(rel, lcov)
        if sf is None:
            # File not in lcov at all (e.g. pure comments) — skip non-src noise
            continue
        hits = lcov[sf]
        for ln in sorted(lines):
            # Only gate lines that appear in DA records (executable).
            if ln not in hits:
                continue
            if hits[ln] <= 0:
                misses.append((rel, ln))
    return misses


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--lcov",
        type=Path,
        default=ROOT / "lcov.info",
        help="Path to lcov.info (default: ./lcov.info)",
    )
    ap.add_argument(
        "--base",
        default="origin/main",
        help="Git ref for patch base (default: origin/main)",
    )
    ap.add_argument(
        "--project-min",
        type=float,
        default=PROJECT_MIN,
        help=f"Minimum project line coverage percent (default: {PROJECT_MIN})",
    )
    args = ap.parse_args()

    if not args.lcov.is_file():
        print(f"error: {args.lcov} not found; run `./scripts/coverage.sh lcov` first", file=sys.stderr)
        return 2

    pats = load_ignore_regexes()
    lcov = parse_lcov(args.lcov)
    rate, hit, total = project_line_rate(lcov, pats)
    print(f"project line coverage: {rate:.2f}% ({hit}/{total})  [min {args.project_min:.2f}%]")

    ok = True
    if rate + 1e-9 < args.project_min:
        print(
            f"FAIL: project coverage {rate:.2f}% is below {args.project_min:.2f}% "
            f"(same floor as codecov.yml project target)",
            file=sys.stderr,
        )
        ok = False
    else:
        print("OK: project coverage")

    try:
        changed = git_changed_lines(args.base)
    except subprocess.CalledProcessError as e:
        # Never treat a missing base / failed diff as an empty patch (that would
        # vacuous-pass the 100% patch gate). CI must fetch base history first.
        print(
            f"error: could not compute patch vs {args.base}: {e}\n"
            f"  Ensure the base ref exists locally (CI: fetch-depth: 0 or git fetch).",
            file=sys.stderr,
        )
        return 2

    misses = patch_uncovered(lcov, changed, pats)
    if misses:
        print(
            f"FAIL: {len(misses)} uncovered executable line(s) in patch "
            f"(codecov patch target is 100%):",
            file=sys.stderr,
        )
        for path, ln in misses[:50]:
            print(f"  {path}:{ln}", file=sys.stderr)
        if len(misses) > 50:
            print(f"  ... and {len(misses) - 50} more", file=sys.stderr)
        ok = False
    else:
        print("OK: patch coverage (all changed executable lines hit)")

    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
