// Pure helpers that operate on AppState — no UI rendering or thread spawning.

mod labels;

pub use labels::{drive_label, firmware_basename, firmware_sha_prefix, flash_mode_label};

use crate::command;
use crate::drive;
use crate::flash;
use crate::i18n::{log_error, t, t_with_args, L10nKey, Language};
use crate::manifest;
use crate::orchestration;
use crate::process;

use super::file_dialog::FileDialog;
use super::state::{AppState, StopDialog};
use super::validation::{validate_sdf_path, validate_tool_path};
use super::workers::{spawn_streaming_command, WorkerMsg};
use super::OperationMode;
use crate::process::ProcessRunner;

use std::sync::mpsc::Sender;

fn begin_app_shutdown(state: &mut AppState) {
    state.chrome.exiting = true;
    state.chrome.show_settings = false;
    state.chrome.show_about = false;
    state.chrome.show_quit_confirmation = false;
    state.runtime.stop_dialog = StopDialog::None;
    if let Some(control) = state.runtime.probe_control.take() {
        control.request_force_kill();
        control.reap_registered_child();
    }
    state.runtime.probing = false;
}

fn close_child_viewports(ctx: &eframe::egui::Context) {
    use eframe::egui::{ViewportCommand, ViewportId};
    ctx.send_viewport_cmd_to(
        ViewportId::from_hash_of("settings_viewport"),
        ViewportCommand::Close,
    );
    ctx.send_viewport_cmd_to(
        ViewportId::from_hash_of("about_viewport"),
        ViewportCommand::Close,
    );
}

fn prepare_app_exit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.chrome.exiting {
        return;
    }
    begin_app_shutdown(state);
    close_child_viewports(ctx);
    ctx.request_repaint();
}

pub fn request_app_quit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.runtime.busy {
        state.chrome.show_quit_confirmation = true;
    } else {
        prepare_app_exit(ctx, state);
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
    }
}

pub fn on_viewport_close_requested(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.runtime.busy {
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
        state.chrome.show_quit_confirmation = true;
    } else {
        prepare_app_exit(ctx, state);
    }
}

/// Force-kill any running backend and close immediately.
///
/// Clears `busy` before requesting close so `on_viewport_close_requested` does not
/// re-issue `CancelClose` while the worker is still winding down.
pub fn confirm_force_quit_exit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if let Some(control) = state.runtime.probe_control.as_ref() {
        control.request_force_kill();
    }
    if let Some(control) = state.runtime.active_operation.as_ref() {
        control.request_force_kill();
    }
    state.finish_probe();
    state.finish_operation();
    prepare_app_exit(ctx, state);
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
}

pub fn request_stop(state: &mut AppState) {
    if !state.runtime.busy {
        return;
    }
    state.runtime.stop_dialog = if state.runtime.waiting_for_backend_stop {
        StopDialog::ConfirmForceKill
    } else {
        StopDialog::ConfirmStop
    };
}

pub fn confirm_graceful_stop(state: &mut AppState) {
    if let Some(control) = &state.runtime.active_operation {
        control.request_graceful_cancel();
        state.set_status_key(L10nKey::StatusCancelling, state.runtime.progress);
    }
    state.runtime.stop_dialog = StopDialog::None;
}

pub fn confirm_force_kill(state: &mut AppState) {
    if let Some(control) = state.runtime.probe_control.as_ref() {
        control.request_force_kill();
    }
    if let Some(control) = state.runtime.active_operation.as_ref() {
        control.request_force_kill();
    }
    state.log(t(L10nKey::LogOpCancelled, state.chrome.resolved_lang));
    if state.runtime.probe_control.is_some() {
        state.finish_probe_failure();
    }
    if state.runtime.busy {
        state.finish_operation();
        state.set_status_key(L10nKey::StatusOpCancelled, 0.0);
    } else {
        state.runtime.stop_dialog = StopDialog::None;
    }
}

pub fn decline_force_kill(state: &mut AppState) {
    // User chose to wait: keep busy + active_operation (or an active probe) so another
    // flash cannot start while the backend is still mutating the drive. Leave the
    // force-kill dialog open so the user can retry a hard stop if needed.
    if state.runtime.active_operation.is_some() {
        state.runtime.waiting_for_backend_stop = true;
        state.set_status_key(L10nKey::StatusCancelling, state.runtime.progress);
    }
}

