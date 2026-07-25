# ADR 0008: prepare_firmware_op delegates every gate to plan_command

- **Status**: Accepted
- **Date**: 2026-07-25
- **Supersedes**: none

## Context

`prepare_firmware_op` only called `plan_command` once the user had confirmed.
That forced the write-mode-conflict and MT1959 gates to be pre-stated in
orchestration (twice — once in `prepare_firmware_op`, once again in
`FlashSession::prepare_with` before the probe), while the remaining plan
gates were silently skipped on the unconfirmed path: a CLI dry-run with a
missing firmware path or malformed recovery token reported success.

ADR 0004 already established the correct pattern for the GUI Start gate:
call `plan_command` unconditionally and interpret `PlanError`.

## Decision

**`prepare_firmware_op` calls `plan_command` unconditionally.** A
`ConfirmationMismatch` on an unconfirmed request is the one tolerated error —
it means "plan valid, not yet authorized" and yields `plan: None,
would_execute: false`. Every other `PlanError` is a real validation failure,
confirmed or not. The duplicated pre-checks in orchestration are deleted.

## Consequences

- Each plan gate is stated once, in `plan_command`; orchestration errors are
  derived from typed `PlanError` instead of hand-written strings.
- A dry-run validates the full request (firmware path, token format, mode
  conflict, platform) — dry-run success now means the confirmed run will plan.
- Argument errors are reported after the probe rather than before it; the
  probe is read-only, so the reordering costs one device query in the failure
  case.
- The plan is only ever built with a genuinely matching confirmation, so an
  unconfirmed session can never carry an executable plan.
