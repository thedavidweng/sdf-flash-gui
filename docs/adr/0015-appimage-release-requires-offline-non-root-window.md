# ADR 0015: AppImage releases require an offline non-root window

- **Status**: Accepted
- **Date**: 2026-10-17

## Context

The AppImage catalog rejected v1.1.2 because its root-owned AppRun had mode
0744. Cargo Packager 0.11.8 downloads AppRun with mode 0764; copying it under
umask 022 removes group write permission without adding group/other execute.
Rust tests and source coverage do not inspect this generated filesystem.

A real startup check then exposed an additional missing dependency:
xkbcommon-dl opens libxkbcommon-x11 dynamically, so ELF dependency discovery
does not collect it automatically.

## Decision

Before packaging, prepare Cargo Packager's AppRun cache with mode 0755.
Explicitly bundle libxkbcommon-x11 and libxcb-xkb. Build Linux artifacts on
Ubuntu 22.04 rather than an advancing ubuntu-latest image.

Both pull-request CI and release packaging inspect the actual SquashFS using
unsquashfs and require publicly readable/executable entry points and traversable
parent directories. Inspection also requires the keyboard runtime libraries in
the bundle. The AppImage runtime's extraction command is not used for permission
inspection because its extracted root directory does not preserve the image's
root directory mode.

The AppImage embeds `gh-releases-zsync` update information selecting the stable
GitHub release and architecture-specific `.AppImage.zsync` asset. Packaging
publishes the generated delta control file next to the AppImage before computing
release checksums. Smoke acceptance verifies the embedded transport and confirms
the zsync filename and SHA-1 refer to the final AppImage bytes.

Before checksums or publication, launch the extracted AppRun as the non-root
runner in a network namespace without networking, under Xvfb software rendering.
Require a visible process-owned window within 30 seconds and a further three
seconds of process survival. CI retains a window screenshot. No optical-drive
firmware operation is performed during this check.

## Consequences

A successful packager exit is no longer sufficient for publishing Linux assets.
Missing optional backend tools or optical drives must not prevent the main window
from opening. Unit fixtures exercise permissions and missing bundle dependencies;
the actual artifact startup remains the integration acceptance boundary.

The smoke check requires passwordless sudo, network namespaces, Xvfb, xdotool,
ImageMagick, and squashfs-tools on its Linux runner. Other packaging formats and
firmware behavior are unchanged.
