# Changelog

All notable changes to this project are documented here.
Entries are generated from [Conventional Commits](https://www.conventionalcommits.org/).
## [1.0.0] - 2026-07-25

### 🚀 Features

- Persist settings and quiet empty-drive idle UI

### 🐛 Bug Fixes

- Detect firmware encryption from binary content + ADRs 0001/0002 (#23)
- Cover plan_block defensive branches and test assertion patterns
- **ui:** Harden accessibility, theming, and empty-state density
- **ui:** Refine density, log panel, and settings window sizing
- **ui:** Show Start shortcut tooltip when the button is enabled
- Release-hardening audit fixes across process, GUI, and firmware paths

### 💼 Other

- **deps:** Bump thiserror from 2.0.18 to 2.0.19
- **deps:** Bump serde from 1.0.228 to 1.0.229
- **deps:** Bump serde_json from 1.0.150 to 1.0.151
- **deps:** Refresh lockfile to clear quick-xml RUSTSEC-2026-0194/0195

### 🚜 Refactor

- Deepen firmware_db, start_gate, and WorkerMsg interfaces
- Derive product name from Cargo.toml packager metadata
- **i18n:** Drop dead key, English-identical arms, and if/else prefix chain
- Apply verified simplification audit across domain modules
- Consolidate deep modules per architecture review

### 📚 Documentation

- Adopt uppercase AGENTS.md as the canonical agent rulebook
- Fix stale architecture tree, security version table, and triage labels
- Correct AGENTS module list and amend ADR 0003 for private helpers

### 🧪 Testing

- Hit remaining patch lines for 100% codecov
- Cover quiet list-drives match skip arm for patch gate

### ⚙️ Miscellaneous Tasks

- **changelog:** Sync CHANGELOG.md for v0.4.0
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- Gate explanatory comments on added lines per ADR 0002
- Enforce coverage-threshold sync and drop dead release glob
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **release:** Per-target checksum assets and clean changelog group headers
- **release:** 1.0.0 — version bump, supported-versions table, safe-use reality
## [0.4.0] - 2026-07-10

### 🚀 Features

- Detect LibreDrive support from sdftool --info output
- Show SDF.bin version in Drive Properties panel
- Robust optical drive detection and MakeMKV-style properties

### 🐛 Bug Fixes

- Auto-detect respects user-selected backend
- Forward sdf_version through ProbeComplete to GUI state
- Use explicit f32 suffix for Stroke width literal
- Sync mock tool file before exec to avoid ETXTBSY on Linux
- Correct LibreDrive not-possible parsing and cover status paths
- Eliminate Linux ETXTBSY flake in orchestration tests
- Align probe invalidate rules and enforce local Codecov gates
- Harden patch coverage gate and close residual project gaps
- Invalidate probe cache when drive identity changes
- Validate recovery token graphics; restore PathBuf for Linux OS
- Compile pure macOS drive parsers on all targets
- Move pure macOS drive parsers into drive/parse
- Cfg-gate mac drive parsers for test or macos
- Import mac drive parsers only on macOS
- Cover mac drive parser edge cases for patch gate
- Run parse_drive_list_four_fields on all platforms
- Un-nest codecov comment so yaml validates

### 🚜 Refactor

- Drop dead i18n keys and duplicate helpers
- Deepen modules per architecture review

### 📚 Documentation

- Fix sdftool is not standalone, both backends bundled with MakeMKV
- Note absolute coverage floor matches Codecov + gate
- Refresh README for post-refactor architecture
- Drop binary size note from README

### 🧪 Testing

- Close workers.rs Codecov patch gaps from dead match arms

### ⚙️ Miscellaneous Tasks

- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md for v0.3.0
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- Stop tracking generated lcov.info
- **changelog:** Sync CHANGELOG.md
- Align Codecov project status with coverage-gate absolute floor
- **release:** V0.4.0
## [0.3.0] - 2026-07-09

### 🚀 Features

- Add user-facing safety warnings for firmware flashing
- Identify firmware by binary content + SHA-256 hash database
- Replace first-run modal with settings-button nudge
- Link MakeMKV in About and no-backend banner

### 🐛 Bug Fixes

- Address PR review on shared flash pipeline
- Suppress no-manifest warnings on recover flash
- Harden coverage ignore join and validate_flash log
- Restore exit code 1 for dry-run flash without --confirm
- Reset encrypted_write on firmware load and fix Recover mode reason
- Align encrypted auto-detect threshold with probe and soften two-step text
- Downgrade filename-based encrypted detection to advisory hint
- Firmware_db binary search bugs from review
- Serde default for mt1939 + remove dead classify_firmware

### 🚜 Refactor

- Share flash/probe pipeline between CLI and GUI
- Drop dead SDF0 offset errors after structured-header gate
- Remove unused manifest system
- Fix PR review cleanup items

### 📚 Documentation

- Align agents.md Codecov patch target with 100% gate

### 🧪 Testing

- Raise coverage and harden worker spawn waits
- Fix duplicate test attribute and dead-code warning
- Cover OutcomeRunner streaming and Failed list path
- Close remaining patch gaps and pin Codecov thresholds
- Cover remaining patch gaps in ops and workers
- Cover ops/workers patch residual branches
- Cover firmware_db resolve_model/resolve_form_factor_with_sdf branches
- Cover settings_nudge edge branches for 100% patch

### ⚙️ Miscellaneous Tasks

- **changelog:** Sync CHANGELOG.md for v0.2.0
- Ignore legacy SDFFlashGUI DMGs when updating homebrew tap
- **changelog:** Sync CHANGELOG.md
- Centralize coverage ignores and re-home NativeRunner
- Pin Codecov patch to 100% and project to 99%
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- Bump version to 0.3.0 and fix flaky process tests
## [0.2.0] - 2026-06-26

### 🚀 Features

- **i18n:** Complete GUI internationalization with 30 languages
- **gui:** Firmware safety, cancellable flash, egui 0.34, and v0.2.0

### 🐛 Bug Fixes

- Cargo fmt and clippy lint
- Address greptile review comments
- Reset progress_indeterminate in DrivesListed handler
- Address greptile review comments
- Tighten l10n key count assertion to exact bound
- Give can_start_read_invalid_sdf_path a valid tool_path
- Eliminate uncovered closing-brace line in refresh_drives_empty
- Correct misleading comment in sdf.rs and contradictory test name in orchestration.rs
- Remove unused pick_file from FileDialog trait — clippy dead_code
- Address greptile review — restore initial-dir hint in browse_firmware, trim combined() consistently
- **gui:** Mark probe handled after force-kill to stop auto-reprobe loop
- Set product display name to SDF Flash GUI
- Unify SDF Flash GUI display name across all platforms

### 💼 Other

- **deps:** Bump env_logger from 0.11.10 to 0.11.11
- **deps:** Bump sha2 from 0.10.9 to 0.11.0
- **deps:** Bump codecov/codecov-action from 5 to 7
- **deps:** Bump actions/download-artifact from 7 to 8

### 🚜 Refactor

- Resolve all architecture review candidates
- Remove dead code, deduplicate AppState, simplify combined()
- **gui:** Extract views module and responsive layout

### 📚 Documentation

- Add Homebrew install command to README
- Restructure README — icon, title, badges, then divider
- Center title and badges, remove divider
- Keep agent guide lifecycle testing guidance generic

### 🎨 Styling

- Apply cargo fmt to gui modules

### 🧪 Testing

- Maximize coverage — 265 tests, 93.9% line coverage
- Maximize coverage — 324 tests, 99.04% line coverage
- Add firmware pack e2e suite and restore codecov patch coverage

### ⚙️ Miscellaneous Tasks

- Simplify homebrew tap, add codecov config and badges
- Replace deprecated macos-13 with macos-15-intel
- Exclude drive.rs from coverage — platform-specific code untestable on Linux
- Git-cliff release automation, codecov, and workflow fixes
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md for v0.2.0
- Match x64 DMG filename in homebrew tap release step
- **changelog:** Sync CHANGELOG.md
- Split fmt/clippy checks and update GitHub Actions
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- Extract Homebrew cask template with quarantine postflight
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
## [0.1.0] - 2026-06-24

### 🚀 Features

- Create GitHub Release + auto-update Homebrew tap on tag push

### 🐛 Bug Fixes

- Cargo-packager format (msi→wix), cargo fmt, linguist-vendored for Credits.html

### 🚜 Refactor

- Split UI into toolbar, status bar, and central content panels
- Align gui with egui native patterns

### ⚙️ Miscellaneous Tasks

- Add Codecov coverage reporting via cargo-llvm-cov
- Each build uploads to Release directly, codecov token, homebrew downloads from release
- Split macOS build, homebrew tap only waits for macOS
- Homebrew tap progressive update - arm first, intel silently added later
