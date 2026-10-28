# Contributing

Pull requests are welcome. For major changes, open an issue first to discuss what you'd like to change.

## Building

```bash
cargo build --release
```

## Before submitting

Run `mise install` to install the hk hooks. The pre-push gate is mandatory; never push with `--no-verify` or set `HK=0`.

All contribution rules live in [AGENTS.md](AGENTS.md): required local checks, the coverage gate, tests-in-the-same-commit, the comment policy (ADR 0002), and architecture decision records.
