# Security Policy

SDF Flash GUI writes firmware to optical drives. A failed or misdirected flash can permanently damage hardware. Treat this software and any firmware files as high-risk.

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.2.x   | Yes       |
| < 0.2   | No        |

## Reporting a Vulnerability

Please **do not** open public GitHub issues for security vulnerabilities.

1. Email **security reports** via GitHub private vulnerability reporting:  
   https://github.com/thedavidweng/sdf-flash-gui/security/advisories/new  
   Or open a minimal issue asking for a private contact channel if advisory reporting is unavailable.

2. Include:
   - Affected version and platform (macOS / Linux / Windows)
   - Steps to reproduce
   - Impact (data loss, drive bricking, crash, privilege issue, etc.)
   - Proof-of-concept if available

3. We aim to acknowledge reports within **5 business days** and provide a remediation plan or status update within **30 days** for confirmed issues.

## Scope

In scope:

- Command construction and process invocation
- Firmware validation and safety gates
- Binary parsers (`sdf.bin`, firmware dumps)
- GUI/CLI paths that could bypass flash safety checks
- Dependency vulnerabilities affecting the shipped binary

Out of scope:

- Bugs in third-party backends (`sdftool`, `makemkvcon`) — report to MakeMKV / upstream tooling
- Drive failures caused by power loss during an in-progress flash
- Issues requiring physical access to already-compromised firmware files on disk

## Safe Use

- Always validate firmware against a trusted manifest before flashing.
- The app checks **signature presence only** — not cryptographic authenticity.
- On Linux, optical drive access may require membership in the `cdrom` group or elevated permissions.
- On Windows, some operations may require running as Administrator.