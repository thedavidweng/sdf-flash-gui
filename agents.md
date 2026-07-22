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

- **project** line coverage ≥ **99%** absolute floor (`codecov.yml` + `coverage-gate.py`; not “no drop vs main”)
- **patch** coverage **100%** on changed executable lines in non-ignored `src/`

Ignore set is centralized (CI runs `scripts/coverage-ignore-sync-check.py`):

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

## Architecture decisions

ADRs live in [`docs/adr/`](docs/adr/) and record decisions that constrain future changes. Read all existing ADRs before making non-trivial changes. When you discover a non-obvious constraint or external reason that the code alone cannot express, write a new ADR (next number, same directory) rather than burying it in a comment.

## Code must be self-explanatory

Per [ADR 0002](docs/adr/0002-self-explanatory-code-no-explanatory-comments.md): do not add explanatory comments. Code should communicate its intent through naming, structure, and tests. If a non-obvious constraint or external reason prevents the code from being self-explanatory, record it in an ADR and add at most a one-line reference comment pointing to that ADR.

This overrides the general "do not add or remove comments unless asked" rule for this repository:

- **Do not** add explanatory comments to new or edited code.
- **May** remove explanatory comments from code already in your diff.
- **Do not** mass-remove comments from code you are not otherwise touching.
- **Preserve** doc comments (`///`, `//!`), reference comments (citations to ADRs, specs, or reverse-engineering findings), and section markers in data tables.