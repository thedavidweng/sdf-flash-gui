//! Start enablement and execute_start planning/spawn.
use crate::i18n::{log_error, t, t_with_args, L10nKey, Language};
use crate::orchestration;
use crate::process;
use crate::process::ProcessRunner;

use crate::gui::file_dialog::FileDialog;
use crate::gui::state::AppState;
use crate::gui::validation::validate_tool_path;
use crate::gui::workers::{spawn_streaming_command, WorkerMsg};
use crate::gui::OperationMode;

use std::sync::mpsc::Sender;

pub(crate) fn cross_flash_confirmation_required(state: &AppState) -> bool {
    let Some(drive) = state.selected_drive() else {
        return false;
    };
    let drive_ff = crate::platform::classify_drive(&drive.product);
    let fw_ff = state.flash.firmware_form_factor;
    drive_ff != crate::platform::DriveFormFactor::Unknown
        && fw_ff != crate::platform::DriveFormFactor::Unknown
        && drive_ff != fw_ff
}

/// Whether Start is enabled. Single source of rules: [`start_gate::evaluate`].
pub fn can_start(state: &AppState) -> bool {
    start_disabled_reason(state).is_empty()
}

fn start_gate_input(state: &AppState) -> crate::gui::start_gate::StartGateInput<'_> {
    use crate::gui::start_gate::{StartGateInput, StartMode};
    let mode = match state.operation_mode {
        OperationMode::Read => StartMode::Read,
        OperationMode::Write => StartMode::Write,
        OperationMode::Recover => StartMode::Recover,
    };
    let device = state
        .selected_drive()
        .map(|d| d.device.as_str())
        .unwrap_or("");
    StartGateInput {
        busy: state.runtime.busy,
        probing: state.runtime.probing,
        has_drive: state.selected_drive().is_some(),
        drive_mt1959: state.drive.drive_mt1959,
        drive_mt1939: state.drive.drive_mt1939,
        tool_path: &state.config.tool_path,
        backend: state.config.backend,
        sdf_path: &state.config.sdf_path,
        lang: state.chrome.resolved_lang,
        mode,
        firmware_path: &state.flash.firmware_path,
        has_firmware_data: state.flash.firmware_data.is_some(),
        encrypted_write: state.flash.encrypted_write,
        include_boot_loader: state.flash.include_boot_loader,
        cross_flash_required: cross_flash_confirmation_required(state),
        cross_flash_confirmed: state.flash.cross_flash_confirmed,
        confirmation: &state.flash.confirmation,
        device,
        recovery_token: &state.flash.recovery_token,
    }
}

/// Localized reason Start is disabled, or empty if start is allowed.
pub fn start_disabled_reason(state: &AppState) -> String {
    use crate::gui::start_gate::{evaluate, StartBlock};
    let lang = state.chrome.resolved_lang;
    match evaluate(&start_gate_input(state)) {
        None => String::new(),
        Some(StartBlock::Busy) => t(L10nKey::ReasonBusy, lang).to_string(),
        Some(StartBlock::Probing) => t(L10nKey::ReasonProbing, lang).to_string(),
        Some(StartBlock::NoDrive) => t(L10nKey::ReasonNoDrive, lang).to_string(),
        Some(StartBlock::NotMt1959 { is_mt1939: true }) => {
            t(L10nKey::ReasonMt1939NotCompatible, lang).to_string()
        }
        Some(StartBlock::NotMt1959 { is_mt1939: false }) => {
            t(L10nKey::ReasonNotMt1959, lang).to_string()
        }
        Some(StartBlock::InvalidToolPath(ref e)) => {
            t_with_args(L10nKey::ReasonInvalidToolPath, lang, &[("error", e)])
        }
        Some(StartBlock::InvalidSdfPath(ref e)) => {
            t_with_args(L10nKey::ReasonInvalidSdfPath, lang, &[("error", e)])
        }
        Some(StartBlock::NoFirmware) => t(L10nKey::ReasonNoFirmware, lang).to_string(),
        Some(StartBlock::WriteModeConflict) => t(L10nKey::ReasonConflict, lang).to_string(),
        Some(StartBlock::CrossFlashNotConfirmed) => {
            t(L10nKey::ReasonCrossFlashNotConfirmed, lang).to_string()
        }
        Some(StartBlock::NeedConfirmation) => t(L10nKey::ReasonEnterToken, lang).to_string(),
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
            let recover = matches!(state.operation_mode, OperationMode::Recover);
            if !recover && state.flash.firmware_data.is_none() {
                return;
            }
            let confirm = orchestration::FlashConfirm::Typed(state.flash.confirmation.clone());

            let prepared =
                match orchestration::prepare_firmware_op(orchestration::FirmwareOpRequest {
                    backend: state.config.backend,
                    tool_path: &state.config.tool_path,
                    sdf_path: &state.config.sdf_path,
                    device: &drive.device,
                    drive_is_mt1959: state.drive.drive_mt1959,
                    firmware_path: &state.flash.firmware_path,
                    encrypted: state.flash.encrypted_write,
                    include_boot_loader: state.flash.include_boot_loader,
                    recover,
                    wrong_firmware: None,
                    recovery_token: recover.then_some(state.flash.recovery_token.as_str()),
                    confirm,
                }) {
                    Ok(p) => p,
                    Err(e) => {
                        state.log(&log_error(lang, &e));
                        return;
                    }
                };

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
