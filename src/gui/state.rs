use crate::command::Backend;
use crate::drive::{self, Drive};
use crate::i18n::{self, t, L10nKey, Language};
use crate::process::OperationControl;

use super::OperationMode;

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopDialog {
    None,
    ConfirmStop,
    ConfirmForceKill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeChoice {
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub fn to_egui(self) -> eframe::egui::ThemePreference {
        match self {
            Self::System => eframe::egui::ThemePreference::System,
            Self::Dark => eframe::egui::ThemePreference::Dark,
            Self::Light => eframe::egui::ThemePreference::Light,
        }
    }
}

/// Persisted user settings, stored via eframe `Storage` and restored on launch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedSettings {
    pub backend: Backend,
    pub tool_path: String,
    pub sdf_path: String,
    pub auto_detected: bool,
    pub language: Language,
    pub theme: ThemeChoice,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            backend: Backend::SdfTool,
            tool_path: String::new(),
            sdf_path: String::new(),
            auto_detected: false,
            language: Language::Auto,
            theme: ThemeChoice::System,
        }
    }
}

/// eframe `Storage` key under which [`PersistedSettings`] is saved.
pub const SETTINGS_STORAGE_KEY: &str = "sdf-flash-gui-settings";

/// Maximum log lines retained in [`OperationRuntime::log_text`].
const LOG_MAX_LINES: usize = 500;

#[derive(Debug)]
pub struct Chrome {
    pub exiting: bool,
    pub show_settings: bool,
    pub show_about: bool,
    pub show_quit_confirmation: bool,
    pub show_flash_failure_dialog: bool,
    /// egui time until which the Settings toolbar button should pulse (no-backend nudge).
    pub settings_nudge_until: Option<f64>,
    pub language: Language,
    pub resolved_lang: Language,
    pub theme: ThemeChoice,
}

#[derive(Debug)]
pub struct ToolConfig {
    pub backend: Backend,
    pub tool_path: String,
    pub sdf_path: String,
    pub auto_detected: bool,
    pub tool_detect_failed: bool,
    pub sdf_detect_failed: bool,
}

#[derive(Debug)]
pub struct DriveState {
    pub drives: Vec<Drive>,
    pub selected_drive: Option<usize>,
    pub last_probed_drive: Option<usize>,
    pub drive_mt1959: bool,
    pub drive_mt1939: bool,
    pub drive_encrypted_firmware: bool,
    pub drive_libredrive: crate::command::LibreDriveStatus,
    pub drive_sdf_version: Option<String>,
    pub drive_probed: bool,
}

#[derive(Debug)]
pub struct FlashWorkflow {
    pub include_boot_loader: bool,
    pub encrypted_write: bool,
    /// Whether the loaded firmware file itself is encrypted (date ≥ 2020).
    /// `None` when no firmware is loaded or encryption could not be determined.
    /// `encrypted_write` is computed as `drive_encrypted OR firmware_file_encrypted`.
    pub firmware_file_encrypted: Option<bool>,
    pub firmware_path: String,
    pub firmware_candidates: Vec<String>,
    pub firmware_picker_items: Vec<(String, String)>,
    pub firmware_data: Option<Vec<u8>>,
    pub firmware_form_factor: crate::platform::DriveFormFactor,
    pub firmware_resolved: Option<crate::firmware_db::ResolvedFirmware>,
    pub cross_flash_confirmed: bool,
    pub confirmation: String,
    pub dry_run_only: bool,
    pub recovery_token: String,
    pub wrong_firmware_path: String,
    pub pending_recover_browse: bool,
}

pub struct OperationRuntime {
    pub status_message: String,
    pub progress: f32,
    pub progress_indeterminate: bool,
    pub busy: bool,
    pub probing: bool,
    pub log_text: String,
    pub stop_dialog: StopDialog,
    pub active_operation: Option<Arc<OperationControl>>,
    pub probe_control: Option<Arc<OperationControl>>,
    /// Drive index for the in-flight auto-probe (if any).
    pub probing_drive: Option<usize>,
    /// User declined force-kill; keep the UI locked until the backend exits.
    pub waiting_for_backend_stop: bool,
}

pub struct AppState {
    pub chrome: Chrome,
    pub config: ToolConfig,
    pub drive: DriveState,
    pub flash: FlashWorkflow,
    pub runtime: OperationRuntime,
    pub operation_mode: OperationMode,
}

