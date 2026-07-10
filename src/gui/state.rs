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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Dark,
    Light,
}

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
    pub firmware_path: String,
    pub firmware_candidates: Vec<String>,
    pub firmware_picker_items: Vec<(String, String)>,
    pub firmware_data: Option<Vec<u8>>,
    pub firmware_form_factor: crate::platform::DriveFormFactor,
    pub firmware_sdf_info: Option<crate::flash::FirmwareSdfInfo>,
    pub firmware_identification: Option<crate::firmware_db::FirmwareIdentification>,
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
                firmware_path: String::new(),
                firmware_candidates: Vec::new(),
                firmware_picker_items: Vec::new(),
                firmware_data: None,
                firmware_form_factor: crate::platform::DriveFormFactor::Unknown,
                firmware_sdf_info: None,
                firmware_identification: None,
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

    pub fn new() -> Self {
        let (backend, path, auto) =
            resolved_discovered_backend(drive::find_backend(Backend::SdfTool));
        Self {
            config: ToolConfig {
                backend,
                tool_path: path,
                sdf_path: drive::find_sdf_bin(),
                auto_detected: auto,
                ..Self::defaults().config
            },
            chrome: Chrome {
                resolved_lang: i18n::detect_system_language(),
                ..Self::defaults().chrome
            },
            ..Self::defaults()
        }
    }

    pub fn log(&mut self, msg: &str) {
        if !self.runtime.log_text.is_empty() {
            self.runtime.log_text.push('\n');
        }
        self.runtime.log_text.push_str(msg);
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
        // Path, selection index, or hardware identity at the same path changed
        // (e.g. different drive re-enumerated as /dev/sr0).
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
/// Kept free of `drive::` I/O so both outcomes are unit-testable; `AppState::new`
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
}