pub fn can_start(state: &AppState) -> bool {
    if state.runtime.busy
        || state.runtime.probing
        || state.selected_drive().is_none()
        || !state.drive.drive_mt1959
    {
        return false;
    }
    if validate_tool_path(
        &state.config.tool_path,
        state.config.backend,
        crate::i18n::Language::English,
    )
    .is_err()
    {
        return false;
    }
    if validate_sdf_path(&state.config.sdf_path, crate::i18n::Language::English).is_err() {
        return false;
    }
    match state.operation_mode {
        OperationMode::Read => true,
        OperationMode::Write => {
            if state.flash.firmware_data.is_none() || state.flash.firmware_path.is_empty() {
                return false;
            }
            if command::write_modes_conflict(
                state.flash.encrypted_write,
                state.flash.include_boot_loader,
            ) {
                return false;
            }
            // With manifest: gates must pass (flash_report). Without: confirmation only
            // (same as CLI flash without --manifest).
            if state.flash.manifest.is_some() {
                state
                    .flash
                    .flash_report
                    .as_ref()
                    .is_some_and(|r| r.would_execute)
            } else {
                state.selected_drive().is_some_and(|d| {
                    command::confirmation_matches(&d.device, &state.flash.confirmation)
                })
            }
        }
        OperationMode::Recover => {
            !state.flash.firmware_path.is_empty()
                && state.flash.recovery_token.len() == 16
                && state.selected_drive().is_some_and(|d| {
                    command::confirmation_matches(&d.device, &state.flash.confirmation)
                })
        }
    }
}

pub fn start_disabled_reason(state: &AppState) -> String {
    let lang = state.chrome.resolved_lang;
    if state.runtime.busy {
        return t(L10nKey::ReasonBusy, lang).to_string();
    }
    if state.runtime.probing {
        return t(L10nKey::ReasonProbing, lang).to_string();
    }
    if state.selected_drive().is_none() {
        return t(L10nKey::ReasonNoDrive, lang).to_string();
    }
    if !state.drive.drive_mt1959 {
        return t(L10nKey::ReasonNotMt1959, lang).to_string();
    }
    if let Err(e) = validate_tool_path(&state.config.tool_path, state.config.backend, lang) {
        return t_with_args(L10nKey::ReasonInvalidToolPath, lang, &[("error", &e)]);
    }
    if let Err(e) = validate_sdf_path(&state.config.sdf_path, lang) {
        return t_with_args(L10nKey::ReasonInvalidSdfPath, lang, &[("error", &e)]);
    }
    match state.operation_mode {
        OperationMode::Read => String::new(),
        OperationMode::Write => {
            if state.flash.firmware_data.is_none() {
                return t(L10nKey::ReasonNoFirmware, lang).to_string();
            }
            if command::write_modes_conflict(
                state.flash.encrypted_write,
                state.flash.include_boot_loader,
            ) {
                return t(L10nKey::ReasonConflict, lang).to_string();
            }
            if state.flash.manifest.is_none() {
                let device = state
                    .selected_drive()
                    .map(|d| d.device.as_str())
                    .unwrap_or("");
                if !command::confirmation_matches(device, &state.flash.confirmation) {
                    return t(L10nKey::ReasonEnterToken, lang).to_string();
                }
                return String::new();
            }
            t(L10nKey::ReasonRunValidation, lang).to_string()
        }
        OperationMode::Recover => t(L10nKey::ReasonEnterToken, lang).to_string(),
    }
}

/// Returns true when a valid backend executable is configured.
pub fn backend_configured(state: &AppState) -> bool {
    validate_tool_path(
        &state.config.tool_path,
        state.config.backend,
        Language::English,
    )
    .is_ok()
}

pub fn on_operation_mode_changed(state: &mut AppState, mode: OperationMode) {
    state.flash.flash_report = None;
    state.flash.confirmation.clear();
    match mode {
        OperationMode::Read => {
            state.set_status_key(L10nKey::StatusHintRead, 0.0);
        }
        OperationMode::Write => {
            state.set_status_key(L10nKey::StatusHintWrite, 0.0);
        }
        OperationMode::Recover => {
            state.set_status_key(L10nKey::StatusHintRecover, 0.0);
            state.flash.pending_recover_browse = true;
        }
    }
}

