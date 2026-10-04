#!/usr/bin/env bash
set -euo pipefail

branch="$1"
message="$2"

git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add CHANGELOG.md
if git diff --staged --quiet; then
  echo "CHANGELOG.md is up to date"
  exit 0
fi

git checkout -b "$branch"
git commit -m "$message"
git push --force origin "HEAD:${branch}"

if ! gh pr view "$branch" --json url --jq .url >/dev/null 2>&1; then
  gh pr create --base main --head "$branch" --title "$message" --body "Update CHANGELOG.md from git-cliff."
fi

pr="$(gh pr view "$branch" --json number --jq .number)"
for _ in $(seq 1 40); do
  result="$(
    gh pr view "$pr" --json statusCheckRollup --jq '
      def want: ["fmt", "clippy", "test (ubuntu-latest, x86_64-unknown-linux-gnu)"];
      [.statusCheckRollup[]? | select(.name as $n | want | index($n))]
      | if length < 3 then "pending"
        elif any(.conclusion == "FAILURE" or .conclusion == "CANCELLED" or .conclusion == "TIMED_OUT") then "failed"
        elif all(.conclusion == "SUCCESS" or .conclusion == "SKIPPED") then "ok"
        else "pending"
        end
    '
  )"
  echo "changelog checks: ${result}"
  if [ "$result" = "ok" ]; then
    gh pr merge "$pr" --squash --delete-branch
    exit 0
  fi
  if [ "$result" = "failed" ]; then
    echo "::error::Required checks failed on the changelog pull request"
    exit 1
  fi
  sleep 30
done

echo "::error::Required checks did not finish on the changelog pull request"
exit 1
