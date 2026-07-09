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
    pub show_first_run_setup: bool,
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
    pub drive_encrypted_firmware: bool,
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
                show_first_run_setup: false,
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
                drive_encrypted_firmware: false,
                drive_probed: false,
            },
            flash: FlashWorkflow {
                include_boot_loader: false,
                encrypted_write: false,
                firmware_path: String::new(),
                firmware_candidates: Vec::new(),
                firmware_picker_items: Vec::new(),
                firmware_data: None,
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
        let (backend, path, auto) = match drive::find_backend() {
            Some((b, p)) => (b, p, true),
            None => (Backend::SdfTool, String::new(), false),
        };
        let show_first_run = path.is_empty();
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
                show_first_run_setup: show_first_run,
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

    #[cfg(test)]
    pub fn new_no_backend() -> Self {
        Self::defaults()
    }
}

/// Locate `sdf.bin` (re-export for GUI callers; logic lives in `drive`).
pub fn find_sdf_bin() -> String {
    drive::find_sdf_bin()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn find_sdf_bin_matches_drive_module() {
        // Re-export must stay in sync with drive::find_sdf_bin.
        assert_eq!(find_sdf_bin(), drive::find_sdf_bin());
    }

    #[test]
    fn new_no_backend_defaults() {
        let state = AppState::new_no_backend();
        assert_eq!(state.runtime.status_message, "Ready");
        assert!(!state.runtime.busy);
        assert!(state.drive.drives.is_empty());
        assert!(state.flash.firmware_data.is_none());
        assert_eq!(state.chrome.resolved_lang, Language::English);
    }
}
