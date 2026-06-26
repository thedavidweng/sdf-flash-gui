# Changelog

All notable changes to this project are documented here.
Entries are generated from [Conventional Commits](https://www.conventionalcommits.org/).
## [Unreleased]

### <!-- 0 -->🚀 Features

- Auto-detect feedback in validation line
- Add firmware safety warnings (category, wildcard, SDF metadata)
- Wire up i18n dispatch and add German translations
- Complete translations for all 30 languages (58 keys each)
- I18n for GUI-generated logs and errors across all locales
- **gui:** Phosphor icons, egui 0.34, and reliable fast quit

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
- Wire up orphaned i18n keys to actual GUI strings
- Make settings/about windows resizable with min size and scroll safety net
- Remove ScrollArea from settings window so buttons stay near content
- Settings window columns back to Grid, remove ScrollArea
- Increase settings window height to 400px for bottom buttons
- Request repaint on language switch for hot-update
- Repaint main window on language switch via app context
- Suppress unreachable_patterns warning in translations macro

### <!-- 2 -->🚜 Refactor

- Resolve all architecture review candidates
- Remove dead code, deduplicate AppState, simplify combined()
- Add browse ops for manifest and wrong firmware files
- Extract reusable file_picker widget
- Mark browse ops as dead_code (replaced by file_picker widget)
- Unify file pickers and add spacing constants
- Wire up orphaned logic and remove dead code
- Simplify logic, remove redundant tests and comments
- Settings layout — Grid to columns, buttons right-aligned
- Convert main window Grids to columns for auto-fill width
- Deepen orchestration and unify version to 0.2.0

### <!-- 3 -->📚 Documentation

- Add Homebrew install command to README
- Restructure README — icon, title, badges, then divider
- Center title and badges, remove divider

### <!-- 5 -->🎨 Styling

- Apply cargo fmt to gui modules

### <!-- 6 -->🧪 Testing

- Maximize coverage — 265 tests, 93.9% line coverage
- Maximize coverage — 324 tests, 99.04% line coverage

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Simplify homebrew tap, add codecov config and badges
- Replace deprecated macos-13 with macos-15-intel
- Exclude drive.rs from coverage — platform-specific code untestable on Linux
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
