# Contributing

Pull requests are welcome. For major changes, open an issue first to discuss what you'd like to change.

## Building

```bash
cargo build --release
```

## Before submitting

Install the git hooks once (runs `cargo fmt --check` before each commit):

```bash
./scripts/install-hooks.sh
```

Then manually before pushing:

- `cargo fmt`
- `cargo clippy -- -D warnings`
- Test on at least one platform (macOS, Linux, or Windows)