pub fn validate_flash(state: &mut AppState) {
    let Some(drive) = state.selected_drive().cloned() else {
        return;
    };
    let Some(firmware_data) = state.flash.firmware_data.clone() else {
        return;
    };
    let lang = state.chrome.resolved_lang;
    let drive_match = state
        .drive_match()
        .expect("selected drive implies drive match");
    let confirm = orchestration::FlashConfirm::Typed(state.flash.confirmation.clone());

    // Shared prepare path (no re-probe) — same gates/plan as CLI FlashSession after probe.
    match orchestration::prepare_firmware_op(orchestration::FirmwareOpRequest {
        backend: state.config.backend,
        tool_path: &state.config.tool_path,
        sdf_path: &state.config.sdf_path,
        device: &drive.device,
        drive_is_mt1959: state.drive.drive_mt1959,
        drive_match: &drive_match,
        firmware_path: &state.flash.firmware_path,
        firmware_data: &firmware_data,
        manifest: state.flash.manifest.as_ref(),
        image_id: state.flash.selected_image_id.as_deref(),
        encrypted: state.flash.encrypted_write,
        include_boot_loader: state.flash.include_boot_loader,
        recover: false,
        wrong_firmware: None,
        recovery_token: None,
        confirm,
        lang,
    }) {
        Ok(prepared) => {
            for w in &prepared.no_manifest_warnings {
                state.log(&format!("WARNING: {w}"));
            }
            if let Some(report) = prepared.report {
                for w in &report.warnings {
                    state.log(&format!("WARNING: {w}"));
                }
                state.log(&report.summary);
                state.flash.flash_report = Some(report);
            } else {
                state.flash.flash_report = None;
                if prepared.would_execute {
                    state.log(t(L10nKey::StatusReady, lang));
                }
                if !prepared.would_execute && state.flash.manifest.is_none() {
                    state.log(t(L10nKey::LogErrLoadManifestBeforeValidate, lang));
                }
            }
        }
        Err(e) => {
            state.log(&e);
            state.flash.flash_report = None;
        }
    }
}

pub fn execute_start(
    state: &mut AppState,
    worker_tx: &Sender<WorkerMsg>,
    dialog: &impl FileDialog,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    let lang = state.chrome.resolved_lang;

    match state.operation_mode {
        OperationMode::Read => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let Some(folder) = dialog.pick_folder() else {
                return;
            };
            let output_dir = folder.to_string_lossy().to_string();
            match orchestration::plan_read(
                state.config.backend,
                &state.config.tool_path,
                &state.config.sdf_path,
                &drive.device,
                &output_dir,
                state.drive.drive_mt1959,
            ) {
                Ok(plan) => {
                    let status = t(L10nKey::StatusReadingFirmware, lang);
                    let control = state.begin_operation(status);
                    spawn_streaming_command(worker_tx, plan.command, status, runner, lang, control);
                }
                Err(e) => state.log(&log_error(lang, &e)),
            }
        }
        OperationMode::Write | OperationMode::Recover => {
            let Some(drive) = state.selected_drive().cloned() else {
                return;
            };
            let firmware_data = state.flash.firmware_data.clone().unwrap_or_default();
            let recover = matches!(state.operation_mode, OperationMode::Recover);
            if !recover && state.flash.firmware_data.is_none() {
                return;
            }
            let drive_match = state
                .drive_match()
                .expect("selected drive implies drive match");
            let confirm = orchestration::FlashConfirm::Typed(state.flash.confirmation.clone());

            let prepared =
                match orchestration::prepare_firmware_op(orchestration::FirmwareOpRequest {
                    backend: state.config.backend,
                    tool_path: &state.config.tool_path,
                    sdf_path: &state.config.sdf_path,
                    device: &drive.device,
                    drive_is_mt1959: state.drive.drive_mt1959,
                    drive_match: &drive_match,
                    firmware_path: &state.flash.firmware_path,
                    firmware_data: &firmware_data,
                    manifest: if recover {
                        None
                    } else {
                        state.flash.manifest.as_ref()
                    },
                    image_id: state.flash.selected_image_id.as_deref(),
                    encrypted: state.flash.encrypted_write,
                    include_boot_loader: state.flash.include_boot_loader,
                    recover,
                    wrong_firmware: None,
                    recovery_token: if recover {
                        Some(state.flash.recovery_token.as_str())
                    } else {
                        None
                    },
                    confirm,
                    lang,
                }) {
                    Ok(p) => p,
                    Err(e) => {
                        state.log(&log_error(lang, &e));
                        return;
                    }
                };

            for w in &prepared.no_manifest_warnings {
                state.log(&format!("WARNING: {w}"));
            }
            if let Some(report) = &prepared.report {
                for w in &report.warnings {
                    state.log(&format!("WARNING: {w}"));
                }
                state.log(&report.summary);
                state.flash.flash_report = Some(report.clone());
            }

            // prepare_firmware_op only yields a plan when would_execute is true.
            let Some(plan) = prepared.plan else {
                return;
            };

            if !recover && state.flash.dry_run_only {
                state.log(&t_with_args(
                    L10nKey::LogDryRunCommand,
                    lang,
                    &[("command", &process::format_command(&plan.command))],
                ));
                state.set_status_key(L10nKey::StatusReady, 100.0);
                return;
            }

            let status = if recover {
                t(L10nKey::StatusRecoveringDrive, lang)
            } else {
                t(L10nKey::StatusWritingFirmware, lang)
            };
            let control = state.begin_operation(status);
            spawn_streaming_command(worker_tx, plan.command, status, runner, lang, control);
        }
    }
}

