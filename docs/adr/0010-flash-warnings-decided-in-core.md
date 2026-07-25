# ADR 0010: Flash warnings are decided in core, rendered in views

- **Status**: Accepted
- **Date**: 2026-07-25
- **Supersedes**: none

## Context

`src/gui/views/**` is excluded from coverage on the claim "egui rendering
only; logic lives in ops/workers". `show_safety_warnings` in
`main_panel.rs` quietly violated that claim with four compute-and-branch
decisions welded to `&mut egui::Ui`, and the options block re-inlined a
fifth:

- The cross-flash mismatch predicate re-derived what
  `ops::cross_flash_confirmation_required` already computed.
- The write-mode-conflict warning re-inlined
  `command::write_modes_conflict` (which had zero GUI callers).
- The downgrade branch and the model-mismatch containment heuristic existed
  only inside the render function — untested and coverage-invisible.

## Decision

**Warning decisions live in `src/warnings.rs`** (coverage-required core):
`flash_warnings(drive, firmware_form_factor, resolved) -> Vec<FlashWarning>`
decides cross-flash, two-step-flash, downgrade, and model-mismatch;
`cross_flash_mismatch` is the shared predicate used by both the warnings
list and `ops::cross_flash_confirmation_required`. Views render the returned
list (colour + i18n key per variant) and re-derive nothing; the conflict
warning in the options block calls `command::write_modes_conflict`.

## Consequences

- Every flash-safety rule is unit-tested through the `flash_warnings`
  interface; the views coverage exemption claim is true again.
- Changing a warning rule is a one-place edit; the view only changes when
  presentation changes.
- New warnings are added as enum variants — the compiler forces the view to
  render them.
