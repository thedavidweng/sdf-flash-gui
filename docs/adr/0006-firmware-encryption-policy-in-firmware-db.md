# ADR 0006: Firmware encryption policy lives in firmware_db

- **Status**: Accepted
- **Date**: 2026-07-25
- **Supersedes**: none

## Context

The "firmware dated ≥ 2020 (year prefix 2120) is encrypted; encrypted context
requires `rawflash enc`" rule was implemented three times:

- `command.rs` parsed the drive label / `--info` output with its own
  `extract_firmware_date_prefix` — an unvalidated "first token starting with
  four digits" scan — and compared against a bare `2120` literal. A serial
  number token could mask the real date, and a non-date token like `9999…`
  classified a drive as encrypted.
- `firmware_db.rs` parsed the firmware binary with a validated date parser
  (year/month/day ranges) and the named `ENCRYPTED_FIRMWARE_YEAR_THRESHOLD`.
- `gui/state.rs` held the combination rule
  (`encrypted_write = drive_encrypted OR firmware_file_encrypted`) inline.

The two date parsers could disagree on the same input, and the threshold
existed twice. Worse, the CLI flash path never ran file-side detection at
all — `rawflash enc` selection rode entirely on the user passing
`--encrypted`, which is the silent-wrong-mode bug class ADR 0001 documents.

## Decision

**All Firmware encryption knowledge lives in `firmware_db`** (the firmware
property module per ADR 0003):

1. One validated date parser. `extract_firmware_date_from_text` scans
   alphanumeric-delimited tokens of a drive label or `--info` output with the
   same year/month/day validation as the binary scan. `classify_drive_safety`
   delegates to it and compares against `ENCRYPTED_FIRMWARE_YEAR_THRESHOLD` —
   no second parser, no bare literal.
2. One combination rule. `encrypted_write_required(drive_encrypted,
   firmware_file_encrypted)` is the only statement of the drive-OR-file rule.
   `AppState::recompute_encrypted_write` and the flash session both call it.
3. CLI detection. `FlashSession::prepare_with` best-effort reads the firmware
   file, runs `firmware_db::identify`, and ORs the result with the probed
   drive state and the `--encrypted` flag. An encrypted context combined with
   `--include-boot-loader` now fails as `ConflictingWriteModes` instead of
   flashing in the wrong mode. An unreadable file detects nothing (the real
   flash would fail on it anyway).

## Consequences

- Drive side and file side agree by construction; changing the threshold or
  date validation is a one-place edit.
- The CLI gains the same encryption auto-detection the GUI has. `--encrypted`
  remains an explicit override.
- Serial-number and version tokens no longer produce false drive-side
  positives (regression-tested in `drive/probe.rs`).
- The GUI checkbox still binds `encrypted_write` directly (user override);
  probe completion and firmware load re-derive it through the policy call.
