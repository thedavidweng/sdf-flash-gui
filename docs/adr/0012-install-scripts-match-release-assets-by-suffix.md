# ADR 0012: Install scripts match release assets and checksums by suffix

- **Status**: Accepted
- **Date**: 2026-10-04
- **Supersedes**: none

## Context

The site offers one-line installs: `site/install.sh` for macOS and Linux and
`site/install.ps1` for Windows, served from GitHub Pages. They pick a file
from the latest GitHub release and must verify it before installing.

Release file names are not stable enough to construct:

- They embed the version (`SDF.Flash.GUI_1.1.0_aarch64.dmg`).
- GitHub rewrites spaces in uploaded names to dots, but the
  `SHA256SUMS-<target>.txt` files are written before upload, so they list
  `SDF Flash GUI_1.1.0_aarch64.dmg`.
- Checksum lines come from `sha256sum`, `shasum`, or PowerShell, so a line
  may use `hash  name`, `hash *name`, or CRLF endings.

## Decision

- The scripts read the release from the GitHub API and choose assets by a
  case-insensitive name suffix: `_aarch64.dmg`, `_x64.dmg`, `_amd64.deb`,
  `_x86_64.appimage`, `.msi`.
- They find the expected hash by the same suffix in the target's
  `SHA256SUMS-<target>.txt`, ignoring an optional `*` and a trailing CR, and
  refuse to install on a missing line or a mismatch.
- On macOS, an installed `brew` takes precedence and the script defers to
  the `thedavidweng/tap/sdf-flash-gui` cask, so Homebrew users keep upgrades
  through `brew`.
- `scripts/test-install.sh` runs `install.sh` against stubbed system tools
  for every branch; CI runs it under dash and bash, shellchecks it, and
  parses `install.ps1`.

## Consequences

- Renaming packager outputs or checksum files must keep these suffixes and
  the `SHA256SUMS-<target>.txt` names, or the install scripts break.
- The release workflow must upload every artifact the scripts can pick;
  the AppImage glob is `dist/*.AppImage` because the packager writes that
  exact case.
