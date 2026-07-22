# ADR 0002: Code must be self-explanatory; constraints go to ADRs

- **Status**: Accepted
- **Date**: 2026-07-21
- **Supersedes**: none

## Context

Explanatory comments — comments that describe what the code does or why it is
written a certain way — are a form of duplication. The code already says what it
does; the comment says it again in a less precise language. Over time the two
drift apart: the code is refactored, the comment is not, and the comment becomes
a lie. A misleading comment is worse than no comment because it actively
misleads the reader.

This project has accumulated explanatory comments of several kinds:

- `// Search in the first 256KB where the model string is typically embedded.`
  — restates what the line below it already says.
- `// We scan for the first sequence of 10+ consecutive ASCII digits and
  validate it as a plausible date` — restates the loop body it sits above.
- `// Must not panic (regression test for off-by-one in length check).` — the
  test name already says this.

Each of these can be replaced by better naming, better structure, or nothing at
all. The information either survives without the comment or is too important to
live in a comment that only one reader will see.

The counter-argument is that some context cannot be expressed in code: a magic
offset comes from reverse-engineering a proprietary format, a threshold matches
a rule published in a forum post, a workaround exists because of a hardware bug.
This is real and valuable context — but it does not belong in an inline comment
that rots with the code. It belongs in a decision record that outlives any
single file.

## Decision

**Do not add explanatory comments. Code must be self-explanatory through naming,
structure, and tests. If a non-obvious constraint or external reason prevents
the code from being self-explanatory, record it in an ADR.**

### What is banned

Comments that explain what the code does or why it is structured a certain way,
when the code could communicate the same thing through:

- **Better naming**: `let search_region = &data[..data.len().min(256 * 1024)]`
  instead of `let region = &data[..256 * 1024]; // search first 256KB`.
- **Smaller functions**: a function named `extract_pcb_type` does not need a
  comment saying "extract the PCB type".
- **Tests**: a test named `extract_pcb_type_boundary_length_12307` does not need
  `// 12307 = BOOT_OFFSET + 19 — one byte short of the 20-byte slice` if the
  test body computes the value from the constant.
- **Type signatures**: a `Result<(), PlanError>` return type does not need a
  comment explaining that it can fail.

### What is allowed

1. **Doc comments on public API items** (`///`): these document the interface
   contract — what callers need to know — not the implementation. They are
   consumed by `cargo doc` and are part of the public surface. Keep them short;
   if a doc comment is longer than the function body, the function is either too
   complex or the comment is explaining too much.

2. **Module-level doc comments** (`//!`): one or two sentences describing what
   the module is responsible for. This is the module's "elevator pitch" and
   helps navigation. It is not a place to explain implementation details.

3. **Reference comments**: comments that point to an external source the code
   cannot derive — a specific ADR, a hardware datasheet section, a reverse-
   engineering finding, or a forum post that defines a format. These are
   citations, not explanations. Format: `// per ADR 0001` or
   `// MT1959 boot string at offset 12288 (from MakeMKV reverse engineering)`.
   The comment should be a pointer, not a paragraph.

4. **Section markers in large data tables**: `// === Internal Desktop Drives ===`
  in a static array of 18 entries. These are navigation aids for structured
  data, not code explanations. Allowed only in data declarations (static
  arrays, match arms with many cases), not in logic.

### What happens when code cannot be self-explanatory

When a non-obvious constraint, external requirement, or historical reason makes
the code hard to understand without context:

1. First, try to make the code self-explanatory — better naming, a helper
   function, a constant with a descriptive name, a test that encodes the
   constraint.
2. If the constraint is external (hardware behavior, format specification, past
   bug) and cannot be expressed in code, write an ADR explaining the constraint.
3. If needed, add a one-line reference comment in the code pointing to the ADR.
   The ADR is the source of truth; the comment is a breadcrumb.

### Relationship to the "do not add or remove comments" rule

The general agent instruction "do not add or remove comments unless asked"
is overridden by this ADR for this repository. Agents working in this repo:

- **Must not** add explanatory comments to new code.
- **Must not** add explanatory comments when editing existing code.
- **May** remove explanatory comments from code they are already editing for
  other reasons (the comment is in the diff context and is clearly explanatory).
- **Must not** mass-remove comments from code they are not otherwise touching.
- **Must** preserve doc comments, reference comments, and section markers.

## Consequences

- **Positive**: Code is the single source of truth for what it does. No comment
  can lie because there are no explanatory comments to lie.
- **Positive**: Diffs are smaller and easier to review — no noise from comment
  updates that track code changes.
- **Positive**: Non-obvious constraints are captured in ADRs, which are reviewed
  and versioned, rather than buried in inline comments that only someone reading
  that specific line will see.
- **Positive**: Better naming and structure emerge naturally when the crutch of
  comments is removed.
- **Negative**: Some code becomes more verbose — a descriptive constant name is
  longer than a short variable with a comment. This is an acceptable trade-off.
- **Negative**: Contributors used to comment-heavy code need to adjust. Code
  review should enforce this policy, not rely on self-discipline.
- **Maintenance**: When a non-obvious constraint is discovered during code
  review, the reviewer should ask: "Can the code express this? If not, does an
  ADR exist? If not, write one."

## Enforcement

- Code review: reject PRs that add explanatory comments to new or edited code.
- The `docs/adr/` directory is the canonical home for constraints that cannot
  live in the code.
- Existing explanatory comments are not mass-removed (that would produce
  enormous noisy diffs for no behavioral change). They are cleaned up
  opportunistically when the surrounding code is already being edited.
