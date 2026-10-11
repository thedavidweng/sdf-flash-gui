# Testing and coverage

## Test changes alongside code

Every production code change must include matching test updates in the same commit:

- New behavior: add or extend unit/integration tests for it.
- Bug fixes: include a regression test that fails on the old behavior.
- Logic refactors: keep or relocate coverage without reducing it.
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

The project coverage gate is an absolute >=99% project line floor and 100% patch line coverage for changed executable lines in non-ignored `src/` (not merely no coverage drop). Do not push and wait for Codecov or CI to discover missing coverage.

```sh
cargo install cargo-llvm-cov  # once, if needed
./scripts/coverage.sh html
./scripts/coverage.sh lcov
./scripts/coverage.sh report
./scripts/coverage.sh gate  # required before pushing changed src/
```

The `gate` task uses the same LCOV generator as Ubuntu CI and invokes `scripts/coverage-gate.py`. `lcov` writes `lcov.info`; `report` shows missing lines. Open `target/llvm-cov/html/index.html` and confirm changed non-ignored files have no meaningful uncovered branches in the diff. Do not rely only on the Codecov PR check after pushing.

The ignore list is synchronized between [scripts/coverage-ignore.regex](../../scripts/coverage-ignore.regex) (llvm-cov) and [codecov.yml](../../codecov.yml) (Codecov upload) by `scripts/coverage-ignore-sync-check.py` in CI. They must list the same paths. Excluded adapters/views include the native entry/shell, egui views, `src/drive/os.rs`, rfd dialogs, and `NativeRunner` (thin process adapter). Domain logic (`command`, `firmware_db`, `orchestration`, `process`, `sdf`, `drive/parse`, `drive/probe`, `warnings`, `gui/ops`, `gui/workers`, and other non-ignored modules) remains in coverage scope.

CI runs clippy and tests on Linux, macOS (Intel/Apple Silicon) and Windows; validate available platforms, and at minimum run local fmt/clippy/test.