impl AppState {
    fn defaults() -> Self {
        Self {
            chrome: Chrome {
                exiting: false,
                show_settings: false,
                show_about: false,
                show_quit_confirmation: false,
                show_flash_failure_dialog: false,
                settings_nudge_until: None,
                language: Language::Auto,
                resolved_lang: Language::English,
                theme: ThemeChoice::System,
            },
            config: ToolConfig {
                backend: Backend::SdfTool,
                tool_path: String::new(),
                sdf_path: String::new(),
                auto_detected: false,
                tool_detect_failed: false,
                sdf_detect_failed: false,
            },
            drive: DriveState {
                drives: Vec::new(),
                selected_drive: None,
                last_probed_drive: None,
                drive_mt1959: false,
                drive_mt1939: false,
                drive_encrypted_firmware: false,
                drive_libredrive: crate::command::LibreDriveStatus::Unknown,
                drive_sdf_version: None,
                drive_probed: false,
            },
            flash: FlashWorkflow {
                include_boot_loader: false,
                encrypted_write: false,
                firmware_file_encrypted: None,
                firmware_path: String::new(),
                firmware_candidates: Vec::new(),
                firmware_picker_items: Vec::new(),
                firmware_data: None,
                firmware_form_factor: crate::platform::DriveFormFactor::Unknown,
                firmware_resolved: None,
                cross_flash_confirmed: false,
                confirmation: String::new(),
                dry_run_only: false,
                recovery_token: String::new(),
                wrong_firmware_path: String::new(),
                pending_recover_browse: false,
            },
            runtime: OperationRuntime {
                status_message: t(L10nKey::StatusReady, Language::English).to_string(),
                progress: 0.0,
                progress_indeterminate: false,
                busy: false,
                probing: false,
                log_text: String::new(),
                stop_dialog: StopDialog::None,
                active_operation: None,
                probe_control: None,
                probing_drive: None,
                waiting_for_backend_stop: false,
            },
            operation_mode: OperationMode::Write,
        }
    }

    /// Build [`AppState`] applying optional persisted settings on top of OS auto-detection.
    pub fn with_persisted(persisted: Option<&PersistedSettings>) -> Self {
        let (detected_backend, detected_path, detected_auto) =
            resolved_discovered_backend(drive::find_backend(Backend::SdfTool));
        let detected_sdf = drive::find_sdf_bin();

        let (backend, tool_path, sdf_path, auto_detected) = match persisted {
            Some(p) if !p.tool_path.is_empty() => (
                p.backend,
                p.tool_path.clone(),
                p.sdf_path.clone(),
                p.auto_detected,
            ),
            _ => (detected_backend, detected_path, detected_sdf, detected_auto),
        };

        let language = persisted.map(|p| p.language).unwrap_or(Language::Auto);
        let theme = persisted.map(|p| p.theme).unwrap_or(ThemeChoice::System);

        Self {
            config: ToolConfig {
                backend,
                tool_path,
                sdf_path,
                auto_detected,
                ..Self::defaults().config
            },
            chrome: Chrome {
                language,
                resolved_lang: i18n::resolve_language(language),
                theme,
                ..Self::defaults().chrome
            },
            ..Self::defaults()
        }
    }

    /// Snapshot the user-configurable fields into a persistable struct.
    pub fn snapshot_settings(&self) -> PersistedSettings {
        PersistedSettings {
            backend: self.config.backend,
            tool_path: self.config.tool_path.clone(),
            sdf_path: self.config.sdf_path.clone(),
            auto_detected: self.config.auto_detected,
            language: self.chrome.language,
            theme: self.chrome.theme,
        }
    }

    pub fn log(&mut self, msg: &str) {
        if !self.runtime.log_text.is_empty() {
            self.runtime.log_text.push('\n');
        }
        self.runtime.log_text.push_str(msg);
        self.trim_log();
    }

