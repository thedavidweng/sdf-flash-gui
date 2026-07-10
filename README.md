<p align="center">
  <img src="assets/icon.png" width="128" alt="SDF Flash GUI icon">
</p>

<h1 align="center">SDF Flash GUI</h1>

<p align="center">
  <a href="https://github.com/thedavidweng/sdf-flash-gui/actions/workflows/ci.yml"><img src="https://github.com/thedavidweng/sdf-flash-gui/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://codecov.io/gh/thedavidweng/sdf-flash-gui"><img src="https://codecov.io/gh/thedavidweng/sdf-flash-gui/branch/main/graph/badge.svg" alt="codecov"></a>
</p>

Cross-platform GUI for optical drive firmware dump/flash. Inspired by the Windows-only `SDFtool Flasher.exe`

<p align="center">
  <img src="assets/screenshot.png" width="600" alt="SDF Flash GUI screenshot">
</p>

## Features

- **Drive enumeration** — optical drives on macOS, Linux, and Windows (IOKit / sysfs / drive letters), with MakeMKV-style backend `-l` list when a tool is configured
- **Drive properties** — vendor, model, revision, firmware date, MT1959 / MT1939 detection, encrypted firmware, LibreDrive status, SDF.bin version
- **Firmware dump** — read drive firmware via the selected backend
- **Firmware flash** — multi-gate safety before write/recover:
  - MT1959 platform check (probe)
  - Typed confirmation (`FLASH <device>`)
  - Mutually exclusive encrypted vs boot-loader rawflash modes
  - Cross form-factor warning + explicit confirm when slim/desktop mismatch
- **Firmware identification** — SHA-256 lookup in a known-firmware table plus binary content scan (PCB type / embedded model); no reliance on filenames
- **Recovery flash** — 16-byte boot token entry or extraction from a wrong firmware dump
- **SDF0 parsing** — `sdf.bin` / firmware containers (vendor, model, version, flags)
- **Dual backends** — `sdftool` and `makemkvcon` (both from [MakeMKV](https://www.makemkv.com/)); auto-detected on PATH and common install paths
- **CLI** — `list`, `info`, `dump`, `flash`, `sdf-info` share the same planning path as the GUI

## Install

**macOS (Homebrew)**

```bash
brew install --cask thedavidweng/tap/sdf-flash-gui
```

**Linux / Windows**

Download the latest installer from [Releases](https://github.com/thedavidweng/sdf-flash-gui/releases) — `.deb` / `.AppImage` for Linux, `.msi` for Windows.

## Requirements

- [MakeMKV](https://www.makemkv.com/) installed (provides `sdftool` and `makemkvcon`)
- `sdf.bin` from the SDFtool/MKV firmware pack (optional; improves probe/flash when present)

### Platform permissions

| Platform | Notes |
|----------|-------|
| **Linux** | Optical drive access often needs the `cdrom` group (or equivalent rights) to open `/dev/sr*` / `/dev/sg*`. |
| **macOS** | Drive access is usually available to the logged-in user. |
| **Windows** | Some raw device paths may require running as Administrator. |

## Build

```bash
cargo build --release
```

Output: `target/release/sdf-flash-gui`.

Local quality gate (matches Ubuntu CI):

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
./scripts/coverage.sh gate   # project ≥99%, patch 100% on changed domain lines
```

## Architecture

```
src/
  main.rs              CLI entry (no args → GUI)
  lib.rs               library crate for tests
  command.rs           Backend argv planner (no shell strings)
  orchestration.rs     Shared probe / list / flash session (CLI + GUI)
  process.rs           Run / stream / cancel / reap + ProcessRunner trait
  process_runner.rs    NativeRunner (OS process adapter; coverage-ignored)
  flash.rs             SHA-256, SDF peek, version compare
  firmware_db.rs       Known-hash table + binary firmware ID
  platform.rs          Slim/desktop model tables, form-factor helpers
  drive/
    parse.rs           Pure list/identity/selection parsers (covered)
    os.rs              OS enumerate + find_backend / find_sdf_bin (ignored)
  sdf.rs               SDF0 container parser + presentation helpers
  i18n/                Language keys, English + locale tables
  gui/
    state.rs           AppState (drive list apply, probe cache)
    start_gate.rs      Structured Start enablement reasons
    ops/               Lifecycle, start, firmware load, drives, nudge, labels
    workers.rs         Background probe / list / streaming
    views/             egui paint only
    validation.rs      Tool / sdf.bin path checks
    file_dialog.rs     rfd adapter (coverage-ignored)
```

CLI and GUI share probe/list/flash planning through `orchestration` and `command`. GUI Start rules live in `start_gate`; i18n maps them at the edge.

Coverage ignore set is shared: `scripts/coverage-ignore.regex` ↔ `codecov.yml` (checked in CI).

## Security

See `SECURITY.md` for vulnerability reporting.

## Acknowledgements

- **MakeMKV**
- **MartyMcNuts** for the original Windows SDFtool Flasher: [forum thread](https://forum.makemkv.com/forum/viewtopic.php?f=16&t=22896)

## License

GPL-2.0-or-later
