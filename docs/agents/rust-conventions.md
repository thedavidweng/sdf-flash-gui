# Rust conventions

## SDF parser

`src/sdf.rs` may not call `.unwrap(` in production code before `#[cfg(test)]`; CI checks this. Use explicit parse/error handling.

```sh
awk '/^#\[cfg\(test\)\]/{exit} {print}' src/sdf.rs | grep -cF '.unwrap('
# must print 0
```

## Comments

- Follow [ADR 0002](../adr/0002-self-explanatory-code-no-explanatory-comments.md): use naming, structure and tests instead of explanatory comments in new or edited code.
- Allowed necessary exceptions: Rust docs (`///`, `//!`), `SAFETY`, section markers, and concise citations to ADRs/specifications/reverse-engineering findings.
- For a durable non-obvious constraint or external reason, add an ADR; an adjacent source comment can be at most a one-line reference to it.
- This policy overrides the general "do not add or remove comments unless asked" rule. You may remove explanatory comments in code already in your diff; do not bulk-delete unrelated ones. Preserve doc comments, reference comments and section markers.

Before pushing, run:

```sh
python3 scripts/comment-gate.py --base origin/main
```

CI's `comment-policy` job rejects added `*.rs` comments outside the allowed kinds. ADR 0002 remains the canonical rationale.
