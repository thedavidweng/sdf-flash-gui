# ADR 0003: firmware_db as the single firmware identification module

- **Status**: Accepted
- **Date**: 2026-07-21
- **Supersedes**: none

## Context

Firmware identification logic was spread across two modules:

- `src/flash.rs` — a "grab bag" containing `sha256_hex`, `check_firmware_sdf`
  (SDF0 metadata extraction), `compare_versions`, and `FlashDirection`. These
  were utilities with no cohesive theme, used by different callers.
- `src/firmware_db.rs` — the known-hash database and binary content analysis.

Callers (GUI `ops/firmware.rs`, `ops/labels.rs`, `views/main_panel.rs`) had to
orchestrate four separate functions to fully identify a firmware file:

```rust
let sdf_info = flash::check_firmware_sdf(&data);
let id = firmware_db::identify_firmware(&data);
let form_factor = firmware_db::resolve_form_factor_with_sdf(&id, sdf_info.as_ref());
let model = firmware_db::resolve_model_with_sdf(&id, sdf_info.as_ref());
let encrypted = firmware_db::resolve_firmware_encrypted(&id, &data);
```

This shallow interface forced every caller to know the cascade order
(known hash → binary PCB → SDF0 model → date stamp) and thread five fields
through `AppState`. The cascade order was duplicated knowledge — if the
priority changed, every caller would need updating.

## Decision

1. **Dissolve `flash.rs` into `firmware_db`.** All firmware-related utilities
   (`sha256_hex`, `check_firmware_sdf`, `FirmwareSdfInfo`, `compare_versions`,
   `FlashDirection`) moved to `firmware_db.rs`. The `flash` module no longer
   exists.

2. **Add `firmware_db::identify` as the single deep entry point.** It runs the
   full cascade and returns a `ResolvedFirmware` struct with all resolved
   properties. Callers store `Option<ResolvedFirmware>` instead of five
   separate fields.

```rust
let resolved = firmware_db::identify(&data);
// resolved.form_factor, resolved.model, resolved.encrypted, resolved.sdf_info, resolved.identification
```

## Consequences

- `firmware_db` is the single module for all firmware identification. Its
  interface is deeper: one call replaces five.
- The cascade order is encoded in one place (`identify`), not duplicated across
  callers.
- `AppState` stores `firmware_resolved: Option<ResolvedFirmware>` instead of
  `firmware_identification`, `firmware_sdf_info`, `firmware_form_factor`, and
  `firmware_file_encrypted` as separate fields.
- The individual `resolve_*` functions remain `pub` for testing and for
  `identify`'s internal composition, but callers should prefer `identify`.
