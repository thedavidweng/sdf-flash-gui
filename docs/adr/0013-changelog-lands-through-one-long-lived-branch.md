# ADR 0013: Changelog lands through one long-lived branch, not one branch per commit

- **Status**: Accepted
- **Date**: 2026-10-09
- **Supersedes**: none

## Context

`.github/workflows/changelog.yml` runs on every push to `main` and rebuilds
`CHANGELOG.md` with git-cliff. It delegated the landing to
`scripts/changelog-pr.sh`, which derived the branch name from the triggering
commit: `changelog/${{ github.sha }}`.

Two problems compound:

1. **One PR per commit.** The branch name embeds the SHA of the main tip that
   triggered the run, so every push to `main` produced a brand-new branch and a
   brand-new pull request. Seven of them accumulated (#54…#59, #62) while none
   of them could merge.

2. **The required checks can never run.** The script authenticated with the
   default `GITHUB_TOKEN` (`GH_TOKEN: ${{ github.token }}`). GitHub's rule is
   that events caused by the `GITHUB_TOKEN` do not start workflow runs, and a
   pull request created or updated with it is placed in an approval-required
   state. Observed effect on `thedavidweng/sdf-flash-gui#62`: the CI run for the
   `pull_request` event ended `action_required` with **zero jobs**, so
   `fmt`, `clippy`, and `test (ubuntu-latest, x86_64-unknown-linux-gnu)` — the
   required contexts for `main` — were never reported. The pull request stayed
   `BLOCKED` forever, and the workflow's 20-minute wait loop finally failed with
   "Required checks did not finish on the changelog pull request", which is the
   failing `update` job on `main`.

The `push: branches: [main, "changelog/**"]` entry in `ci.yml` looks like it
should have covered this, but a `push` made with the `GITHUB_TOKEN` does not
start workflow runs either, so that resolver never fired.

## Decision

**Land changelog changes by refreshing a single long-lived branch. Never create
a branch whose name is derived from the triggering commit.**

- `scripts/changelog-pr.sh` takes the branch name as an argument and always
  resets it onto the current `main` tip (`git checkout -B`), so there is at most
  one changelog branch and at most one changelog pull request, ever. A new
  commit on `main` updates the existing pull request instead of spawning
  another one.
- The script creates a pull request only when the branch has none. If one
  already exists it reports it and exits successfully — no duplicate, no
  waiting, no failing `update` job.
- Pushes use `--force-with-lease`, and only when the remote branch already
  exists.
- `ci.yml` keeps the resolver that makes the required checks reachable: a
  maintainer push to the long-lived branch (a human, not the `GITHUB_TOKEN`)
  starts `push`-triggered CI runs, which report the exact required context
  names on the pull request's head commit. The pattern is the fixed branch
  name, so `changelog/**` no longer matches anything and is replaced by
  `changelog`.
- The `GITHUB_TOKEN` approval-required rule cannot be worked around without a
  credential other than the default token (a PAT or a GitHub App installation
  token). This ADR deliberately does **not** ask for one. Until such a
  credential exists, one maintainer approval per changelog update is the
  intended path: approve the pending CI run, then squash-merge.

## Consequences

- **Positive**: At most one changelog pull request exists at any time; the
  #54…#62 backlog cannot recur.
- **Positive**: Pushing a changelog update no longer depends on a 20-minute
  poll; the job finishes quickly and either reports the existing PR or creates
  one.
- **Positive**: The `update` job on `main` succeeds whenever the changelog is
  already current, instead of failing after a timeout.
- **Negative**: The changelog pull request cannot merge unattended. A
  maintainer must approve the pending CI run (or push the branch once) and
  merge. Adding a PAT or GitHub App token would restore full automation; that
  is the only change needed, and it is intentionally deferred.
- **Negative**: `main`'s `CHANGELOG.md` lags by one merge. That is inherent to
  routing every change through a reviewed pull request.

## Enforcement

- CI: `.github/workflows/workflow-lint.yml` runs jactionlint over the workflow
  files.
- Review: reject any change that reintroduces a commit-derived branch name in
  `changelog.yml` / `changelog-pr.sh`.
