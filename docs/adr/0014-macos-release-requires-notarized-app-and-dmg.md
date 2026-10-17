# ADR 0014: macOS releases require a notarized App and DMG

- **Status**: Accepted
- **Date**: 2026-10-16

## Context

Gatekeeper checks Developer ID signatures and Apple notarization for downloads
outside the App Store. Apple Distribution certificates cannot sign these releases.
Cargo Packager 0.11.8 creates the App as a dependency of DMG packaging, and its
DMG packager does not submit or staple the disk image. Re-running DMG packaging
also rebuilds the App, discarding signatures added between commands.

## Decision

The macOS release workflow imports a Developer ID Application identity into a
temporary keychain and authenticates notarization using an App Store Connect API
key. Missing credentials fail the release; there is no unsigned release path.

Cargo Packager builds only the App. `scripts/sign-and-notarize.sh` signs with
hardened runtime and a secure timestamp, verifies the expected team, requires an
Accepted notary response, staples and validates the ticket, and assesses Gatekeeper.
Only then does `scripts/package-macos.sh` copy the App into a drag-and-drop DMG
using `hdiutil`. The DMG passes the same signature, notarization, ticket, and
Gatekeeper checks before checksums are written or artifacts uploaded.

The architecture suffixes and checksum filenames remain compatible with ADR 0012.
The Homebrew quarantine workaround stays until an actual published release is
downloaded and independently verified.

## Consequences

The next macOS release requires these repository secrets:

- `APPLE_CERTIFICATE_P12_BASE64`: base64-encoded Developer ID certificate and private key.
- `APPLE_CERTIFICATE_PASSWORD`: P12 password.
- `APPLE_API_KEY_BASE64`: base64-encoded `.p8` API private key.
- `APPLE_API_KEY_ID` and `APPLE_API_ISSUER`: API authentication identifiers.

The workflow always deletes the temporary keychain, P12, and API key. Release
packaging uses a plain DMG layout with an Applications link. Local development
builds and Linux/Windows packaging continue using their existing commands.

Both macOS targets are built on Apple Silicon runners. Rust and Clang cross-compile
the Intel target; signing and stapling stay on the working Apple Silicon network
path. GitHub tracks the Intel-runner CloudKit connection failures in
[runner-images#14924](https://github.com/actions/runner-images/issues/14924).
