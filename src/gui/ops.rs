// Business logic operations — flash validation, execute, can_start, etc.
//
// These are pure helpers that operate on AppState. They don't render UI
// or spawn threads — that happens in mod.rs and workers.rs.

use crate::command::{self, Operation, PlanRequest};
use crate::drive::{self, Drive};
use crate::flash;
use crate::i18n::{t, L10nKey};
use crate::manifest;
use crate::orchestration;

use super::state::AppState;
use super::validation::{validate_sdf_path, validate_tool_path};
use super::workers::{spawn_streaming_command, WorkerMsg};
use super::OperationMode;

use std::sync::mpsc::Sender;

pub fn can_start(state: &AppState) -> bool {
    if state.busy || state.probing || state.selected_drive().is_none() || !state.drive_mt1959 {
        return false;
    }
    if validate_tool_path(&state.tool_path, state.backend).is_err() {
        return false;
    }
    if validate_sdf_path(&state.sdf_path).is_err() {
        return false;
    }
    match state.operation_mode {
        OperationMode::Read => true,
        OperationMode::Write => {
            state.firmware_data.is_some()
                && !state.firmware_path.is_empty()
                && !(state.encrypted_write && state.include_boot_loader)
                && state.flash_report.as_ref().is_some_and(|r| r.would_execute)
        }
        OperationMode::Recover => {
            !state.firmware_path.is_empty()
                && state.recovery_token.len() == 16
                && state.confirmation
                    == state
                        .selected_drive()
                        .map(|d| command::required_flash_confirmation(&d.device))
                        .unwrap_or_default()
        }
    }
}

pub fn start_disabled_reason(state: &AppState) -> String {
    let lang = state.resolved_lang;
    if state.busy {
        return t(L10nKey::ReasonBusy, lang).to_string();
    }
    if state.probing {
        return t(L10nKey::ReasonProbing, lang).to_string();
    }
    if state.selected_drive().is_none() {
        return t(L10nKey::ReasonNoDrive, lang).to_string();
    }
    if !state.drive_mt1959 {
        return t(L10nKey::ReasonNotMt1959, lang).to_string();
    }
    if let Err(e) = validate_tool_path(&state.tool_path, state.backend) {
        return format!("Invalid tool path: {e}");
    }
    if let Err(e) = validate_sdf_path(&state.sdf_path) {
        return format!("Invalid sdf.bin: {e}");
    }
    match state.operation_mode {
        OperationMode::Read => String::new(),
        OperationMode::Write => {
            if state.firmware_data.is_none() {
                return t(L10nKey::ReasonNoFirmware, lang).to_string();
            }
            if state.encrypted_write && state.include_boot_loader {
                return t(L10nKey::ReasonConflict, lang).to_string();
            }
            t(L10nKey::ReasonRunValidation, lang).to_string()
        }
        OperationMode::Recover => t(L10nKey::ReasonEnterToken, lang).to_string(),
    }
}

pub fn on_operation_mode_changed(state: &mut AppState, mode: OperationMode) {
    state.flash_report = None;
    state.confirmation.clear();
    match mode {
        OperationMode::Read => {
            state.set_status("Select output folder when you start", 0.0);
        }
        OperationMode::Write => {
            state.set_status("Load firmware and manifest, then validate", 0.0);
        }
        OperationMode::Recover => {
            state.set_status("Recovery needs boot token from wrong firmware", 0.0);
            state.pending_recover_browse = true;
        }
    }
}

pub fn validate_flash(state: &mut AppState) {
    let drive = match state.selected_drive() {
        Some(d) => d,
        None => return,
    };
    let manifest = match &state.manifest {
        Some(m) => m,
        None => {
            state.log("ERROR: load a manifest before validating");
            return;
        }
    };
    let firmware_data = match &state.firmware_data {
        Some(d) => d,
        None => return,
    };

    let image_id = match &state.selected_image_id {
        Some(id) => id.clone(),
        None => {
            state.log("ERROR: select an image before validating");
            return;
        }
    };

    let drive_match: manifest::DriveMatch = drive.into();
    let user_confirmed = state.confirmation == command::required_flash_confirmation(&drive.device);

    match orchestration::validate_flash(
        manifest,
        &drive_match,
        &image_id,
        firmware_data,
        user_confirmed,
    ) {
        Ok(report) => {
            state.log(&report.summary);
            state.flash_report = Some(report);
        }
        Err(e) => {
            state.log(&e);
            state.flash_report = None;
        }
    }
}

