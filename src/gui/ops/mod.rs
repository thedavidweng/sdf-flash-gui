mod drives;
mod firmware;
mod labels;
mod lifecycle;
mod nudge;
mod start;

pub use drives::refresh_drives;
pub use firmware::{
    extract_recovery_token_from_wrong_firmware, load_firmware, prompt_recovery_wrong_firmware,
};
pub use labels::{drive_label, firmware_basename, firmware_sha_prefix, flash_mode_label};
pub use lifecycle::{
    confirm_force_kill, confirm_force_quit_exit, confirm_graceful_stop, decline_force_kill,
    on_viewport_close_requested, request_app_quit, request_stop,
};
pub use nudge::{
    click_should_nudge_settings, settings_nudge_active, settings_nudge_highlight,
    SETTINGS_NUDGE_SECONDS,
};
pub use start::{
    backend_configured, can_start, execute_start, on_operation_mode_changed, start_disabled_reason,
};

#[cfg(test)]
mod tests {
    use super::super::file_dialog::FileDialog;
    use super::super::state::{AppState, StopDialog};
    use super::super::OperationMode;
    use super::firmware::firmware_picker_label;
    use super::lifecycle::begin_app_shutdown;
    use super::nudge::{
        half_sine_bell, NUDGE_PULSE1_DUR, NUDGE_PULSE1_START, NUDGE_PULSE2_DUR, NUDGE_PULSE2_GAIN,
        NUDGE_PULSE2_START, NUDGE_REDUCED_MOTION_STRENGTH,
    };
    use super::start::cross_flash_confirmation_required;
    use super::*;
    use crate::drive::Drive;
    use crate::i18n::{t, L10nKey, Language};
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct MockDialog {
        folder: Mutex<Option<PathBuf>>,
        file: Mutex<Option<PathBuf>>,
    }

    impl MockDialog {
        fn returning_nothing() -> Self {
            Self {
                folder: Mutex::new(None),
                file: Mutex::new(None),
            }
        }

        fn returning_folder(path: &str) -> Self {
            Self {
                folder: Mutex::new(Some(PathBuf::from(path))),
                file: Mutex::new(None),
            }
        }

        fn returning_file(path: &str) -> Self {
            Self {
                folder: Mutex::new(None),
                file: Mutex::new(Some(PathBuf::from(path))),
            }
        }
    }

    impl FileDialog for MockDialog {
        fn pick_folder(&self) -> Option<PathBuf> {
            self.folder.lock().unwrap().take()
        }

        fn pick_file_with_title(
            &self,
            _title: &str,
            _filter_name: &str,
            _extensions: &[&str],
            _initial_dir: Option<&std::path::Path>,
        ) -> Option<PathBuf> {
            self.file.lock().unwrap().take()
        }
    }

    fn no_dialog() -> MockDialog {
        MockDialog::returning_nothing()
    }

    fn mock_runner() -> std::sync::Arc<dyn crate::process::ProcessRunner> {
        std::sync::Arc::new(crate::test_support::FakeRunner::spawn_error(
            "mock: not implemented",
        ))
    }

