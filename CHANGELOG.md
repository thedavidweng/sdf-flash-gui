# Changelog

All notable changes to this project are documented here.
Entries are generated from [Conventional Commits](https://www.conventionalcommits.org/).
## [Unreleased]

### <!-- 0 -->🚀 Features

- Persist settings and quiet empty-drive idle UI

### <!-- 1 -->🐛 Bug Fixes

- Detect firmware encryption from binary content + ADRs 0001/0002 (#23)
- Cover plan_block defensive branches and test assertion patterns
- **ui:** Harden accessibility, theming, and empty-state density
- **ui:** Refine density, log panel, and settings window sizing
- **ui:** Show Start shortcut tooltip when the button is enabled

### <!-- 2 -->🚜 Refactor

- Deepen firmware_db, start_gate, and WorkerMsg interfaces

### <!-- 6 -->🧪 Testing

- Hit remaining patch lines for 100% codecov
- Cover quiet list-drives match skip arm for patch gate

### <!-- 7 -->⚙️ Miscellaneous Tasks

- **changelog:** Sync CHANGELOG.md for v0.4.0
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
## [0.4.0] - 2026-07-10

### <!-- 0 -->🚀 Features

- Detect LibreDrive support from sdftool --info output
- Show SDF.bin version in Drive Properties panel
- Robust optical drive detection and MakeMKV-style properties

### <!-- 1 -->🐛 Bug Fixes

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

### <!-- 2 -->🚜 Refactor

- Drop dead i18n keys and duplicate helpers
- Deepen modules per architecture review

### <!-- 3 -->📚 Documentation

- Fix sdftool is not standalone, both backends bundled with MakeMKV
- Note absolute coverage floor matches Codecov + gate
- Refresh README for post-refactor architecture
- Drop binary size note from README

### <!-- 6 -->🧪 Testing

- Close workers.rs Codecov patch gaps from dead match arms

### <!-- 7 -->⚙️ Miscellaneous Tasks

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

### <!-- 0 -->🚀 Features

- Add user-facing safety warnings for firmware flashing
- Identify firmware by binary content + SHA-256 hash database
- Replace first-run modal with settings-button nudge
- Link MakeMKV in About and no-backend banner

### <!-- 1 -->🐛 Bug Fixes

- Address PR review on shared flash pipeline
- Suppress no-manifest warnings on recover flash
- Harden coverage ignore join and validate_flash log
- Restore exit code 1 for dry-run flash without --confirm
- Reset encrypted_write on firmware load and fix Recover mode reason
- Align encrypted auto-detect threshold with probe and soften two-step text
- Downgrade filename-based encrypted detection to advisory hint
- Firmware_db binary search bugs from review
- Serde default for mt1939 + remove dead classify_firmware

### <!-- 2 -->🚜 Refactor

- Share flash/probe pipeline between CLI and GUI
- Drop dead SDF0 offset errors after structured-header gate
- Remove unused manifest system
- Fix PR review cleanup items

### <!-- 3 -->📚 Documentation

- Align agents.md Codecov patch target with 100% gate

### <!-- 6 -->🧪 Testing

- Raise coverage and harden worker spawn waits
- Fix duplicate test attribute and dead-code warning
- Cover OutcomeRunner streaming and Failed list path
- Close remaining patch gaps and pin Codecov thresholds
- Cover remaining patch gaps in ops and workers
- Cover ops/workers patch residual branches
- Cover firmware_db resolve_model/resolve_form_factor_with_sdf branches
- Cover settings_nudge edge branches for 100% patch

### <!-- 7 -->⚙️ Miscellaneous Tasks

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

### <!-- 0 -->🚀 Features

- **i18n:** Complete GUI internationalization with 30 languages
- **gui:** Firmware safety, cancellable flash, egui 0.34, and v0.2.0

### <!-- 1 -->🐛 Bug Fixes

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

### <!-- 10 -->💼 Other

- **deps:** Bump env_logger from 0.11.10 to 0.11.11
- **deps:** Bump sha2 from 0.10.9 to 0.11.0
- **deps:** Bump codecov/codecov-action from 5 to 7
- **deps:** Bump actions/download-artifact from 7 to 8

### <!-- 2 -->🚜 Refactor

- Resolve all architecture review candidates
- Remove dead code, deduplicate AppState, simplify combined()
- **gui:** Extract views module and responsive layout

### <!-- 3 -->📚 Documentation

- Add Homebrew install command to README
- Restructure README — icon, title, badges, then divider
- Center title and badges, remove divider
- Keep agent guide lifecycle testing guidance generic

### <!-- 5 -->🎨 Styling

- Apply cargo fmt to gui modules

### <!-- 6 -->🧪 Testing

- Maximize coverage — 265 tests, 93.9% line coverage
- Maximize coverage — 324 tests, 99.04% line coverage
- Add firmware pack e2e suite and restore codecov patch coverage

### <!-- 7 -->⚙️ Miscellaneous Tasks

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

### <!-- 0 -->🚀 Features

- Create GitHub Release + auto-update Homebrew tap on tag push

### <!-- 1 -->🐛 Bug Fixes

- Cargo-packager format (msi→wix), cargo fmt, linguist-vendored for Credits.html

### <!-- 2 -->🚜 Refactor

- Split UI into toolbar, status bar, and central content panels
- Align gui with egui native patterns

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Add Codecov coverage reporting via cargo-llvm-cov
- Each build uploads to Release directly, codecov token, homebrew downloads from release
- Split macOS build, homebrew tap only waits for macOS
- Homebrew tap progressive update - arm first, intel silently added later
