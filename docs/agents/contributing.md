# Contributing and decisions

- Install hooks once: `./scripts/install-hooks.sh` (hook checks `cargo fmt --check`).
- Use [Conventional Commits](https://www.conventionalcommits.org/) (e.g. `feat:`, `fix:`, `test:`, `ci:`, `refactor:`); `git-cliff` generates release notes.
- Read all existing [ADRs](../adr/) before non-trivial changes. Record non-obvious constraints or external reasons the code cannot express as the next sequentially numbered ADR in that directory, rather than an explanatory code comment. Follow [ADR 0002](../adr/0002-self-explanatory-code-no-explanatory-comments.md).
- Follow [testing and coverage](testing-and-coverage.md) and [Rust conventions](rust-conventions.md) for checks before committing/pushing.
- [Issue tracking](issue-tracker.md), [triage labels](triage-labels.md) and [domain/glossary routing](domain.md) already have dedicated guides; consult them without duplicating their rules.
