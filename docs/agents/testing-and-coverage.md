# Testing and coverage

## Test changes alongside code

Every production code change must include matching test updates in the same commit:

- New behavior: add or extend unit/integration tests for it.
- Bug fixes: include a regression test that fails on the old behavior.
- Logic refactors: keep or relocate tests that still describe the behavior.
- Security limits, caps, error paths, and early returns: exercise each changed branch.
- Process cancellation, forced termination and backend exit: exercise each exit path and assert backend child processes are reaped before dropping handles.

## Every change

Run before committing/pushing:

```sh
cargo fmt
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

## Changes to `src/`

Do not add tests only to raise a coverage percentage, and do not treat a coverage number as a merge or push requirement. Add or update tests that describe behavior: new features, bug regressions, error paths, and process lifecycle.

An optional local report is available if you want to see which lines the tests touch:

```sh
./scripts/coverage.sh html
./scripts/coverage.sh report
```

CI runs clippy and tests on Linux, macOS (Intel/Apple Silicon) and Windows; validate available platforms, and at minimum run local fmt/clippy/test.
