# SDF Flash GUI

Cross-platform Rust GUI and CLI for dumping and flashing optical-drive firmware.

- Every production code change must include matching test updates in the same commit.
- Before committing/pushing, run `cargo fmt -- --check`, `cargo clippy -- -D warnings`, and `cargo test`.
- Hooks are managed by hk via mise; run `mise install` to install them. The pre-commit format check is mandatory: never bypass it with `--no-verify` or `HK=0`. CI runs clippy, tests and coverage.
- When changing `src/`, also run `./scripts/coverage.sh gate` (99% project and 100% patch line coverage on non-ignored code).
- Before non-trivial changes, read all existing [ADRs](docs/adr/).

Read the relevant detail:

- [Tests and coverage](docs/agents/testing-and-coverage.md) — tests, lifecycle checks, coverage gates.
- [Rust conventions](docs/agents/rust-conventions.md) — parser and comment rules.
- [Contribution workflow](docs/agents/contributing.md) — hooks, commits, ADR policy.
- [Domain guide](docs/agents/domain.md) and [Issue tracker](docs/agents/issue-tracker.md).
