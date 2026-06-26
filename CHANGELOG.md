# Changelog

All notable changes to this project are documented here.
Entries are generated from [Conventional Commits](https://www.conventionalcommits.org/).
## [Unreleased]

### <!-- 1 -->🐛 Bug Fixes

- Set product display name to SDF Flash GUI
- Unify SDF Flash GUI display name across all platforms

### <!-- 10 -->💼 Other

- **deps:** Bump env_logger from 0.11.10 to 0.11.11
- **deps:** Bump sha2 from 0.10.9 to 0.11.0
- **deps:** Bump codecov/codecov-action from 5 to 7
- **deps:** Bump actions/download-artifact from 7 to 8

### <!-- 7 -->⚙️ Miscellaneous Tasks

- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md for v0.2.0
- Match x64 DMG filename in homebrew tap release step
- **changelog:** Sync CHANGELOG.md
- Split fmt/clippy checks and update GitHub Actions
- **changelog:** Sync CHANGELOG.md
- **changelog:** Sync CHANGELOG.md
- Extract Homebrew cask template with quarantine postflight
- **changelog:** Sync CHANGELOG.md
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
