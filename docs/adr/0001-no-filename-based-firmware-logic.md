# ADR 0001: Never derive firmware properties from filenames

- **Status**: Accepted
- **Date**: 2026-07-21
- **Supersedes**: none

## Context

Firmware files for optical disc drives (LG, ASUS, Buffalo, etc.) are distributed
through community packs, forum attachments, and personal backups. During
distribution, filenames are routinely renamed, translated, prefixed, suffixed,
or flattened. A file named `DE_LG_BP50NB40-NB50_1.03_MK.bin` in one pack may
appear as `BP50NB40_1.03.bin`, `lg_bp50nb40_1.03_mk.bin`, or even
`firmware_new.bin` in another.

This project previously considered extracting firmware metadata — model, version,
encryption status, whether it is an "MK" (modified) variant — from the filename.
A filename parser (`parse_firmware_filename`) existed in the test harness, and
the idea of using filename dates to determine encryption was briefly explored.

The core problem: **filenames are not a property of the firmware**. They are a
label applied by whoever last copied the file. Any logic that depends on
filenames is fragile by construction — it will silently produce wrong results
when a file is renamed, and there is no way to detect the failure.

This is not a theoretical concern. It was the root cause of a real bug class:
`encrypted_write` (which selects `rawflash enc` mode) was derived from the
drive's current firmware state rather than the firmware file being written.
Combined with filename-based assumptions about what "should" be encrypted, users
got silent write failures when cross-flashing with renamed files.

## Decision

**Firmware properties must be determined from the firmware binary content or a
known-hash database — never from the filename.**

This applies to all firmware-derived properties, including but not limited to:

- Encryption status (`is_encrypted` / `encrypted_write`)
- Drive model
- Firmware version
- Form factor (desktop / slim)
- MK (modified) variant status
- PCB type
- Whether two-step flashing is required

### Allowed sources, in priority order

1. **Known-hash database** (`KNOWN_FIRMWARES` in `src/firmware_db.rs`) — SHA-256
   lookup against verified firmware entries. 100% accurate, immune to renaming.
2. **Binary content analysis** — scanning the firmware binary for embedded
   metadata (boot string at offset 12288, model strings, date stamps, SDF0
   headers). Works on unknown firmware without relying on filenames.
3. **Drive inquiry data** — the drive's own reported model/vendor/revision (from
   SCSI INQUIRY), used for drive-side decisions like form factor classification.
   This is a property of the *drive*, not the *file*, and is appropriate for
   drive-state logic.
4. **SDF0 container metadata** — structured metadata inside SDF0-wrapped
   firmware, parsed from the binary.

### Explicitly forbidden

- Parsing model names, versions, dates, or encryption status from filenames.
- Using filename prefixes (`DE_`, `HL-DT-ST_`), suffixes (`_MK`, `-MK`), or
  extensions (`.bin`) to infer firmware properties.
- Branching on `path.file_name()` or `path.to_string_lossy()` for any firmware
  property decision.

### Allowed filename uses (not firmware logic)

- **UI display**: showing the basename in the firmware picker, confirmation
  summary, or log messages. This is presentation, not logic.
- **File picker filtering**: filtering by `.bin` extension in the file dialog
  or firmware candidate enumeration. This is a UX convenience, not a property
  determination — the actual identification happens after the file is read.
- **Backend tool validation**: checking that the sdftool/makemkvcon executable
  path contains the expected tool name. This validates the *tool binary*, not
  firmware.
- **OS device enumeration**: reading `/sys/block/sr*` or `/dev/sg*` entries.
  This is OS-level device discovery, not firmware logic.

## Consequences

- **Positive**: Firmware identification is robust against renaming, translation,
  and repackaging. The same firmware file produces the same result regardless of
  what it is called.
- **Positive**: Bugs from mismatched filename assumptions (e.g. encrypted
  firmware treated as non-encrypted because the filename didn't indicate it)
  are eliminated by construction.
- **Negative**: Unknown firmware that does not match the known-hash database and
  whose binary content cannot be parsed (e.g. truly novel firmware with no
  embedded date stamp) will have undetermined encryption status. In this case
  the code falls back to the drive's current firmware state and should surface a
  UI warning advising manual confirmation. This is the correct behavior — it is
  better to ask the user than to guess wrong from a filename.
- **Maintenance**: When new firmware variants are added to the known database,
  the `is_encrypted` field must be populated from the binary's embedded date
  (year ≥ 2120 = encrypted), not from the filename.

## Enforcement

- Code review: any PR that introduces `path.file_name()` or filename string
  parsing in a firmware property code path must be rejected.
- The `firmware_db` module is the single source of truth for firmware
  identification. New firmware property logic should be added there, not
  scattered across GUI or command modules.
- Tests that verify firmware identification must use binary content (real or
  synthesized), not filename assertions. The test harness in
  `tests/e2e_firmware_pack.rs` must not parse filenames to determine firmware
  metadata — it may use filenames only for display in assertion messages.
