# Agent guide

Instructions for AI agents and contributors working in this repository.

## Code changes require tests

Any production code change must include matching test updates in the same commit:

- New behavior → add or extend unit/integration tests that exercise it.
- Bug fixes → add a regression test that fails without the fix.
- Refactors that move logic → keep or relocate coverage; do not drop it.
- Security bounds (caps, early returns, error paths) → test each branch explicitly.
- Process lifecycle changes (cancel, force-kill, backend exit) → test each exit branch and assert backend children are reaped before handles are dropped.

Do not push and wait for Codecov or CI to discover missing coverage. Run the coverage commands below locally before every push.

## Local checks (run before commit/push)

Install git hooks once (runs `cargo fmt --check` on commit):

```bash
./scripts/install-hooks.sh
```

### Required on every change

```bash
cargo fmt
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

### Coverage (required when touching `src/`)

Matches CI on Linux (Codecov patch gate is 80%):

```bash
cargo install cargo-llvm-cov   # once, if not installed

cargo llvm-cov --html \
  --ignore-filename-regex '(src/main\.rs|src/gui/mod\.rs|src/gui/views/|src/drive\.rs|src/gui/file_dialog\.rs)'
```

Open `target/llvm-cov/html/index.html` and confirm changed files have no uncovered lines in the diff.

Generate lcov the same way CI does:

```bash
cargo llvm-cov --lcov --output-path lcov.info \
  --ignore-filename-regex '(src/main\.rs|src/gui/mod\.rs|src/gui/views/|src/drive\.rs|src/gui/file_dialog\.rs)'
```

### SDF parser policy (CI enforces)

No `.unwrap(` in production code in `src/sdf.rs` (tests excluded). CI checks with:

```bash
awk '/^#\[cfg\(test\)\]/{exit} {print}' src/sdf.rs | grep -cF '.unwrap('
# must print 0
```

### Cross-platform note

CI runs `cargo clippy` and `cargo test` on Linux, macOS (Intel + Apple Silicon), and Windows. At minimum, run fmt + clippy + test locally on your machine before pushing.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `test:`, `ci:`, `refactor:`). Release notes are generated via `git-cliff` from commit history.