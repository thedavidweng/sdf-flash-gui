# ADR 0004: start_gate delegates plan rules to plan_command

- **Status**: Accepted
- **Date**: 2026-07-21
- **Supersedes**: none

## Context

The GUI's "may Start be enabled?" check (`start_gate::evaluate`) re-implemented
the same plan rules as `command::plan_command`:

- MT1959 platform check
- Write mode conflict (`--encrypted` + `--include-boot-loader`)
- Confirmation string match (`FLASH <device>`)
- Recovery boot token format (16 printable ASCII bytes)
- Firmware path required

These rules existed in two places: `start_gate::evaluate` (for the GUI's
disabled-reason tooltip) and `plan_command` (for building the actual command).
If a rule changed in one but not the other, the GUI would show "Start enabled"
but execution would fail, or vice versa.

## Decision

`start_gate::evaluate` delegates plan-level rules to `plan_command` and maps
`PlanError` variants to `StartBlock` reasons. GUI-only pre-checks (busy,
probing, cross-flash confirmation, path validation, firmware loaded) remain in
`start_gate` because they are UI state, not plan rules.

`plan_command`'s check order was adjusted to check firmware path and mode
conflict before confirmation. This gives the correct UX order (select firmware
before typing confirmation) and does not affect the CLI, which only calls
`plan_command` when the user is already confirmed (via `prepare_firmware_op`).

## Consequences

- `plan_command` is the single source of truth for plan rules. Both the GUI
  gate and `prepare_firmware_op` (CLI + GUI execution) use it.
- Adding a new plan rule requires changing only `plan_command` + the
  `PlanError` → `StartBlock` mapping in `start_gate`.
- The `PlanError` enum is the contract between the deep module (`plan_command`)
  and the GUI adapter (`start_gate`).
