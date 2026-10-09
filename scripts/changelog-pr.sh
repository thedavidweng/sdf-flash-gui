#!/usr/bin/env bash
# Land CHANGELOG.md through one long-lived branch. See docs/adr/0013-*.md.
set -euo pipefail

branch="$1"
message="chore(changelog): sync CHANGELOG.md"

git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add CHANGELOG.md
if git diff --staged --quiet; then
  echo "CHANGELOG.md is up to date"
  exit 0
fi

git fetch --no-tags origin main
git checkout -B "$branch" origin/main
git commit -m "$message"

if git rev-parse --verify --quiet "refs/remotes/origin/${branch}^{commit}" >/dev/null; then
  git push --force-with-lease="$branch" origin "$branch"
else
  git push origin "$branch"
fi

open_prs="$(
  gh pr list --head "$branch" --state open --json number,url \
    --jq '.[] | "#\(.number) \(.url)"'
)"
if [ -n "$open_prs" ]; then
  echo "Refreshed ${branch}; updated pull request(s):"
  echo "$open_prs"
  echo "::notice::Checks on ${branch} need one maintainer approval, because the branch was pushed with the default GITHUB_TOKEN. See docs/adr/0013-changelog-lands-through-one-long-lived-branch.md."
  exit 0
fi

gh pr create --base main --head "$branch" --title "$message" --body "Update CHANGELOG.md from git-cliff.

CI on \`${branch}\` starts in an approval-required state, because the branch was
pushed with the default \`GITHUB_TOKEN\`. Approve the pending CI run (or push the
branch once as a maintainer) and squash-merge. See
docs/adr/0013-changelog-lands-through-one-long-lived-branch.md."
