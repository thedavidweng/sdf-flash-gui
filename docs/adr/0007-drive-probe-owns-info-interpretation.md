# ADR 0007: drive::probe owns sdftool --info interpretation

- **Status**: Accepted
- **Date**: 2026-07-25
- **Supersedes**: none

## Context

Interpreting one `sdftool --info` output was split across two modules:
`command.rs` classified platform safety, LibreDrive status, and MTK mode
(`classify_drive_safety`, `classify_libredrive_status`), while
`drive/parse.rs` parsed vendor/model/revision (`parse_identity_from_info`).
`orchestration::probe_from_output` stapled the two results together.

Both files had to track the same output format, and `command.rs` interleaved
two unrelated topics — argv planning (changes with backend CLI syntax) and
output classification (changes with backend text format) — with disjoint
callers and separate test halves.

## Decision

**`src/drive/probe.rs` is the single module for `--info` interpretation.**
It owns `DriveSafety`, `LibreDriveStatus`, the `classify_*` family, and
`parse_identity_from_info`, and exposes `interpret_info(device, output)`
returning both safety and identity in one pass. `command.rs` keeps planning
only. Date-token parsing is delegated to `firmware_db` per ADR 0006.

## Consequences

- The backend `--info` text format is tracked in one file; adding a probed
  property touches `drive/probe.rs` only.
- `command.rs` is single-topic: the Plan module (types, gates, argv,
  command display).
- `probe_from_output` remains as orchestration's thin wrapper that carries
  the raw output alongside the interpretation for logging.
- Classification tests moved next to the code they test.