    fn test_drive() -> Drive {
        Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        }
    }

    #[test]
    fn drive_label_with_vendor() {
        let d = test_drive();
        let label = drive_label(&d);
        assert!(label.contains("HL-DT-ST"));
        assert!(label.contains("BU40N"));
        assert!(label.contains("1.03"));
        assert!(label.contains("/dev/sr0"));
    }

    #[test]
    fn backend_configured_empty_path() {
        let state = AppState::new_no_backend();
        assert!(!backend_configured(&state));
    }

    #[test]
    fn click_should_nudge_settings_when_no_backend_and_not_on_allowed() {
        assert!(click_should_nudge_settings(false, false));
        assert!(!click_should_nudge_settings(false, true));
        assert!(!click_should_nudge_settings(true, false));
        assert!(!click_should_nudge_settings(true, true));
    }

    #[test]
    fn settings_nudge_active_respects_deadline() {
        assert!(!settings_nudge_active(None, 10.0));
        assert!(settings_nudge_active(Some(12.0), 10.0));
        assert!(!settings_nudge_active(Some(12.0), 12.0));
        assert!(!settings_nudge_active(Some(12.0), 13.0));
    }

    #[test]
    fn settings_nudge_highlight_two_soft_pulses_with_decay() {
        let start = 100.0;
        let until = start + SETTINGS_NUDGE_SECONDS;
        assert_eq!(settings_nudge_highlight(None, start, false), 0.0);

        let p1_mid = start + f64::from(NUDGE_PULSE1_START + NUDGE_PULSE1_DUR * 0.5);
        let h1 = settings_nudge_highlight(Some(until), p1_mid, false);
        assert!(
            (h1 - 1.0).abs() < 0.02,
            "first peak expected ~1.0, got {h1}"
        );

        let gap_mid = start
            + f64::from(NUDGE_PULSE1_START + NUDGE_PULSE1_DUR)
            + f64::from(NUDGE_PULSE2_START - (NUDGE_PULSE1_START + NUDGE_PULSE1_DUR)) * 0.5;
        let h_gap = settings_nudge_highlight(Some(until), gap_mid, false);
        assert!(h_gap < 0.05, "gap should be near 0, got {h_gap}");

        let p2_mid = start + f64::from(NUDGE_PULSE2_START + NUDGE_PULSE2_DUR * 0.5);
        let h2 = settings_nudge_highlight(Some(until), p2_mid, false);
        assert!(
            (h2 - NUDGE_PULSE2_GAIN).abs() < 0.05,
            "second peak expected ~{}, got {h2}",
            NUDGE_PULSE2_GAIN
        );
        assert!(h2 < h1, "second pulse should be softer than first");

        assert_eq!(settings_nudge_highlight(Some(until), until, false), 0.0);
        assert_eq!(
            settings_nudge_highlight(Some(until), until + 1.0, false),
            0.0
        );
    }

    #[test]
    fn settings_nudge_highlight_reduced_motion_is_steady() {
        let start = 50.0;
        let until = start + SETTINGS_NUDGE_SECONDS;
        let mid = start + SETTINGS_NUDGE_SECONDS * 0.5;
        let h = settings_nudge_highlight(Some(until), mid, true);
        assert!((h - NUDGE_REDUCED_MOTION_STRENGTH).abs() < 1e-5);
        assert_eq!(settings_nudge_highlight(Some(until), until, true), 0.0);
    }

    #[test]
    fn half_sine_bell_is_smooth_zero_peak_zero() {
        assert_eq!(half_sine_bell(-0.1, 0.0, 1.0), 0.0);
        assert_eq!(half_sine_bell(0.0, 0.0, 1.0), 0.0);
        assert!((half_sine_bell(0.5, 0.0, 1.0) - 1.0).abs() < 1e-5);
        assert!((half_sine_bell(1.0, 0.0, 1.0) - 0.0).abs() < 1e-5);
        assert_eq!(half_sine_bell(1.1, 0.0, 1.0), 0.0);
        let a = half_sine_bell(0.25, 0.0, 1.0);
        let b = half_sine_bell(0.75, 0.0, 1.0);
        assert!((a - b).abs() < 1e-5);
        assert!(a > 0.5 && a < 1.0);
    }

    #[test]
    fn settings_nudge_highlight_before_window_start_is_zero() {
        let until = 200.0;
        let before_start = until - SETTINGS_NUDGE_SECONDS - 0.05;
        assert!(before_start < until);
        assert_eq!(
            settings_nudge_highlight(Some(until), before_start, false),
            0.0
        );
        assert_eq!(
            settings_nudge_highlight(Some(until), before_start, true),
            0.0
        );
    }

    #[test]
    fn half_sine_bell_zero_duration_is_zero() {
        assert_eq!(half_sine_bell(0.0, 0.0, 0.0), 0.0);
        assert_eq!(half_sine_bell(0.5, 0.0, f32::EPSILON / 2.0), 0.0);
    }

    #[test]
    fn flash_mode_label_read() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Read;
        assert!(flash_mode_label(&state).contains("Read"));
    }

    #[test]
    fn flash_mode_label_write_standard() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        assert!(flash_mode_label(&state).contains("Standard"));
    }

    #[test]
    fn firmware_sha_prefix_with_data() {
        let mut state = AppState::new_no_backend();
        state.flash.firmware_data = Some(vec![1, 2, 3, 4]);
        let prefix = firmware_sha_prefix(&state);
        assert_eq!(prefix.len(), 8);
        assert_ne!(prefix, "N/A");
    }

    #[test]
    fn flash_mode_label_bootloader() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        state.flash.include_boot_loader = true;
        assert!(flash_mode_label(&state).contains("Boot"));
    }

    #[test]
    fn flash_mode_label_write_encrypted() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        state.flash.encrypted_write = true;
        assert!(flash_mode_label(&state).contains("Encrypted"));
    }

    #[test]
    fn flash_mode_label_recover() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Recover;
        assert!(flash_mode_label(&state).contains("Recovery"));
    }

    #[test]
    fn firmware_sha_prefix_without_data() {
        let state = AppState::new_no_backend();
        assert_eq!(firmware_sha_prefix(&state), "N/A");
    }

    #[test]
    fn firmware_basename_empty_path() {
        let state = AppState::new_no_backend();
        assert!(firmware_basename(&state).contains("N/A"));
    }

    #[test]
    fn firmware_basename_from_path() {
        let mut state = AppState::new_no_backend();
        state.flash.firmware_path = "/tmp/firmware/test_fw.bin".into();
        assert_eq!(firmware_basename(&state), "test_fw.bin");
    }

    #[test]
    fn drive_label_without_vendor() {
        let d = Drive {
            device: "/dev/sr0".into(),
            vendor: String::new(),
            product: String::new(),
            revision: String::new(),
            ..Default::default()
        };
        let label = drive_label(&d);
        assert_eq!(label, "/dev/sr0");
    }

    #[test]
    fn drive_label_includes_serial_when_present() {
        let mut d = test_drive();
        d.serial = "MODJ9TK3546".into();
        let label = drive_label(&d);
        assert!(label.contains("MODJ9TK3546"), "label={label}");
    }

    #[test]
    fn drive_label_without_serial_omits_extra_token() {
        let d = Drive {
            device: "/dev/sr0".into(),
            vendor: "VENDOR".into(),
            product: "PRODUCT".into(),
            revision: "REV".into(),
            ..Default::default()
        };
        assert_eq!(drive_label(&d), "/dev/sr0 VENDOR PRODUCT REV");
    }

    #[test]
    fn can_start_no_drive() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = None;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_busy() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.runtime.busy = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn idle_viewport_close_prepares_exit_without_blocking() {
        let mut state = AppState::new_no_backend();
        state.chrome.show_settings = true;
        state.chrome.show_about = true;

        begin_app_shutdown(&mut state);

        assert!(state.chrome.exiting);
        assert!(!state.chrome.show_settings);
        assert!(!state.chrome.show_about);
        assert!(!state.chrome.show_quit_confirmation);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
    }

    #[test]
    fn begin_app_shutdown_force_kills_active_probe() {
        let mut state = AppState::new_no_backend();
        let control = std::sync::Arc::new(crate::process::OperationControl::new());
        state.runtime.probe_control = Some(control.clone());
        begin_app_shutdown(&mut state);
        assert!(control.is_force_kill_requested());
        assert!(state.runtime.probe_control.is_none());
    }

    #[test]
    fn request_app_quit_when_idle_exits() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        super::request_app_quit(&ctx, &mut state);
        assert!(state.chrome.exiting);
    }

    #[test]
    fn request_app_quit_when_already_exiting_is_idempotent() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        state.chrome.exiting = true;
        super::request_app_quit(&ctx, &mut state);
        assert!(state.chrome.exiting);
        assert!(!state.chrome.show_quit_confirmation);
    }

    #[test]
    fn request_app_quit_when_busy_shows_confirmation() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("busy");
        super::request_app_quit(&ctx, &mut state);
        assert!(state.chrome.show_quit_confirmation);
        assert!(!state.chrome.exiting);
    }

    #[test]
    fn on_viewport_close_when_busy_blocks_exit() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("busy");
        super::on_viewport_close_requested(&ctx, &mut state);
        assert!(state.chrome.show_quit_confirmation);
        assert!(!state.chrome.exiting);
    }

    #[test]
    fn on_viewport_close_when_idle_exits() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        super::on_viewport_close_requested(&ctx, &mut state);
        assert!(state.chrome.exiting);
    }

    #[test]
    fn request_stop_noop_when_idle() {
        let mut state = AppState::new_no_backend();
        super::request_stop(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
    }

    #[test]
    fn request_stop_sets_confirm_dialog_when_busy() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        super::request_stop(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::ConfirmStop);
    }

    #[test]
    fn request_stop_reopens_force_kill_while_waiting_for_backend() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        state.runtime.waiting_for_backend_stop = true;
        state.runtime.stop_dialog = StopDialog::None;
        super::request_stop(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::ConfirmForceKill);
    }

    #[test]
    fn confirm_graceful_stop_clears_dialog() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        state.runtime.stop_dialog = StopDialog::ConfirmStop;
        super::confirm_graceful_stop(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
    }

    #[test]
    fn confirm_force_kill_clears_busy_state() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        state.runtime.stop_dialog = StopDialog::ConfirmForceKill;
        super::confirm_force_kill(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
        assert!(!state.runtime.busy);
        assert!(state.runtime.active_operation.is_none());
        assert_eq!(state.runtime.progress, 0.0);
    }

    #[test]
    fn confirm_force_kill_reaps_probe_control() {
        let mut state = AppState::new_no_backend();
        let control = std::sync::Arc::new(crate::process::OperationControl::new());
        state.runtime.probe_control = Some(control.clone());
        state.runtime.probing = true;
        state.runtime.stop_dialog = StopDialog::ConfirmForceKill;
        super::confirm_force_kill(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
        assert!(state.runtime.probe_control.is_none());
        assert!(!state.runtime.probing);
        assert!(control.is_force_kill_requested());
    }

    #[test]
    fn confirm_force_kill_marks_probe_handled_to_block_auto_reprobe() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing_drive = Some(0);
        state.runtime.probe_control =
            Some(std::sync::Arc::new(crate::process::OperationControl::new()));
        state.runtime.probing = true;
        state.runtime.stop_dialog = StopDialog::ConfirmForceKill;

        super::confirm_force_kill(&mut state);

        assert_eq!(state.drive.last_probed_drive, Some(0));
        assert!(!state.drive.drive_probed);
        assert!(!state.runtime.probing);
        assert!(state.runtime.probing_drive.is_none());
        assert_eq!(
            state.runtime.status_message,
            t(L10nKey::StatusProbeFailed, state.chrome.resolved_lang)
        );
    }

    #[test]
    fn decline_force_kill_keeps_operation_locked_and_dialog() {
        let mut state = AppState::new_no_backend();
        let control = state.begin_operation("running");
        state.runtime.stop_dialog = StopDialog::ConfirmForceKill;
        super::decline_force_kill(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::ConfirmForceKill);
        assert!(state.runtime.busy);
        assert!(state.runtime.waiting_for_backend_stop);
        assert_eq!(
            state
                .runtime
                .active_operation
                .as_ref()
                .map(std::sync::Arc::as_ptr),
            Some(std::sync::Arc::as_ptr(&control))
        );
        assert_eq!(
            state.runtime.status_message,
            t(L10nKey::StatusCancelling, state.chrome.resolved_lang)
        );
    }

    #[test]
    fn confirm_force_quit_exit_clears_busy() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        let ctx = eframe::egui::Context::default();
        super::confirm_force_quit_exit(&ctx, &mut state);
        assert!(!state.runtime.busy);
        assert!(state.runtime.active_operation.is_none());
        assert!(state.chrome.exiting);
    }

    #[test]
    fn confirm_force_quit_exit_force_kills_active_probe() {
        let ctx = eframe::egui::Context::default();
        let mut state = AppState::new_no_backend();
        let control = std::sync::Arc::new(crate::process::OperationControl::new());
        state.runtime.probe_control = Some(control.clone());
        state.runtime.probing = true;
        super::confirm_force_quit_exit(&ctx, &mut state);
        assert!(control.is_force_kill_requested());
        assert!(state.runtime.probe_control.is_none());
        assert!(!state.runtime.probing);
        assert!(state.chrome.exiting);
    }

    #[test]
    fn can_start_probing() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.runtime.probing = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_not_mt1959() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = false;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_read_mode_valid() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let temp_dir = std::env::temp_dir().join("sdf_flash_test_ops");
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();
        state.config.tool_path = tool.to_string_lossy().to_string();
        state.config.sdf_path = String::new();
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn can_start_write_no_firmware() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = None;
        state.flash.firmware_path = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_write_conflicting_modes() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.encrypted_write = true;
        state.flash.include_boot_loader = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_recover_no_token() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.recovery_token = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_recover_wrong_confirmation() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation = "WRONG".into();
        assert!(!can_start(&state));
    }

    #[test]
    fn start_disabled_reason_busy() {
        let mut state = AppState::new_no_backend();
        state.runtime.busy = true;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("progress"));
    }

    #[test]
    fn start_disabled_reason_probing() {
        let mut state = AppState::new_no_backend();
        state.runtime.probing = true;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Probing"));
    }

    #[test]
    fn start_disabled_reason_no_drive() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = None;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("drive"));
    }

    #[test]
    fn start_disabled_reason_not_mt1959() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = false;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("MT1959"));
    }

    fn state_with_valid_paths(suffix: &str) -> (AppState, std::path::PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!("sdf_flash_test_reasons_{suffix}"));
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();
        let mut state = AppState::new_no_backend();
        state.config.tool_path = tool.to_string_lossy().to_string();
        state.config.sdf_path = String::new();
        (state, temp_dir)
    }

    #[test]
    fn start_disabled_reason_write_no_firmware() {
        let (mut state, temp_dir) = state_with_valid_paths("nofw");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = None;
        let reason = start_disabled_reason(&state);
        assert!(
            !reason.is_empty(),
            "reason should not be empty when no firmware"
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_write_conflict() {
        let (mut state, temp_dir) = state_with_valid_paths("conflict");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.encrypted_write = true;
        state.flash.include_boot_loader = true;
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty(), "reason should not be empty");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_recover() {
        let (mut state, temp_dir) = state_with_valid_paths("recover");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.recovery_token = String::new();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty(), "reason should not be empty");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn on_operation_mode_changed_read() {
        let mut state = AppState::new_no_backend();
        state.flash.confirmation = "test".into();
        on_operation_mode_changed(&mut state, OperationMode::Read);
        assert!(state.flash.confirmation.is_empty());
    }

    #[test]
    fn on_operation_mode_changed_write() {
        let mut state = AppState::new_no_backend();
        on_operation_mode_changed(&mut state, OperationMode::Write);
        assert!(state.flash.confirmation.is_empty());
        assert!(
            state.runtime.status_message.contains("firmware"),
            "got: {}",
            state.runtime.status_message
        );
    }

    #[test]
    fn on_operation_mode_changed_recover() {
        let mut state = AppState::new_no_backend();
        on_operation_mode_changed(&mut state, OperationMode::Recover);
        assert!(state.flash.pending_recover_browse);
        assert!(
            state.runtime.status_message.contains("token"),
            "got: {}",
            state.runtime.status_message
        );
    }

    #[test]
    fn load_firmware_nonexistent() {
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, "/nonexistent/path/fw.bin");
        assert!(state.flash.firmware_data.is_none());
        assert!(state.flash.firmware_path == "/nonexistent/path/fw.bin");
        assert!(state.runtime.log_text.contains("ERROR"));
    }

    #[test]
    fn load_firmware_empty_file() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("empty.bin");
        std::fs::write(&file, b"").unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(state.flash.firmware_data.is_none());
        assert!(state.runtime.log_text.contains("empty"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw_valid");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("valid.bin");
        std::fs::write(&file, &[0u8; 1024]).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(state.flash.firmware_data.is_some());
        assert_eq!(state.flash.firmware_data.as_ref().unwrap().len(), 1024);
        assert!(state.runtime.log_text.contains("Loaded firmware"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_drives_empty() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = Some(0);
        refresh_drives(&mut state);
        assert!(
            !state.drive.drives.is_empty() || state.drive.selected_drive.is_none(),
            "when no drives found, selected_drive must be None"
        );
    }

    #[test]
    fn apply_drive_list_selects_first_when_unselected() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = None;
        state.apply_drive_list(vec![test_drive()]);
        assert_eq!(state.drive.selected_drive, Some(0));
        let status = &state.runtime.status_message;
        assert!(status.to_lowercase().contains("ready"), "status: {status}");
    }

    #[test]
    fn apply_drive_list_clears_selection_when_empty() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.apply_drive_list(vec![]);
        assert_eq!(state.drive.selected_drive, None);
        assert!(state.drive.drives.is_empty());
    }

    #[test]
    fn apply_drive_list_reselects_by_identity() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(crate::drive::Drive {
            device: "/dev/sg1".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.apply_drive_list(vec![crate::drive::Drive {
            device: "/dev/sg9".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        }]);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert_eq!(state.drive.drives[0].device, "/dev/sg9");
    }

    #[test]
    fn apply_drive_list_index_shift_same_path_invalidates_probe() {
        let mut state = AppState::new_no_backend();
        let target = crate::drive::Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        };
        state.drive.drives.push(target.clone());
        state.drive.selected_drive = Some(0);
        state.drive.last_probed_drive = Some(0);
        state.drive.drive_probed = true;
        state.drive.drive_mt1959 = true;
        let filler = crate::drive::Drive {
            device: "/dev/sr9".into(),
            vendor: "OTHER".into(),
            product: "X".into(),
            revision: "0".into(),
            ..Default::default()
        };
        state.apply_drive_list(vec![filler, target]);
        assert_eq!(state.drive.selected_drive, Some(1));
        assert!(state.drive.last_probed_drive.is_none());
        assert!(!state.drive.drive_probed);
        assert!(!state.drive.drive_mt1959);
    }

    #[test]
    fn apply_drive_list_same_path_new_identity_invalidates_probe() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(crate::drive::Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.last_probed_drive = Some(0);
        state.drive.drive_probed = true;
        state.drive.drive_mt1959 = true;
        state.drive.drive_encrypted_firmware = true;
        state.drive.drive_sdf_version = Some("0x00A6".into());
        state.apply_drive_list(vec![crate::drive::Drive {
            device: "/dev/sr0".into(),
            vendor: "PIONEER".into(),
            product: "BD-RW".into(),
            revision: "1.00".into(),
            ..Default::default()
        }]);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert!(state.drive.last_probed_drive.is_none());
        assert!(!state.drive.drive_probed);
        assert!(!state.drive.drive_mt1959);
        assert!(!state.drive.drive_encrypted_firmware);
        assert!(state.drive.drive_sdf_version.is_none());
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_empty_path() {
        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = String::new();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert!(state.runtime.log_text.is_empty());
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_nonexistent() {
        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = "/nonexistent/fw.bin".into();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert!(state.runtime.log_text.contains("ERROR"));
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_extract_token");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("wrong.bin");
        let mut data = vec![0u8; 12_288 + 16];
        data[12_288..12_304].copy_from_slice(b"ABCDEFGHIJKLMNOP");
        std::fs::write(&file, &data).unwrap();

        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert_eq!(state.flash.recovery_token, "ABCDEFGHIJKLMNOP");
        assert!(state.runtime.log_text.contains("Extracted"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_too_short() {
        let dir = std::env::temp_dir().join("sdf_flash_test_extract_short");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("short.bin");
        std::fs::write(&file, &[0u8; 100]).unwrap();

        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert!(state.runtime.log_text.contains("ERROR"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prompt_recovery_wrong_firmware_already_set() {
        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = "/some/path.bin".into();
        prompt_recovery_wrong_firmware(&mut state, &no_dialog());
        assert!(state.runtime.log_text.is_empty());
    }

    #[test]
    fn can_start_read_no_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        state.config.tool_path = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_read_invalid_sdf_path() {
        let (mut state, temp_dir) = state_with_valid_paths("invaliddf");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        state.config.sdf_path = "/nonexistent/file.txt".into();
        assert!(!can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_write_validation_fails() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
    }

    #[test]
    fn execute_start_write_success_path() {
        let (mut state, temp_dir) = state_with_valid_paths("exwrite");
        let data = vec![0u8; 1024];
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(data);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(state.runtime.busy, "log: {}", state.runtime.log_text);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_success_path() {
        let (mut state, temp_dir) = state_with_valid_paths("exrecover");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_write_no_drive_selected() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        state.drive.selected_drive = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
    }

    #[test]
    fn execute_start_write_unconfirmed_does_not_start() {
        let (mut state, temp_dir) = state_with_valid_paths("planfail");
        let data = vec![0u8; 16];
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(data);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation = "WRONG".into();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_unconfirmed_does_not_start() {
        let (mut state, temp_dir) = state_with_valid_paths("recplanfail");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.flash.confirmation = "WRONG".into();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn load_firmware_root_path_no_parent() {
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, "/");
        assert!(state.flash.firmware_candidates.is_empty());
    }

    #[test]
    fn start_disabled_reason_read_empty() {
        let (mut state, temp_dir) = state_with_valid_paths("readempty");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let reason = start_disabled_reason(&state);
        assert!(
            reason.is_empty(),
            "read mode should be startable, got: {reason}"
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_write_with_firmware() {
        let (mut state, temp_dir) = state_with_valid_paths("writeok");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_recover_with_token() {
        let (mut state, temp_dir) = state_with_valid_paths("recoverok");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.flash.firmware_path = "fw.bin".into();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_write_valid() {
        let (mut state, temp_dir) = state_with_valid_paths("canwrite");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_write_rejects_empty_firmware_path() {
        let (mut state, temp_dir) = state_with_valid_paths("emptyfwpath");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.firmware_path.clear();
        assert!(!can_start(&state));
        assert_eq!(
            start_disabled_reason(&state),
            t(L10nKey::ReasonNoFirmware, Language::English)
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_write_rejects_mode_conflict() {
        let (mut state, temp_dir) = state_with_valid_paths("modeconflict");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.encrypted_write = true;
        state.flash.include_boot_loader = true;
        assert!(!can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_write_needs_confirmation() {
        let (mut state, temp_dir) = state_with_valid_paths("nomfconfirm");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation.clear();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        assert!(start_disabled_reason(&state).is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_write_prepare_error_logs() {
        let (mut state, temp_dir) = state_with_valid_paths("prepfail");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.encrypted_write = true;
        state.flash.include_boot_loader = true;
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        assert!(
            state.runtime.log_text.contains("ERROR"),
            "log: {}",
            state.runtime.log_text
        );
        assert!(
            state.runtime.log_text.contains("cannot be combined"),
            "log: {}",
            state.runtime.log_text
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn firmware_picker_label_basename_and_fallback() {
        assert_eq!(firmware_picker_label("/tmp/fw.bin"), "fw.bin");
        assert_eq!(firmware_picker_label(""), "");
        assert_eq!(firmware_picker_label(".."), "..");
    }

    #[test]
    fn confirm_graceful_stop_without_active_operation() {
        let mut state = AppState::new_no_backend();
        state.runtime.stop_dialog = StopDialog::ConfirmStop;
        confirm_graceful_stop(&mut state);
        assert_eq!(state.runtime.stop_dialog, StopDialog::None);
    }

    #[test]
    fn decline_force_kill_without_active_operation() {
        let mut state = AppState::new_no_backend();
        state.runtime.waiting_for_backend_stop = false;
        decline_force_kill(&mut state);
        assert!(!state.runtime.waiting_for_backend_stop);
    }

    #[test]
    fn can_start_recover_valid() {
        let (mut state, temp_dir) = state_with_valid_paths("canrecover");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_invalid_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.config.tool_path = "/nonexistent/sdftool".into();
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Invalid tool path"));
    }

    #[test]
    fn start_disabled_reason_invalid_sdf_path() {
        let (mut state, temp_dir) = state_with_valid_paths("invsdf");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.config.sdf_path = "/nonexistent/sdf.bin".into();
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Invalid sdf"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn load_firmware_populates_candidates() {
        let dir = std::env::temp_dir().join("sdf_flash_test_fw_candidates");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.bin"), &[0u8; 10]).unwrap();
        std::fs::write(dir.join("b.bin"), &[0u8; 20]).unwrap();
        std::fs::write(dir.join("c.txt"), b"not a bin").unwrap();

        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &dir.join("a.bin").to_string_lossy());
        assert!(state.flash.firmware_data.is_some());
        assert!(state
            .flash
            .firmware_candidates
            .iter()
            .any(|p| p.contains("a.bin")));
        assert!(state
            .flash
            .firmware_candidates
            .iter()
            .any(|p| p.contains("b.bin")));
        assert!(!state
            .flash
            .firmware_candidates
            .iter()
            .any(|p| p.contains("c.txt")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_start_read_no_drive_selected() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Read;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
    }

    #[test]
    fn execute_start_read_no_folder_selected() {
        let (mut state, temp_dir) = state_with_valid_paths("readnofolder");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_read_with_folder() {
        let (mut state, temp_dir) = state_with_valid_paths("readfolder");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let dialog = MockDialog::returning_folder("/tmp/output");
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &dialog, &mock_runner());
        assert!(state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_read_plan_fails_logs_error() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        state.config.tool_path = String::new();
        let dialog = MockDialog::returning_folder("/tmp/output");
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &dialog, &mock_runner());
        assert!(!state.runtime.busy);
        assert!(state.runtime.log_text.contains("ERROR"));
    }

    #[test]
    fn execute_start_write_dry_run_only_logs_command() {
        let (mut state, temp_dir) = state_with_valid_paths("dryrun");
        let data = vec![0u8; 16];
        let fw_path = temp_dir.join("fw.bin");
        std::fs::write(&fw_path, &data).unwrap();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.dry_run_only = true;
        state.flash.firmware_path = fw_path.to_string_lossy().into();
        state.flash.firmware_data = Some(data);
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        let log = &state.runtime.log_text;
        assert!(
            log.contains("Dry-run"),
            "expected dry-run log entry, log: {log}"
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn apply_drive_list_keeps_existing_selection() {
        let mut state = AppState::new_no_backend();
        let d = test_drive();
        state.drive.drives.push(d.clone());
        state.drive.selected_drive = Some(0);
        state.apply_drive_list(vec![d]);
        assert_eq!(state.drive.selected_drive, Some(0));
    }

    #[test]
    fn mock_runner_run_command_returns_err() {
        let runner = mock_runner();
        let result = runner.run_command("prog", &[], None);
        assert!(result.is_err());
    }

    #[test]
    fn prompt_recovery_wrong_firmware_dialog_no_selection() {
        let mut state = AppState::new_no_backend();
        prompt_recovery_wrong_firmware(&mut state, &no_dialog());
        assert!(state.runtime.log_text.contains("Recover"));
        assert!(state.flash.wrong_firmware_path.is_empty());
    }

    #[test]
    fn prompt_recovery_wrong_firmware_dialog_with_selection() {
        let dir = std::env::temp_dir().join("sdf_flash_test_prompt");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("wrong.bin");
        let mut data = vec![0u8; 12_288 + 16];
        data[12_288..12_304].copy_from_slice(b"ABCDEFGHIJKLMNOP");
        std::fs::write(&file, &data).unwrap();

        let mut state = AppState::new_no_backend();
        let dialog = MockDialog::returning_file(&file.to_string_lossy());
        prompt_recovery_wrong_firmware(&mut state, &dialog);
        assert_eq!(state.flash.wrong_firmware_path, file.to_string_lossy());
        assert_eq!(state.flash.recovery_token, "ABCDEFGHIJKLMNOP");
        let _ = std::fs::remove_dir_all(dir);
    }

    fn build_sdf0_firmware(vendor: &str, model: &str, version: &str) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        let metadata = format!("Vendor\0{vendor}\0Model\0{model}\0Version\0{version}\0");
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_be_bytes());
        data.extend_from_slice(metadata.as_bytes());
        data
    }

    #[test]
    fn load_firmware_sets_form_factor_from_sdf_metadata() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw_ff");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("BU40N_fw.bin");
        let firmware = build_sdf0_firmware("HL-DT-ST", "BU40N", "1.03");
        std::fs::write(&file, &firmware).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert_eq!(
            state.flash.firmware_form_factor,
            crate::platform::DriveFormFactor::Slim
        );
        let resolved = state.flash.firmware_resolved.as_ref().unwrap();
        assert!(resolved.sdf_info.is_some());
        assert_eq!(
            resolved.sdf_info.as_ref().unwrap().model.as_deref(),
            Some("BU40N")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_sets_form_factor_desktop() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw_desktop");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("WH16NS60_fw.bin");
        let firmware = build_sdf0_firmware("HL-DT-ST", "WH16NS60", "1.02");
        std::fs::write(&file, &firmware).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert_eq!(
            state.flash.firmware_form_factor,
            crate::platform::DriveFormFactor::Desktop
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_resets_cross_flash_confirmed() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw_reset");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("fw.bin");
        std::fs::write(&file, &[0u8; 1024]).unwrap();
        let mut state = AppState::new_no_backend();
        state.flash.cross_flash_confirmed = true;
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(!state.flash.cross_flash_confirmed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_identifies_known_firmware_by_hash() {
        let dir = std::env::temp_dir().join("sdf_flash_test_known_hash");
        let _ = std::fs::create_dir_all(&dir);
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot JB8 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BW-16D1HT";
        data[37600..37600 + model.len()].copy_from_slice(model);
        let file = dir.join("renamed_firmware.bin");
        std::fs::write(&file, &data).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert_eq!(
            state.flash.firmware_form_factor,
            crate::platform::DriveFormFactor::Desktop
        );
        let id = state.flash.firmware_resolved.as_ref().unwrap();
        assert_eq!(
            id.identification.binary_info.pcb_type.as_deref(),
            Some("JB8")
        );
        assert_eq!(
            id.identification.binary_info.model.as_deref(),
            Some("BW-16D1HT")
        );
        assert!(id.identification.known.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_identifies_slim_by_pcb_type() {
        let dir = std::env::temp_dir().join("sdf_flash_test_slim_pcb");
        let _ = std::fs::create_dir_all(&dir);
        let mut data = vec![0u8; 40000];
        let boot = b"MT1959 Boot BU5 ";
        data[12288..12288 + boot.len()].copy_from_slice(boot);
        let model = b"BU40N";
        data[37900..37900 + model.len()].copy_from_slice(model);
        let file = dir.join("whatever_name.bin");
        std::fs::write(&file, &data).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert_eq!(
            state.flash.firmware_form_factor,
            crate::platform::DriveFormFactor::Slim
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_unknown_binary_gets_unknown_form_factor() {
        let dir = std::env::temp_dir().join("sdf_flash_test_unknown_bin");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("mystery.bin");
        std::fs::write(&file, &[0u8; 100]).unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert_eq!(
            state.flash.firmware_form_factor,
            crate::platform::DriveFormFactor::Unknown
        );
        let id = state.flash.firmware_resolved.as_ref().unwrap();
        assert!(id.identification.known.is_none());
        assert!(id.identification.binary_info.pcb_type.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_resets_encrypted_to_probe_value() {
        let dir = std::env::temp_dir().join("sdf_flash_test_probe_enc");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("HL-DT-ST_BW-16D1HT_3.10.bin");
        std::fs::write(&file, &[0u8; 100]).unwrap();
        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = true;
        state.flash.encrypted_write = false;
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(
            state.flash.encrypted_write,
            "encrypted_write should reset to probe-detected value"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_detects_encrypted_firmware_file_even_when_drive_not_encrypted() {
        let dir = std::env::temp_dir().join("sdf_flash_test_fw_enc");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("DE_LG_BP50NB40_1.03_MK.bin");
        let mut data = vec![0u8; 1_400_000];
        let date = b"212005070917";
        data[1_370_000..1_370_000 + date.len()].copy_from_slice(date);
        std::fs::write(&file, &data).unwrap();

        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = false;
        state.flash.encrypted_write = false;
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(
            state.flash.encrypted_write,
            "encrypted_write must be true when firmware file is encrypted, even if drive is not"
        );
        assert_eq!(state.flash.firmware_file_encrypted, Some(true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_non_encrypted_firmware_file_with_non_encrypted_drive() {
        let dir = std::env::temp_dir().join("sdf_flash_test_fw_nonenc");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("DE_LG_WH16NS60_1.02_MK.bin");
        let mut data = vec![0u8; 1_400_000];
        let date = b"211810291936";
        data[1_370_000..1_370_000 + date.len()].copy_from_slice(date);
        std::fs::write(&file, &data).unwrap();

        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = false;
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(
            !state.flash.encrypted_write,
            "encrypted_write should be false when neither drive nor firmware is encrypted"
        );
        assert_eq!(state.flash.firmware_file_encrypted, Some(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_firmware_encrypted_file_and_encrypted_drive() {
        let dir = std::env::temp_dir().join("sdf_flash_test_both_enc");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("DE_LG_BP50NB40_1.03_MK.bin");
        let mut data = vec![0u8; 1_400_000];
        let date = b"212005070917";
        data[1_370_000..1_370_000 + date.len()].copy_from_slice(date);
        std::fs::write(&file, &data).unwrap();

        let mut state = AppState::new_no_backend();
        state.drive.drive_encrypted_firmware = true;
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(state.flash.encrypted_write);
        assert_eq!(state.flash.firmware_file_encrypted, Some(true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn start_disabled_reason_recover_empty_when_valid() {
        let (mut state, temp_dir) = state_with_valid_paths("recover_valid");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.recovery_token = "1234567890ABCDEF".into();
        state.flash.confirmation = crate::command::required_flash_confirmation("/dev/sr0");
        assert_eq!(start_disabled_reason(&state), "");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_recover_shows_reason_when_token_missing() {
        let (mut state, temp_dir) = state_with_valid_paths("recover_no_token");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.recovery_token = String::new();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn cross_flash_confirmation_required_no_drive_returns_false() {
        let (mut state, temp_dir) = state_with_valid_paths("no_drive_cross");
        state.drive.selected_drive = None;
        state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Desktop;
        assert!(!cross_flash_confirmation_required(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_blocks_cross_flash_not_confirmed() {
        let (mut state, temp_dir) = state_with_valid_paths("crossflash");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Desktop;
        state.flash.cross_flash_confirmed = false;
        state.flash.confirmation = crate::command::required_flash_confirmation("/dev/sr0");
        assert!(!can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_allows_cross_flash_when_confirmed() {
        let (mut state, temp_dir) = state_with_valid_paths("crossflash_ok");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Desktop;
        state.flash.cross_flash_confirmed = true;
        state.flash.confirmation = crate::command::required_flash_confirmation("/dev/sr0");
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_allows_when_platform_matches() {
        let (mut state, temp_dir) = state_with_valid_paths("platformmatch");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Slim;
        state.flash.cross_flash_confirmed = false;
        state.flash.confirmation = crate::command::required_flash_confirmation("/dev/sr0");
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_cross_flash_not_confirmed() {
        let (mut state, temp_dir) = state_with_valid_paths("reason_crossflash");
        state.drive.drives.push(Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
            ..Default::default()
        });
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Desktop;
        state.flash.cross_flash_confirmed = false;
        state.flash.confirmation = crate::command::required_flash_confirmation("/dev/sr0");
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("cross-flash"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_mt1939() {
        let (mut state, _temp_dir) = state_with_valid_paths("reason_mt1939");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = false;
        state.drive.drive_mt1939 = true;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("MT1939"));
    }
}