pub fn execute_start(state: &mut AppState, worker_tx: &Sender<WorkerMsg>) {
    match state.operation_mode {
        OperationMode::Read => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            let output_dir = folder.to_string_lossy().to_string();
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: String::new(),
                operation: Operation::Read { output_dir },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Reading firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Reading firmware");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
        OperationMode::Write => {
            validate_flash(state);
            if !state.flash_report.as_ref().is_some_and(|r| r.would_execute) {
                return;
            }
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: state.confirmation.clone(),
                operation: Operation::Write {
                    firmware_path: state.firmware_path.clone(),
                    encrypted: state.encrypted_write,
                    include_boot_loader: state.include_boot_loader,
                },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Writing firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Writing firmware");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
        OperationMode::Recover => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: state.confirmation.clone(),
                operation: Operation::Recover {
                    firmware_path: state.firmware_path.clone(),
                    recovery_boot_token: state.recovery_token.clone(),
                },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Recovering drive");
                    spawn_streaming_command(worker_tx, plan.command, "Recovering drive");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
    }
}

fn begin_operation(state: &mut AppState, status: &str) {
    state.busy = true;
    state.progress_indeterminate = true;
    state.progress = 0.0;
    state.set_status(status, 0.0);
}

pub fn refresh_drives(state: &mut AppState) {
    state.drives = drive::enumerate_drives();
    state.last_probed_drive = None;
    state.log(&format!("Found {} drive(s).", state.drives.len()));
    if state.drives.is_empty() {
        state.selected_drive = None;
        state.set_status("No optical drives detected", 0.0);
    } else if state.selected_drive.is_none() {
        state.selected_drive = Some(0);
        state.set_status("Ready", 0.0);
    }
}

pub fn load_firmware(state: &mut AppState, path: &str) {
    state.firmware_path = path.to_string();
    state.flash_report = None;
    match std::fs::read(path) {
        Ok(data) => {
            if data.is_empty() {
                state.log(&format!("ERROR: firmware file is empty: {path}"));
                state.firmware_data = None;
            } else {
                state.firmware_data = Some(data);
            }
        }
        Err(e) => {
            state.log(&format!("ERROR: cannot read firmware file {path}: {e}"));
            state.firmware_data = None;
        }
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        state.firmware_candidates = std::fs::read_dir(parent)
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

    state.firmware_picker_items = state
        .firmware_candidates
        .iter()
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| path.clone());
            (name, path.clone())
        })
        .collect();

    if let Some(data) = &state.firmware_data {
        state.log(&format!(
            "Loaded firmware: {} ({} bytes, sha256 {})",
            path,
            data.len(),
            &flash::sha256_hex(data)[..16]
        ));
    }
}

pub fn load_manifest(state: &mut AppState, path: &str) {
    state.manifest_path = path.to_string();
    match std::fs::read(path) {
        Ok(data) => match manifest::parse_manifest(&data) {
            Ok(m) => {
                state.log(&format!(
                    "Loaded manifest: {} {} ({} image(s))",
                    m.vendor,
                    m.model,
                    m.firmware_images.len()
                ));
                state.selected_image_id = if m.firmware_images.len() == 1 {
                    Some(m.firmware_images[0].image_id.clone())
                } else {
                    None
                };
                state.manifest = Some(m);
                state.flash_report = None;
            }
            Err(e) => state.log(&format!("ERROR: invalid manifest: {e}")),
        },
        Err(e) => state.log(&format!("ERROR: cannot read manifest: {e}")),
    }
}

pub fn prompt_recovery_wrong_firmware(state: &mut AppState) {
    if !state.wrong_firmware_path.is_empty() {
        return;
    }
    state.log("RECOVER: select the wrong firmware file to extract boot token");
    if let Some(file) = rfd::FileDialog::new()
        .set_title("Wrong firmware (for token extraction)")
        .add_filter("Firmware", &["bin"])
        .pick_file()
    {
        state.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(state);
    }
}

pub fn extract_recovery_token_from_wrong_firmware(state: &mut AppState) {
    if state.wrong_firmware_path.is_empty() {
        return;
    }
    match std::fs::read(&state.wrong_firmware_path) {
        Ok(data) => match command::extract_recovery_boot_token(&data) {
            Ok(token) => {
                state.recovery_token = token.clone();
                state.log(&format!("Extracted recovery boot token: {token}"));
            }
            Err(e) => state.log(&format!("ERROR: {e}")),
        },
        Err(e) => state.log(&format!(
            "ERROR: cannot read {}: {e}",
            state.wrong_firmware_path
        )),
    }
}

pub fn browse_firmware_file(state: &mut AppState) {
    let mut dialog = rfd::FileDialog::new().add_filter("Firmware", &["bin"]);
    if !state.firmware_path.is_empty() {
        if let Some(parent) = std::path::Path::new(&state.firmware_path).parent() {
            dialog = dialog.set_directory(parent);
        }
    }
    if let Some(file) = dialog.pick_file() {
        load_firmware(state, &file.to_string_lossy());
    }
}

pub fn drive_label(drive: &Drive) -> String {
    if drive.vendor.is_empty() {
        drive.device.clone()
    } else {
        format!(
            "{} {} {} {} {}",
            drive.device,
            drive.vendor,
            drive.product,
            drive.revision,
            drive_serial_hint(drive)
        )
        .trim()
        .to_string()
    }
}

fn drive_serial_hint(drive: &Drive) -> String {
    let label = format!("{}_{}_{}", drive.vendor, drive.product, drive.revision);
    label
        .split(['_', '-', ' '])
        .skip(2)
        .collect::<Vec<_>>()
        .join(" ")
}