fn finalize_drive_selection(state: &mut AppState) {
    if state.drive.drives.is_empty() {
        state.drive.selected_drive = None;
        state.set_status_key(L10nKey::StatusNoDrives, 0.0);
        return;
    }
    if state.drive.selected_drive.is_some() {
        return;
    }
    state.drive.selected_drive = Some(0);
    state.set_status_key(L10nKey::StatusReady, 0.0);
}

pub fn refresh_drives(state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    state.drive.drives = drive::enumerate_drives();
    state.drive.last_probed_drive = None;
    state.log(&t_with_args(
        L10nKey::StatusDrivesFound,
        lang,
        &[("count", &state.drive.drives.len().to_string())],
    ));
    finalize_drive_selection(state);
}

/// Display label for a firmware candidate path (basename, or full path as fallback).
pub(crate) fn firmware_picker_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string())
}

pub fn load_firmware(state: &mut AppState, path: &str) {
    let lang = state.chrome.resolved_lang;
    state.flash.firmware_path = path.to_string();
    state.flash.flash_report = None;
    match std::fs::read(path) {
        Ok(data) => {
            if data.is_empty() {
                state.log(&t_with_args(
                    L10nKey::LogFirmwareEmpty,
                    lang,
                    &[("path", path)],
                ));
                state.flash.firmware_data = None;
            } else {
                state.flash.firmware_data = Some(data);
            }
        }
        Err(e) => {
            state.log(&t_with_args(
                L10nKey::LogFirmwareReadFailed,
                lang,
                &[("path", path), ("error", &e.to_string())],
            ));
            state.flash.firmware_data = None;
        }
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        state.flash.firmware_candidates = std::fs::read_dir(parent)
            .map(|entries| {
                let mut files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "bin"))
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                files.sort();
                files
            })
            .unwrap_or_default();
    }

    state.flash.firmware_picker_items = state
        .flash
        .firmware_candidates
        .iter()
        .map(|path| (firmware_picker_label(path), path.clone()))
        .collect();

    if let Some(data) = &state.flash.firmware_data {
        state.log(&t_with_args(
            L10nKey::LogFirmwareLoaded,
            lang,
            &[
                ("path", path),
                ("size", &data.len().to_string()),
                ("hash", &flash::sha256_hex(data)[..16]),
            ],
        ));
    }
}