    fn trim_log(&mut self) {
        let line_count = self.runtime.log_text.matches('\n').count() + 1;
        if line_count <= LOG_MAX_LINES {
            return;
        }
        let keep = LOG_MAX_LINES - 1;
        let bytes = self.runtime.log_text.as_bytes();
        let mut newlines_seen = 0usize;
        let mut cut_from = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                newlines_seen += 1;
                if newlines_seen == line_count - keep {
                    cut_from = i + 1;
                    break;
                }
            }
        }
        let mut truncated = String::from(t(L10nKey::LogTruncated, self.chrome.resolved_lang));
        truncated.push('\n');
        truncated.push_str(&self.runtime.log_text[cut_from..]);
        self.runtime.log_text = truncated;
    }

    pub fn selected_drive(&self) -> Option<&Drive> {
        self.drive
            .selected_drive
            .and_then(|i| self.drive.drives.get(i))
    }

    pub fn set_status(&mut self, msg: impl Into<String>, progress: f32) {
        self.runtime.status_message = msg.into();
        self.runtime.progress = progress.clamp(0.0, 100.0);
    }

    pub fn set_status_key(&mut self, key: L10nKey, progress: f32) {
        self.set_status(t(key, self.chrome.resolved_lang), progress);
    }

    pub fn begin_operation(&mut self, status: &str) -> Arc<OperationControl> {
        let control = Arc::new(OperationControl::new());
        self.runtime.active_operation = Some(control.clone());
        self.runtime.busy = true;
        self.runtime.progress_indeterminate = true;
        self.runtime.progress = 0.0;
        self.runtime.stop_dialog = StopDialog::None;
        self.set_status(status, 0.0);
        control
    }

    pub fn finish_operation(&mut self) {
        if let Some(control) = self.runtime.active_operation.take() {
            control.reap_registered_child();
        }
        self.runtime.busy = false;
        self.runtime.progress_indeterminate = false;
        self.runtime.stop_dialog = StopDialog::None;
        self.runtime.waiting_for_backend_stop = false;
    }

    /// Record probe results for the selected drive and mark it handled for auto-probe.
    pub fn record_probe_outcome(&mut self, drive_idx: usize, success: bool) {
        if self.drive.selected_drive == Some(drive_idx) {
            self.drive.drive_probed = success;
            self.drive.last_probed_drive = Some(drive_idx);
        }
    }

    pub fn finish_probe_failure(&mut self) {
        if let Some(drive_idx) = self.runtime.probing_drive.or(self.drive.selected_drive) {
            self.record_probe_outcome(drive_idx, false);
        }
        self.finish_probe();
        self.set_status_key(L10nKey::StatusProbeFailed, 0.0);
    }

    pub fn finish_probe(&mut self) {
        if let Some(control) = self.runtime.probe_control.take() {
            control.reap_registered_child();
        }
        self.runtime.probing = false;
        self.runtime.probing_drive = None;
    }

    /// Drop probe-derived flags so Start cannot use results from another drive.
    fn invalidate_probe_cache(&mut self) {
        self.drive.last_probed_drive = None;
        self.drive.drive_probed = false;
        self.drive.drive_mt1959 = false;
        self.drive.drive_mt1939 = false;
        self.drive.drive_encrypted_firmware = false;
        self.drive.drive_libredrive = crate::command::LibreDriveStatus::Unknown;
        self.drive.drive_sdf_version = None;
    }

    /// Recompute `encrypted_write` from the drive's current firmware state and
    /// the loaded firmware file's encryption status.
    ///
    /// `rawflash enc` is needed when **either** the drive's current firmware is
    /// encrypted (date ≥ 2020) **or** the firmware file being written is
    /// encrypted. If no firmware file is loaded, only the drive state matters.
    pub fn recompute_encrypted_write(&mut self) {
        let drive_enc = self.drive.drive_encrypted_firmware;
        let fw_enc = self.flash.firmware_file_encrypted.unwrap_or(false);
        self.flash.encrypted_write = drive_enc || fw_enc;
    }

    /// Replace the drive list and re-select by path / identity (stable after re-enum).
    ///
    /// Single implementation for ops refresh and worker `DrivesListed` (avoids
    /// ops↔workers cycle and dual probe-cache rules).
    pub fn apply_drive_list(&mut self, drives: Vec<Drive>) {
        let previous = self
            .drive
            .selected_drive
            .and_then(|i| self.drive.drives.get(i))
            .cloned();
        let prev_idx = self.drive.selected_drive;
        let old_device = previous.as_ref().map(|d| d.device.as_str());
        let old_identity = previous.as_ref().map(|d| d.identity_key());
        self.drive.drives = drives;
        self.drive.selected_drive =
            drive::resolve_selection(&self.drive.drives, previous.as_ref(), prev_idx);
        let selected = self
            .drive
            .selected_drive
            .and_then(|i| self.drive.drives.get(i));
        let new_device = selected.map(|d| d.device.as_str());
        let new_identity = selected.map(|d| d.identity_key());
        if old_device != new_device
            || self.drive.selected_drive != prev_idx
            || old_identity != new_identity
        {
            self.invalidate_probe_cache();
        }
        if self.drive.drives.is_empty() {
            self.set_status_key(L10nKey::StatusNoDrives, 0.0);
        } else {
            self.set_status_key(L10nKey::StatusReady, 0.0);
        }
    }

    #[cfg(test)]
    pub fn new_no_backend() -> Self {
        Self::defaults()
    }
}

