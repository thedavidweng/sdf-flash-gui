# Domain Glossary

Ubiquitous language for the sdf-flash-gui codebase. Terms are ordered by
dependency: earlier terms are used to define later ones.

## Drive

A physical optical disc drive connected to the host (SATA, USB, or external
enclosure). Represented by [`Drive`](src/drive/parse.rs). Discovered via OS
enumeration (`sdftool -l` or `makemkvcon f -l`) and identified by a
[`DriveIdentity`](src/drive/parse.rs) (vendor, model, revision).

### MT1959 / MT1939

Two MediaTek SoC platforms that this tool targets. MT1959 drives are
flashable; MT1939 drives are not compatible. The platform is detected from
`sdftool --info` output by [`classify_drive_safety`](src/drive/probe.rs).

### Probe interpretation

Turning one `sdftool --info` output into everything the app knows about a
drive: platform safety ([`DriveSafety`](src/drive/probe.rs)), LibreDrive
status, and identity. [`interpret_info`](src/drive/probe.rs) is the single
module owning the `--info` text format; orchestration's `probe_from_output`
wraps it with the raw output for logging.

### Drive form factor

`Desktop` (5.25" internal, e.g. BW-16D1HT, WH16NS60) or `Slim` (external/slim,
e.g. BU40N, BP50NB40). Classified from the model string by
[`classify_drive`](src/platform.rs). Used for cross-flash safety warnings.

### LibreDrive

A per-drive unlock mechanism reported in `sdftool --info` Identification SDF
strings. Status: `Unknown`, `NotAvailable`, `PossibleNotEnabled`, `Enabled`.
Parsed by [`classify_libredrive_status`](src/drive/probe.rs).

## Firmware

A binary blob that contains the drive's operating software. Loaded from disk
(`.bin` file) and written to or read from the drive. Per ADR 0001, firmware
properties are derived from binary content analysis and known-hash lookup,
never from filenames.

### SDF0 container

A container format wrapping some firmware binaries. Parsed by
[`parse_sdf0`](src/sdf.rs). Contains a model field used as a fallback for
identification when the binary content and known-hash lookup are inconclusive.

### Firmware identification

The process of determining a firmware file's properties (model, form factor,
encryption status) from its binary content. Implemented in
[`firmware_db`](src/firmware_db.rs). The cascade is:

1. SHA-256 hash lookup against [`KNOWN_FIRMWARES`](src/firmware_db.rs) (known
   firmware database with curated metadata).
2. Binary content analysis (PCB type from boot string, embedded model string).
3. SDF0 metadata parse (model from container header).

### ResolvedFirmware

The output of [`firmware_db::identify`](src/firmware_db.rs) — the single deep
call that runs the full identification cascade and returns all resolved
properties (identification, SDF0 info, form factor, model, encryption status).
The GUI and CLI store this struct instead of orchestrating the individual
`resolve_*` functions.

### Firmware encryption

Firmware dated ≥ 2020 (year prefix `2120+`) is encrypted and requires
`rawflash enc` mode. The policy lives entirely in [`firmware_db`](src/firmware_db.rs):

- File side: [`resolve_firmware_encrypted`](src/firmware_db.rs) — known-hash
  `is_encrypted` flag takes priority, then the binary date stamp.
- Drive side: [`extract_firmware_date_from_text`](src/firmware_db.rs) applied
  to the drive label / `--info` output by `classify_drive_safety`, using the
  same validated date parser and `ENCRYPTED_FIRMWARE_YEAR_THRESHOLD`.
- Combination: [`encrypted_write_required`](src/firmware_db.rs) —
  `drive_encrypted OR firmware_file_encrypted`; either side being encrypted
  requires the enc mode. GUI state and the CLI flash session both call it;
  the CLI auto-detects file-side encryption via `firmware_db::identify`.

### Recovery boot token

A 16-byte printable ASCII string embedded at offset 12288 in a firmware
binary. Required for `rawflash main,nowait,nocheck,boot=<token>` recovery
mode. Extracted by [`extract_recovery_boot_token`](src/command.rs).

## Flash session

The full workflow of probing a drive, planning an operation, and executing it.
Implemented by [`FlashSession`](src/orchestration.rs) (CLI) and
[`prepare_firmware_op`](src/orchestration.rs) (GUI, pre-probed). Both paths
converge on [`plan_command`](src/command.rs) as the single source of plan
rules (mt1959, mode conflict, confirmation, firmware path, recover token).

### FlashConfirm

How the user confirmed a destructive write/recover. `None` (dry-run), `Flag`
(CLI `--confirm`), or `Typed(String)` (GUI typed `FLASH <device>`). Checked by
[`confirmation_matches`](src/command.rs).

### Cross-flash

Writing a firmware file whose form factor differs from the drive's form factor
(e.g. slim firmware to a desktop drive). Requires explicit user confirmation
via the `cross_flash_confirmed` GUI checkbox. This is a GUI-only safety gate —
not enforced by `plan_command`. The mismatch predicate is
[`cross_flash_mismatch`](src/warnings.rs).

### Flash warnings

Advisories about a pending flash, decided by
[`flash_warnings`](src/warnings.rs) from the selected drive and the
[`ResolvedFirmware`](src/firmware_db.rs): cross-flash mismatch, two-step-flash
models, version downgrade, and firmware/drive model mismatch. Views render the
returned list; no warning decisions live in rendering code.

## Backend

The external tool that performs drive operations: `SdfTool` (sdftool) or
`MakeMkvCon` (makemkvcon). Discovered by [`find_backend`](src/drive/os.rs).
Both backends share the same argv planning via
[`backend_prefix`](src/command.rs).

### Backend outcome

Every cancellable backend operation (probe, list, streaming write/read,
execute) fails as one [`BackendOpError`](src/orchestration.rs): `Failed`,
`Cancelled`, or `NeedsForceKill`. The GUI maps it to Worker messages in one
adapter (`send_backend_error`).

## Worker messages

The GUI uses background threads for long-running operations (probe, flash,
list drives). Results are communicated via `WorkerMsg` on an mpsc channel:

- `WorkerMsg::Stream(StreamEvent)` — incremental events (log lines, progress,
  status) that never produce user-attention notifications.
- `WorkerMsg::Done(WorkerResult)` — terminal results (probe complete,
  operation complete, drives listed, force-kill needed) that may produce
  user-attention notifications.

## Plan

A [`Plan`](src/command.rs) contains a [`Command`](src/command.rs) (program +
structured args). Built by [`plan_command`](src/command.rs) from a
[`PlanRequest`](src/command.rs). The plan is the deep module that both the CLI
and GUI use — the GUI's `start_gate` delegates to `plan_command` and maps
`PlanError` variants to `StartBlock` reasons, and `prepare_firmware_op` calls
it unconditionally so a dry-run validates every plan rule; a
`ConfirmationMismatch` on an unconfirmed request is the one tolerated error
(valid plan, not yet authorized).

## Start gate

The GUI's "may Start be enabled?" check in [`start_gate`](src/gui/start_gate.rs).
Combines GUI-only pre-checks (busy, probing, cross-flash confirmation, path
validation, firmware loaded) with plan-level rules delegated to
`plan_command`. Returns `Option<StartBlock>` — `None` means start is allowed.
