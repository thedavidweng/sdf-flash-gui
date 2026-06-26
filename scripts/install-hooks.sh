#!/bin/sh
set -e

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$repo_root" config core.hooksPath .githooks
chmod +x "$repo_root/.githooks/pre-commit"
echo "Installed git hooks from .githooks/ (cargo fmt --check runs before each commit)."