pub fn load_manifest(state: &mut AppState, path: &str) {
    let lang = state.chrome.resolved_lang;
    state.flash.manifest_path = path.to_string();
    match std::fs::read(path) {
        Ok(data) => match manifest::parse_manifest(&data) {
            Ok(m) => {
                state.log(&t_with_args(
                    L10nKey::LogManifestLoaded,
                    lang,
                    &[
                        ("vendor", &m.vendor),
                        ("model", &m.model),
                        ("count", &m.firmware_images.len().to_string()),
                    ],
                ));
                state.flash.selected_image_id = if m.firmware_images.len() == 1 {
                    Some(m.firmware_images[0].image_id.clone())
                } else {
                    None
                };
                state.flash.manifest = Some(m);
                state.flash.flash_report = None;
            }
            Err(e) => state.log(&t_with_args(
                L10nKey::LogManifestInvalid,
                lang,
                &[("error", &e.to_string())],
            )),
        },
        Err(e) => state.log(&t_with_args(
            L10nKey::LogManifestReadFailed,
            lang,
            &[("error", &e.to_string())],
        )),
    }
}

pub fn prompt_recovery_wrong_firmware(state: &mut AppState, dialog: &impl FileDialog) {
    if !state.flash.wrong_firmware_path.is_empty() {
        return;
    }
    let lang = state.chrome.resolved_lang;
    state.log(t(L10nKey::LogRecoverSelectWrongFw, lang));
    if let Some(file) = dialog.pick_file_with_title(
        t(L10nKey::DialogTitleWrongFirmware, lang),
        "Firmware",
        &["bin"],
        None,
    ) {
        state.flash.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(state);
    }
}

