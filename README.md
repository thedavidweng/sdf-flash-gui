# SDF Flash GUI

<p align="center">
  <img src="assets/icon.png" width="128" alt="SDF Flash GUI icon">
</p>

Cross-platform GUI for optical drive firmware dump/flash. Inspired by the Windows-only `SDFtool Flasher.exe`

<p align="center">
  <img src="assets/screenshot.png" width="600" alt="SDF Flash GUI screenshot">
</p>

## Features

- **Drive enumeration** — auto-detects optical drives on macOS, Linux, and Windows
- **Drive info** — vendor, model, firmware revision, MT1959 platform detection, encrypted firmware detection
- **Firmware dump** — saves drive firmware to file via the selected backend
- **Firmware flash** — multi-gate safety validation before execution:
  - Model/revision matching against firmware manifest (glob patterns)
  - SHA-256 payload verification
  - Signature presence check (presence only — not cryptographic validity)
  - User confirmation string required
- **Multi-image manifests** — image selector when a manifest contains multiple firmware images
- **Encrypted / full boot-loader rawflash** — mutually exclusive flash modes
- **Recovery flash** — boot token entry or extraction from a wrong firmware dump (offset `0x3000`)
- **SDF0 container parsing** — reads `sdf.bin` metadata (vendor, model, firmware version, encryption, compression)
- **Dual backend support** — works with both `sdftool` (standalone) and `makemkvcon` (MakeMKV); auto-detected via PATH or common install locations

## Requirements

- [MakeMKV](https://www.makemkv.com/) installed on the system
- `sdf.bin` from the SDFtool/MKV firmware pack (optional, for SDF container parsing)

## Build

```bash
cargo build --release
```

Output: `target/release/sdf-flash-gui` (~4 MB, single binary, no runtime deps beyond the backend).

## Architecture

```
src/
  main.rs          Entry point, CLI arg parsing, GUI launch
  gui.rs           egui-based interface (drives, flash, settings tabs)
  drive.rs         Optical drive enumeration + backend binary detection
  sdf.rs           SDF0 container parser
  flash.rs         Flash safety model (validation, dry-run, execute)
  command.rs       sdftool/makemkvcon command planner (structured args, no shell strings)
  manifest.rs      Firmware manifest parser + drive matching
```

## Acknowledgements

- **MakeMKV**
- **MartyMcNuts** for creating the original Windows-based SDFtool Flasher. See the release thread here: [https://forum.makemkv.com/forum/viewtopic.php?f=16&t=22896](https://forum.makemkv.com/forum/viewtopic.php?f=16&t=22896)

## License

GPL-2.0-or-later
