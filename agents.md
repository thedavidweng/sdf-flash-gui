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

Matches CI on Linux. Codecov (and the local/CI gate) enforce:

- **project** line coverage ≥ **99%** (`codecov.yml`)
- **patch** coverage **100%** on changed executable lines in non-ignored `src/`

Ignore set is centralized:

- `scripts/coverage-ignore.regex` — llvm-cov (CI + local)
- `codecov.yml` `ignore:` — Codecov upload (must list the same paths)

```bash
cargo install cargo-llvm-cov   # once, if not installed

./scripts/coverage.sh html     # HTML under target/llvm-cov/html
./scripts/coverage.sh lcov     # lcov.info (same as CI upload)
./scripts/coverage.sh report   # text + missing lines
./scripts/coverage.sh gate     # **required before push**: lcov + project/patch gates
```

`gate` uses the **same lcov generator as Ubuntu CI**, then runs `scripts/coverage-gate.py`. Do not rely only on the Codecov PR check after push.

Open `target/llvm-cov/html/index.html` and confirm changed **non-ignored** files have no meaningful uncovered branches in the diff.

**What is ignored (and why):** native entry/shell, egui views, OS drive discovery (`src/drive/os.rs`), rfd dialogs, and `NativeRunner` (thin process adapter). Domain modules (`command`, `flash`, `orchestration`, `process` lifecycle, `sdf`, `drive/parse`, `gui/ops`, `gui/workers`, …) are **not** ignored — cover new behaviour there with tests.

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