pub fn extract_recovery_token_from_wrong_firmware(state: &mut AppState) {
    if state.flash.wrong_firmware_path.is_empty() {
        return;
    }
    let lang = state.chrome.resolved_lang;
    let path = state.flash.wrong_firmware_path.clone();
    match orchestration::resolve_recovery_token(Some(&path), None) {
        Ok(token) => {
            state.flash.recovery_token = token.clone();
            state.log(&t_with_args(
                L10nKey::LogRecoveryTokenExtracted,
                lang,
                &[("token", &token)],
            ));
        }
        Err(e) => state.log(&log_error(lang, &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::file_dialog::FileDialog;
    use super::super::state::AppState;
    use super::super::OperationMode;
    use super::*;
    use crate::drive::Drive;
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

    struct MockRunner;

    impl crate::process::ProcessRunner for MockRunner {
        fn run_command(
            &self,
            _program: &str,
            _args: &[String],
            _control: Option<&crate::process::OperationControl>,
        ) -> Result<crate::process::CommandRunOutcome, String> {
            Err("mock: not implemented".into())
        }

        fn run_command_streaming(
            &self,
            _program: &str,
            _args: &[String],
            _on_line: &dyn Fn(&str),
            _control: Option<&crate::process::OperationControl>,
        ) -> Result<crate::process::CommandRunOutcome, String> {
            Err("mock: not implemented".into())
        }
    }

    fn mock_runner() -> std::sync::Arc<dyn crate::process::ProcessRunner> {
        std::sync::Arc::new(MockRunner)
    }

    fn test_drive() -> Drive {
        Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
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
    fn flash_mode_label_read() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Read;
        assert!(flash_mode_label(&state).contains("READ"));
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
        };
        let label = drive_label(&d);
        assert_eq!(label, "/dev/sr0");
    }

    #[test]
    fn drive_serial_hint_basic() {
        let d = test_drive();
        let hint = labels::drive_serial_hint(&d);
        // "HL-DT-ST_BU40N_1.03" split by ['_', '-', ' '] → ["HL", "DT", "ST", "BU40N", "1.03"]
        // skip 2 → "ST BU40N 1.03"
        assert_eq!(hint, "ST BU40N 1.03");
    }

    #[test]
    fn drive_serial_hint_longer() {
        let d = Drive {
            device: "/dev/sr0".into(),
            vendor: "VENDOR".into(),
            product: "PRODUCT".into(),
            revision: "REV".into(),
        };
        let hint = labels::drive_serial_hint(&d);
        assert_eq!(hint, "REV");
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

        super::begin_app_shutdown(&mut state);

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
        super::begin_app_shutdown(&mut state);
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
        // Need a valid tool path — create a temp file
        let temp_dir = std::env::temp_dir().join("sdf_flash_test_ops");
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();
        state.config.tool_path = tool.to_string_lossy().to_string();
        // validate_sdf_path with empty is ok
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
        state.config.sdf_path = String::new(); // empty sdf_path is OK (validate_sdf_path returns Ok for empty)
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
        state.flash.flash_report = Some(crate::flash::FlashReport {
            would_execute: true,
            direction: crate::flash::FlashDirection::Upgrade,
            checks: crate::flash::FlashChecks {
                model_match: true,
                revision_check: true,
                image_checksum: true,
                signature_present: true,
                user_confirmed: true,
            },
            summary: "test".into(),
            warnings: vec![],
        });
        state.flash.confirmation = "test".into();
        on_operation_mode_changed(&mut state, OperationMode::Read);
        assert!(state.flash.flash_report.is_none());
        assert!(state.flash.confirmation.is_empty());
    }

    #[test]
    fn on_operation_mode_changed_write() {
        let mut state = AppState::new_no_backend();
        on_operation_mode_changed(&mut state, OperationMode::Write);
        assert!(state.flash.flash_report.is_none());
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
    fn load_manifest_nonexistent() {
        let mut state = AppState::new_no_backend();
        load_manifest(&mut state, "/nonexistent/manifest.json");
        assert!(state.flash.manifest.is_none());
        assert!(state.runtime.log_text.contains("ERROR"));
    }

    #[test]
    fn load_manifest_valid() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_mf");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("manifest.json");
        let json = r#"{
            "schema_version": 1,
            "vendor": "HL-DT-ST",
            "model": "BU40N",
            "revision_match": "*",
            "firmware_images": [{
                "image_id": "main",
                "filename": "fw.bin",
                "target_version": "1.04",
                "size": 1024,
                "sha256": "abcd"
            }]
        }"#;
        std::fs::write(&file, json).unwrap();
        let mut state = AppState::new_no_backend();
        load_manifest(&mut state, &file.to_string_lossy());
        assert!(state.flash.manifest.is_some());
        assert_eq!(state.flash.selected_image_id.as_deref(), Some("main"));
        assert!(state.runtime.log_text.contains("Loaded manifest"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_invalid_json() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_mf_bad");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("bad.json");
        std::fs::write(&file, "not json").unwrap();
        let mut state = AppState::new_no_backend();
        load_manifest(&mut state, &file.to_string_lossy());
        assert!(state.flash.manifest.is_none());
        assert!(state.runtime.log_text.contains("invalid manifest"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_multiple_images() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_mf_multi");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("manifest.json");
        let json = r#"{
            "schema_version": 1,
            "vendor": "V",
            "model": "M",
            "revision_match": "*",
            "firmware_images": [
                {"image_id": "a", "filename": "a.bin", "target_version": "1.0", "size": 100, "sha256": "aa"},
                {"image_id": "b", "filename": "b.bin", "target_version": "2.0", "size": 200, "sha256": "bb"}
            ]
        }"#;
        std::fs::write(&file, json).unwrap();
        let mut state = AppState::new_no_backend();
        load_manifest(&mut state, &file.to_string_lossy());
        assert!(state.flash.manifest.is_some());
        assert!(state.flash.selected_image_id.is_none()); // multiple images, no auto-select
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_drives_empty() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = Some(0);
        refresh_drives(&mut state);
        // On CI there are no optical drives — verify postcondition
        assert!(
            !state.drive.drives.is_empty() || state.drive.selected_drive.is_none(),
            "when no drives found, selected_drive must be None"
        );
    }

    #[test]
    fn finalize_drive_selection_selects_first_when_unselected() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = None;
        finalize_drive_selection(&mut state);
        assert_eq!(state.drive.selected_drive, Some(0));
        let status = &state.runtime.status_message;
        assert!(status.to_lowercase().contains("ready"), "status: {status}");
    }

    #[test]
    fn finalize_drive_selection_clears_selection_when_empty() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = Some(0);
        finalize_drive_selection(&mut state);
        assert_eq!(state.drive.selected_drive, None);
    }

    #[test]
    fn validate_flash_no_drive() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = None;
        validate_flash(&mut state);
        // Should not crash, no flash report set
        assert!(state.flash.flash_report.is_none());
    }

    #[test]
    fn validate_flash_no_manifest() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = None;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_data = Some(vec![0u8; 16]);
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
        assert!(
            state.runtime.log_text.to_lowercase().contains("manifest"),
            "log: {}",
            state.runtime.log_text
        );
    }

    #[test]
    fn validate_flash_no_firmware_data() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.flash.firmware_data = None;
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
    }

    #[test]
    fn validate_flash_no_image_id() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![
                crate::manifest::FirmwareImage {
                    image_id: "main".into(),
                    filename: "fw.bin".into(),
                    target_version: "1.04".into(),
                    size: 1024,
                    sha256: "abcd".into(),
                    signature_present: true,
                },
                crate::manifest::FirmwareImage {
                    image_id: "alt".into(),
                    filename: "fw2.bin".into(),
                    target_version: "1.05".into(),
                    size: 1024,
                    sha256: "ef01".into(),
                    signature_present: true,
                },
            ],
        });
        state.flash.firmware_data = Some(vec![0u8; 1024]);
        state.flash.selected_image_id = None;
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
        assert!(
            state.runtime.log_text.contains("image"),
            "log: {}",
            state.runtime.log_text
        );
    }

    #[test]
    fn validate_flash_success() {
        let temp_dir = std::env::temp_dir().join("sdf_flash_test_validate");
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();

        let mut state = AppState::new_no_backend();
        state.config.tool_path = tool.to_string_lossy().to_string();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd1234".into(),
                signature_present: true,
            }],
        });
        state.flash.firmware_data = Some(vec![0u8; 1024]);
        state.flash.selected_image_id = Some("main".into());
        state.flash.confirmation = "FLASH /dev/sr0".into();
        validate_flash(&mut state);
        // flash_report should be set (even if checksum doesn't match)
        assert!(state.flash.flash_report.is_some());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_empty_path() {
        let mut state = AppState::new_no_backend();
        state.flash.wrong_firmware_path = String::new();
        extract_recovery_token_from_wrong_firmware(&mut state);
        // Should not crash, should not log error
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
        // Should not log anything since path is already set
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
    fn execute_start_write_no_drive() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash
    }

    #[test]
    fn execute_start_write_validation_fails() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = None; // no firmware → validation fails
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash, flash_report should be None
        assert!(state.flash.flash_report.is_none());
    }

    #[test]
    fn execute_start_write_success_path() {
        let (mut state, temp_dir) = state_with_valid_paths("exwrite");
        let data = vec![0u8; 1024];
        let sha = crate::flash::sha256_hex(&data);
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: sha,
                signature_present: true,
            }],
        });
        state.flash.selected_image_id = Some("main".into());
        state.flash.firmware_data = Some(data);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Shared prepare_firmware_op planned the write → begin_operation
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
    fn execute_start_write_no_drive_with_flash_report() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        state.drive.selected_drive = None;
        state.flash.flash_report = Some(crate::flash::FlashReport {
            would_execute: true,
            direction: crate::flash::FlashDirection::Upgrade,
            checks: crate::flash::FlashChecks {
                model_match: true,
                revision_check: true,
                image_checksum: true,
                signature_present: true,
                user_confirmed: true,
            },
            summary: "ok".into(),
            warnings: vec![],
        });
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Returns early at line 180 — no drive selected
        assert!(!state.runtime.busy);
    }

    #[test]
    fn execute_start_write_unconfirmed_does_not_start() {
        let (mut state, temp_dir) = state_with_valid_paths("planfail");
        let data = vec![0u8; 16];
        let sha = crate::flash::sha256_hex(&data);
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 16,
                sha256: sha,
                signature_present: true,
            }],
        });
        state.flash.selected_image_id = Some("main".into());
        state.flash.firmware_data = Some(data);
        state.flash.firmware_path = "fw.bin".into();
        // Wrong confirmation → would_execute false via shared prepare
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
        // Wrong confirmation → would_execute false via shared prepare
        state.flash.confirmation = "WRONG".into();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.runtime.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_no_drive() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Recover;
        state.drive.selected_drive = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash
    }

    #[test]
    fn load_firmware_root_path_no_parent() {
        // "/" has no parent → if let Some(parent) is false, skips candidates
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
        // Read mode with valid paths should return empty (can start)
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
        // No manifest → confirmation required (CLI parity path).
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        // With manifest → ask user to run validation.
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 100,
                sha256: "x".into(),
                signature_present: true,
            }],
        });
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
        // Should return ReasonEnterToken
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
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 100,
                sha256: "x".into(),
                signature_present: true,
            }],
        });
        state.flash.flash_report = Some(crate::flash::FlashReport {
            would_execute: true,
            direction: crate::flash::FlashDirection::Upgrade,
            checks: crate::flash::FlashChecks {
                model_match: true,
                revision_check: true,
                image_checksum: true,
                signature_present: true,
                user_confirmed: true,
            },
            summary: "Flash ready".into(),
            warnings: vec![],
        });
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
    fn start_disabled_reason_write_no_manifest_needs_confirmation() {
        let (mut state, temp_dir) = state_with_valid_paths("nomfconfirm");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.manifest = None;
        state.flash.confirmation.clear();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        assert!(start_disabled_reason(&state).is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_write_mode_conflict() {
        let (mut state, temp_dir) = state_with_valid_paths("reasonconflict");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 8]);
        state.flash.encrypted_write = true;
        state.flash.include_boot_loader = true;
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn validate_flash_no_manifest_confirmed_logs_ready() {
        let (mut state, temp_dir) = state_with_valid_paths("nomfready");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = None;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_data = Some(vec![0u8; 16]);
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
        // Confirmed no-manifest prepare logs warnings and Ready.
        assert!(
            state.runtime.log_text.contains("WARNING") && state.runtime.log_text.contains("Ready"),
            "log: {}",
            state.runtime.log_text
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn validate_flash_no_manifest_unconfirmed_logs_hint() {
        let (mut state, temp_dir) = state_with_valid_paths("nomfhint");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = None;
        state.flash.firmware_path = "fw.bin".into();
        state.flash.firmware_data = Some(vec![0u8; 16]);
        state.flash.confirmation.clear();
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
        assert!(
            state
                .runtime
                .log_text
                .contains("load a manifest before validating"),
            "log: {}",
            state.runtime.log_text
        );
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
        // log_error prefixes ERROR; body includes mode-conflict text.
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
    fn can_start_write_without_manifest_with_confirmation() {
        let (mut state, temp_dir) = state_with_valid_paths("canwritenomf");
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash.firmware_data = Some(vec![0u8; 100]);
        state.flash.firmware_path = "fw.bin".into();
        state.flash.manifest = None;
        state.flash.confirmation =
            crate::command::required_flash_confirmation(&test_drive().device);
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
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
    fn validate_flash_image_not_found() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.flash.firmware_data = Some(vec![0u8; 1024]);
        state.flash.selected_image_id = Some("nonexistent".into());
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_none());
        // ImageNotFound is surfaced via the localized validation-failed path.
        assert!(
            state.runtime.log_text.to_lowercase().contains("validation"),
            "log: {}",
            state.runtime.log_text
        );
    }

    #[test]
    fn validate_flash_model_mismatch() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.drive.drive_mt1959 = true;
        state.flash.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "OTHER".into(),
            model: "WRONG".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.flash.firmware_data = Some(vec![0u8; 1024]);
        state.flash.selected_image_id = Some("main".into());
        state.flash.confirmation = "FLASH /dev/sr0".into();
        validate_flash(&mut state);
        assert!(state.flash.flash_report.is_some());
        assert!(!state.flash.flash_report.as_ref().unwrap().would_execute);
        assert!(state
            .flash
            .flash_report
            .as_ref()
            .unwrap()
            .summary
            .contains("model"));
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
        // Should find .bin files but not .txt
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

    // --- FileDialog trait tests ---

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
        // Dialog returns no folder → early return
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
        // plan_command should succeed and begin_operation should be called
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
        state.flash.manifest = None; // CLI parity: no-manifest write with confirmation
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
    fn finalize_drive_selection_keeps_existing_selection() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        finalize_drive_selection(&mut state);
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
        // Dialog returns nothing → log message but no token
        assert!(state.runtime.log_text.contains("RECOVER"));
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
}