/// Map OS backend discovery into `(backend, tool_path, auto_detected)`.
///
/// Kept free of `drive::` I/O so both outcomes are unit-testable; `AppState::with_persisted`
/// is the only caller that hits the real enumerator.
fn resolved_discovered_backend(found: Option<(Backend, String)>) -> (Backend, String, bool) {
    match found {
        Some((b, p)) => (b, p, true),
        None => (Backend::SdfTool, String::new(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_discovered_backend_some() {
        let (b, p, auto) =
            resolved_discovered_backend(Some((Backend::MakeMkvCon, "/opt/makemkvcon".into())));
        assert_eq!(b, Backend::MakeMkvCon);
        assert_eq!(p, "/opt/makemkvcon");
        assert!(auto);
    }

    #[test]
    fn resolved_discovered_backend_none() {
        let (b, p, auto) = resolved_discovered_backend(None);
        assert_eq!(b, Backend::SdfTool);
        assert!(p.is_empty());
        assert!(!auto);
    }

    #[test]
    fn begin_operation_sets_state() {
        let mut state = AppState::new_no_backend();
        assert!(!state.runtime.busy);
        let _control = state.begin_operation("Writing firmware");
        assert!(state.runtime.busy);
        assert!(state.runtime.active_operation.is_some());
        assert!(state.runtime.progress_indeterminate);
        assert_eq!(state.runtime.status_message, "Writing firmware");
    }

    #[test]
    fn set_status_clamps_progress() {
        let mut state = AppState::new_no_backend();
        state.set_status("test", 150.0);
        assert_eq!(state.runtime.progress, 100.0);
        state.set_status("test", -50.0);
        assert_eq!(state.runtime.progress, 0.0);
    }

    #[test]
    fn log_appends() {
        let mut state = AppState::new_no_backend();
        state.log("first");
        assert_eq!(state.runtime.log_text, "first");
        state.log("second");
        assert_eq!(state.runtime.log_text, "first\nsecond");
    }

    #[test]
    fn selected_drive_none_when_empty() {
        let state = AppState::new_no_backend();
        assert!(state.selected_drive().is_none());
    }

    #[test]
    fn finish_probe_failure_marks_drive_handled() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "V".into(),
            product: "P".into(),
            revision: "R".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.runtime.probing_drive = Some(0);
        state.runtime.probing = true;
        state.runtime.probe_control = Some(Arc::new(OperationControl::new()));

        state.finish_probe_failure();

        assert_eq!(state.drive.last_probed_drive, Some(0));
        assert!(!state.drive.drive_probed);
        assert!(!state.runtime.probing);
        assert!(state.runtime.probe_control.is_none());
    }

    #[test]
    fn new_no_backend_defaults() {
        let state = AppState::new_no_backend();
        assert_eq!(state.runtime.status_message, "Ready");
        assert!(!state.runtime.busy);
        assert!(state.drive.drives.is_empty());
        assert!(state.flash.firmware_data.is_none());
        assert_eq!(state.chrome.resolved_lang, Language::English);
        assert!(state.chrome.settings_nudge_until.is_none());
        assert!(!state.chrome.show_settings);
    }

    #[test]
    fn apply_drive_list_keeps_probe_when_identity_unchanged() {
        let mut state = AppState::new_no_backend();
        let d = Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        };
        state.drive.drives.push(d.clone());
        state.drive.selected_drive = Some(0);
        state.drive.last_probed_drive = Some(0);
        state.drive.drive_probed = true;
        state.drive.drive_mt1959 = true;
        state.apply_drive_list(vec![d]);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert_eq!(state.drive.last_probed_drive, Some(0));
        assert!(state.drive.drive_probed);
        assert!(state.drive.drive_mt1959);
    }

    #[test]
    fn apply_drive_list_clears_probe_when_identity_changes_at_same_path() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "OLD".into(),
            product: "DRIVE".into(),
            revision: "1.00".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.last_probed_drive = Some(0);
        state.drive.drive_probed = true;
        state.drive.drive_mt1959 = true;
        state.drive.drive_encrypted_firmware = true;
        state.apply_drive_list(vec![Drive {
            device: "/dev/sr0".into(),
            vendor: "NEW".into(),
            product: "DRIVE".into(),
            revision: "2.00".into(),
            ..Default::default()
        }]);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert!(state.drive.last_probed_drive.is_none());
        assert!(!state.drive.drive_probed);
        assert!(!state.drive.drive_mt1959);
        assert!(!state.drive.drive_encrypted_firmware);
    }

    #[test]
    fn recompute_encrypted_write_drive_only_when_no_firmware() {
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = true;
        state.flash.firmware_file_encrypted = None;
        state.recompute_encrypted_write();
        assert!(state.flash.encrypted_write);
    }

    #[test]
    fn recompute_encrypted_write_firmware_only_when_drive_not_encrypted() {
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = false;
        state.flash.firmware_file_encrypted = Some(true);
        state.recompute_encrypted_write();
        assert!(state.flash.encrypted_write);
    }

    #[test]
    fn recompute_encrypted_write_neither_encrypted() {
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = false;
        state.flash.firmware_file_encrypted = Some(false);
        state.recompute_encrypted_write();
        assert!(!state.flash.encrypted_write);
    }

    #[test]
    fn recompute_encrypted_write_both_encrypted() {
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = true;
        state.flash.firmware_file_encrypted = Some(true);
        state.recompute_encrypted_write();
        assert!(state.flash.encrypted_write);
    }

    #[test]
    fn recompute_encrypted_write_firmware_none_drive_not_encrypted() {
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = false;
        state.flash.firmware_file_encrypted = None;
        state.recompute_encrypted_write();
        assert!(!state.flash.encrypted_write);
    }

    #[test]
    fn log_truncation_caps_line_count() {
        let mut state = AppState::new_no_backend();
        for i in 0..(LOG_MAX_LINES + 50) {
            state.log(&format!("line {i}"));
        }
        let lines: Vec<&str> = state.runtime.log_text.lines().collect();
        assert!(lines.len() <= LOG_MAX_LINES);
        assert!(state.runtime.log_text.contains("truncated"));
        assert_eq!(lines.last().copied(), Some("line 549"));
    }

    #[test]
    fn log_truncation_not_triggered_under_cap() {
        let mut state = AppState::new_no_backend();
        for i in 0..10 {
            state.log(&format!("line {i}"));
        }
        assert!(!state.runtime.log_text.contains("truncated"));
        assert_eq!(state.runtime.log_text.lines().count(), 10);
    }

    #[test]
    fn snapshot_and_with_persisted_round_trip() {
        let mut state = AppState::new_no_backend();
        state.config.backend = Backend::MakeMkvCon;
        state.config.tool_path = "/custom/makemkvcon".into();
        state.config.sdf_path = "/custom/sdf.bin".into();
        state.config.auto_detected = false;
        state.chrome.language = Language::German;
        state.chrome.theme = ThemeChoice::Light;

        let snapshot = state.snapshot_settings();
        let restored = AppState::with_persisted(Some(&snapshot));
        assert_eq!(restored.config.backend, Backend::MakeMkvCon);
        assert_eq!(restored.config.tool_path, "/custom/makemkvcon");
        assert_eq!(restored.config.sdf_path, "/custom/sdf.bin");
        assert_eq!(restored.chrome.language, Language::German);
        assert_eq!(restored.chrome.theme, ThemeChoice::Light);
    }

    #[test]
    fn with_persisted_falls_back_to_auto_detect_when_tool_path_empty() {
        let persisted = PersistedSettings {
            backend: Backend::SdfTool,
            tool_path: String::new(),
            sdf_path: String::new(),
            auto_detected: false,
            language: Language::French,
            theme: ThemeChoice::Dark,
        };
        let state = AppState::with_persisted(Some(&persisted));
        assert_eq!(state.chrome.language, Language::French);
        assert_eq!(state.chrome.theme, ThemeChoice::Dark);
    }

    #[test]
    fn theme_choice_to_egui_mapping() {
        assert_eq!(
            ThemeChoice::System.to_egui(),
            eframe::egui::ThemePreference::System
        );
        assert_eq!(
            ThemeChoice::Dark.to_egui(),
            eframe::egui::ThemePreference::Dark
        );
        assert_eq!(
            ThemeChoice::Light.to_egui(),
            eframe::egui::ThemePreference::Light
        );
    }

    #[test]
    fn persisted_settings_default_is_system_auto() {
        let d = PersistedSettings::default();
        assert_eq!(d.backend, Backend::SdfTool);
        assert_eq!(d.language, Language::Auto);
        assert_eq!(d.theme, ThemeChoice::System);
        assert!(d.tool_path.is_empty());
    }
}
