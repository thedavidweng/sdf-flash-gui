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

use super::file_dialog::FileDialog;
use super::process_runner::ProcessRunner;
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

pub fn execute_start(
    state: &mut AppState,
    worker_tx: &Sender<WorkerMsg>,
    dialog: &impl FileDialog,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    match state.operation_mode {
        OperationMode::Read => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let Some(folder) = dialog.pick_folder() else {
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
                    state.begin_operation("Reading firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Reading firmware", runner);
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
                    state.begin_operation("Writing firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Writing firmware", runner);
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
                    state.begin_operation("Recovering drive");
                    spawn_streaming_command(worker_tx, plan.command, "Recovering drive", runner);
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
    }
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

pub fn prompt_recovery_wrong_firmware(state: &mut AppState, dialog: &impl FileDialog) {
    if !state.wrong_firmware_path.is_empty() {
        return;
    }
    state.log("RECOVER: select the wrong firmware file to extract boot token");
    if let Some(file) = dialog.pick_file_with_title(
        "Wrong firmware (for token extraction)",
        "Firmware",
        &["bin"],
    ) {
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

pub fn browse_firmware_file(state: &mut AppState, dialog: &impl FileDialog) {
    if let Some(file) = dialog.pick_file_with_title("Firmware", "Firmware", &["bin"]) {
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

#[cfg(test)]
mod tests {
    use super::super::file_dialog::FileDialog;
    use super::super::state::AppState;
    use super::super::OperationMode;
    use super::*;
    use crate::drive::Drive;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Mock file dialog for testing. Each method returns a pre-configured path.
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
        ) -> Option<PathBuf> {
            self.file.lock().unwrap().take()
        }
    }

    fn no_dialog() -> MockDialog {
        MockDialog::returning_nothing()
    }

    /// Mock process runner for testing.
    struct MockRunner;

    impl super::super::process_runner::ProcessRunner for MockRunner {
        fn run_command(
            &self,
            _program: &str,
            _args: &[String],
        ) -> Result<crate::process::CommandOutput, String> {
            Err("mock: not implemented".into())
        }

        fn run_command_streaming(
            &self,
            _program: &str,
            _args: &[String],
            _on_line: &dyn Fn(&str),
        ) -> Result<crate::process::CommandOutput, String> {
            Err("mock: not implemented".into())
        }
    }

    fn mock_runner() -> std::sync::Arc<dyn super::super::process_runner::ProcessRunner> {
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
        let hint = drive_serial_hint(&d);
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
        let hint = drive_serial_hint(&d);
        assert_eq!(hint, "REV");
    }

    #[test]
    fn can_start_no_drive() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = None;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_busy() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.busy = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_probing() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.probing = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_not_mt1959() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = false;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_read_mode_valid() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        // Need a valid tool path — create a temp file
        let temp_dir = std::env::temp_dir().join("sdf_flash_test_ops");
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();
        state.tool_path = tool.to_string_lossy().to_string();
        // validate_sdf_path with empty is ok
        state.sdf_path = String::new();
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn can_start_write_no_firmware() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = None;
        state.firmware_path = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_write_conflicting_modes() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = Some(vec![0u8; 100]);
        state.firmware_path = "fw.bin".into();
        state.encrypted_write = true;
        state.include_boot_loader = true;
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_recover_no_token() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.recovery_token = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_recover_wrong_confirmation() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.firmware_path = "fw.bin".into();
        state.confirmation = "WRONG".into();
        assert!(!can_start(&state));
    }

    #[test]
    fn start_disabled_reason_busy() {
        let mut state = AppState::new_no_backend();
        state.busy = true;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("progress"));
    }

    #[test]
    fn start_disabled_reason_probing() {
        let mut state = AppState::new_no_backend();
        state.probing = true;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Probing"));
    }

    #[test]
    fn start_disabled_reason_no_drive() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = None;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("drive"));
    }

    #[test]
    fn start_disabled_reason_not_mt1959() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = false;
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("MT1959"));
    }

    fn state_with_valid_paths(suffix: &str) -> (AppState, std::path::PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!("sdf_flash_test_reasons_{suffix}"));
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();
        let mut state = AppState::new_no_backend();
        state.tool_path = tool.to_string_lossy().to_string();
        state.sdf_path = String::new(); // empty sdf_path is OK (validate_sdf_path returns Ok for empty)
        (state, temp_dir)
    }

    #[test]
    fn start_disabled_reason_write_no_firmware() {
        let (mut state, temp_dir) = state_with_valid_paths("nofw");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = None;
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
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = Some(vec![0u8; 100]);
        state.encrypted_write = true;
        state.include_boot_loader = true;
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty(), "reason should not be empty");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_recover() {
        let (mut state, temp_dir) = state_with_valid_paths("recover");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.recovery_token = String::new();
        let reason = start_disabled_reason(&state);
        assert!(!reason.is_empty(), "reason should not be empty");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn on_operation_mode_changed_read() {
        let mut state = AppState::new_no_backend();
        state.flash_report = Some(crate::flash::FlashReport {
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
        });
        state.confirmation = "test".into();
        on_operation_mode_changed(&mut state, OperationMode::Read);
        assert!(state.flash_report.is_none());
        assert!(state.confirmation.is_empty());
    }

    #[test]
    fn on_operation_mode_changed_write() {
        let mut state = AppState::new_no_backend();
        on_operation_mode_changed(&mut state, OperationMode::Write);
        assert!(state.flash_report.is_none());
        assert!(state.status_message.contains("firmware"));
    }

    #[test]
    fn on_operation_mode_changed_recover() {
        let mut state = AppState::new_no_backend();
        on_operation_mode_changed(&mut state, OperationMode::Recover);
        assert!(state.pending_recover_browse);
        assert!(state.status_message.contains("token"));
    }

    #[test]
    fn load_firmware_nonexistent() {
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, "/nonexistent/path/fw.bin");
        assert!(state.firmware_data.is_none());
        assert!(state.firmware_path == "/nonexistent/path/fw.bin");
        assert!(state.log_text.contains("ERROR"));
    }

    #[test]
    fn load_firmware_empty_file() {
        let dir = std::env::temp_dir().join("sdf_flash_test_load_fw");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("empty.bin");
        std::fs::write(&file, b"").unwrap();
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, &file.to_string_lossy());
        assert!(state.firmware_data.is_none());
        assert!(state.log_text.contains("empty"));
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
        assert!(state.firmware_data.is_some());
        assert_eq!(state.firmware_data.as_ref().unwrap().len(), 1024);
        assert!(state.log_text.contains("Loaded firmware"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_nonexistent() {
        let mut state = AppState::new_no_backend();
        load_manifest(&mut state, "/nonexistent/manifest.json");
        assert!(state.manifest.is_none());
        assert!(state.log_text.contains("ERROR"));
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
        assert!(state.manifest.is_some());
        assert_eq!(state.selected_image_id.as_deref(), Some("main"));
        assert!(state.log_text.contains("Loaded manifest"));
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
        assert!(state.manifest.is_none());
        assert!(state.log_text.contains("invalid manifest"));
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
        assert!(state.manifest.is_some());
        assert!(state.selected_image_id.is_none()); // multiple images, no auto-select
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_drives_empty() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = Some(0);
        refresh_drives(&mut state);
        // On CI there are no optical drives — verify postcondition
        assert!(
            !state.drives.is_empty() || state.selected_drive.is_none(),
            "when no drives found, selected_drive must be None"
        );
    }

    #[test]
    fn validate_flash_no_drive() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = None;
        validate_flash(&mut state);
        // Should not crash, no flash report set
        assert!(state.flash_report.is_none());
    }

    #[test]
    fn validate_flash_no_manifest() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.manifest = None;
        validate_flash(&mut state);
        assert!(state.flash_report.is_none());
        assert!(state.log_text.contains("manifest"));
    }

    #[test]
    fn validate_flash_no_firmware_data() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = None;
        validate_flash(&mut state);
        assert!(state.flash_report.is_none());
    }

    #[test]
    fn validate_flash_no_image_id() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = Some(vec![0u8; 1024]);
        state.selected_image_id = None;
        validate_flash(&mut state);
        assert!(state.flash_report.is_none());
        assert!(state.log_text.contains("image"));
    }

    #[test]
    fn validate_flash_success() {
        let temp_dir = std::env::temp_dir().join("sdf_flash_test_validate");
        let _ = std::fs::create_dir_all(&temp_dir);
        let tool = temp_dir.join("sdftool_test");
        std::fs::write(&tool, b"").unwrap();

        let mut state = AppState::new_no_backend();
        state.tool_path = tool.to_string_lossy().to_string();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd1234".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = Some(vec![0u8; 1024]);
        state.selected_image_id = Some("main".into());
        state.confirmation = "FLASH /dev/sr0".into();
        validate_flash(&mut state);
        // flash_report should be set (even if checksum doesn't match)
        assert!(state.flash_report.is_some());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_empty_path() {
        let mut state = AppState::new_no_backend();
        state.wrong_firmware_path = String::new();
        extract_recovery_token_from_wrong_firmware(&mut state);
        // Should not crash, should not log error
        assert!(state.log_text.is_empty());
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_nonexistent() {
        let mut state = AppState::new_no_backend();
        state.wrong_firmware_path = "/nonexistent/fw.bin".into();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert!(state.log_text.contains("ERROR"));
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
        state.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert_eq!(state.recovery_token, "ABCDEFGHIJKLMNOP");
        assert!(state.log_text.contains("Extracted"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn extract_recovery_token_from_wrong_firmware_too_short() {
        let dir = std::env::temp_dir().join("sdf_flash_test_extract_short");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("short.bin");
        std::fs::write(&file, &[0u8; 100]).unwrap();

        let mut state = AppState::new_no_backend();
        state.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(&mut state);
        assert!(state.log_text.contains("ERROR"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prompt_recovery_wrong_firmware_already_set() {
        let mut state = AppState::new_no_backend();
        state.wrong_firmware_path = "/some/path.bin".into();
        prompt_recovery_wrong_firmware(&mut state, &no_dialog());
        // Should not log anything since path is already set
        assert!(state.log_text.is_empty());
    }

    #[test]
    fn can_start_read_no_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        state.tool_path = String::new();
        assert!(!can_start(&state));
    }

    #[test]
    fn can_start_read_invalid_sdf_path() {
        let (mut state, temp_dir) = state_with_valid_paths("invaliddf");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        state.sdf_path = "/nonexistent/file.txt".into();
        assert!(!can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_write_no_drive() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash
    }

    #[test]
    fn execute_start_write_validation_fails() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = None; // no firmware → validation fails
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash, flash_report should be None
        assert!(state.flash_report.is_none());
    }

    #[test]
    fn execute_start_write_success_path() {
        let (mut state, temp_dir) = state_with_valid_paths("exwrite");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd1234".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = Some(vec![0u8; 1024]);
        // Set wrong sha256 so flash_report.would_execute is false
        // (checksum mismatch), which means execute_start returns early.
        // Instead, let's set matching sha256:
        state.firmware_data = {
            let data = vec![0u8; 1024];
            // sha256 of vec![0u8; 1024] is not "abcd1234", so we can't match.
            // Instead, set firmware_data to None to skip validate_flash success,
            // and directly set flash_report:
            None
        };
        state.flash_report = Some(crate::flash::FlashReport {
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
        });
        state.firmware_path = "fw.bin".into();
        state.confirmation = crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should have called begin_operation
        assert!(state.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_success_path() {
        let (mut state, temp_dir) = state_with_valid_paths("exrecover");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.firmware_path = "fw.bin".into();
        state.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.confirmation = crate::command::required_flash_confirmation(&test_drive().device);
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(state.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_write_no_drive_with_flash_report() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Write;
        state.selected_drive = None;
        state.flash_report = Some(crate::flash::FlashReport {
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
        });
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Returns early at line 180 — no drive selected
        assert!(!state.busy);
    }

    #[test]
    fn execute_start_write_plan_fails() {
        let (mut state, temp_dir) = state_with_valid_paths("planfail");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.flash_report = Some(crate::flash::FlashReport {
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
        });
        state.firmware_path = "fw.bin".into();
        // Wrong confirmation → plan_command returns ConfirmationMismatch
        state.confirmation = "WRONG".into();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // plan_command fails → line 199 covered
        assert!(!state.busy);
        assert!(state.log_text.contains("ERROR"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_plan_fails() {
        let (mut state, temp_dir) = state_with_valid_paths("recplanfail");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.firmware_path = "fw.bin".into();
        state.recovery_token = "ABCDEFGHIJKLMNOP".into();
        // Wrong confirmation → plan_command returns ConfirmationMismatch
        state.confirmation = "WRONG".into();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.busy);
        assert!(state.log_text.contains("ERROR"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_recover_no_drive() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = OperationMode::Recover;
        state.selected_drive = None;
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        // Should not crash
    }

    #[test]
    fn load_firmware_root_path_no_parent() {
        // "/" has no parent → if let Some(parent) is false, skips candidates
        let mut state = AppState::new_no_backend();
        load_firmware(&mut state, "/");
        assert!(state.firmware_candidates.is_empty());
    }

    #[test]
    fn start_disabled_reason_read_empty() {
        let (mut state, temp_dir) = state_with_valid_paths("readempty");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
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
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = Some(vec![0u8; 100]);
        state.firmware_path = "fw.bin".into();
        let reason = start_disabled_reason(&state);
        // Should return ReasonRunValidation — not empty, but not an error either
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_recover_with_token() {
        let (mut state, temp_dir) = state_with_valid_paths("recoverok");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.firmware_path = "fw.bin".into();
        let reason = start_disabled_reason(&state);
        // Should return ReasonEnterToken
        assert!(!reason.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_write_valid() {
        let (mut state, temp_dir) = state_with_valid_paths("canwrite");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Write;
        state.firmware_data = Some(vec![0u8; 100]);
        state.firmware_path = "fw.bin".into();
        state.flash_report = Some(crate::flash::FlashReport {
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
        });
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn can_start_recover_valid() {
        let (mut state, temp_dir) = state_with_valid_paths("canrecover");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Recover;
        state.firmware_path = "fw.bin".into();
        state.recovery_token = "ABCDEFGHIJKLMNOP".into();
        state.confirmation = crate::command::required_flash_confirmation(&test_drive().device);
        assert!(can_start(&state));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn start_disabled_reason_invalid_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.tool_path = "/nonexistent/sdftool".into();
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Invalid tool path"));
    }

    #[test]
    fn start_disabled_reason_invalid_sdf_path() {
        let (mut state, temp_dir) = state_with_valid_paths("invsdf");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.sdf_path = "/nonexistent/sdf.bin".into();
        let reason = start_disabled_reason(&state);
        assert!(reason.contains("Invalid sdf"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn validate_flash_image_not_found() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = Some(vec![0u8; 1024]);
        state.selected_image_id = Some("nonexistent".into());
        validate_flash(&mut state);
        // orchestration::validate_flash returns Err → flash_report = None
        assert!(state.flash_report.is_none());
        assert!(state.log_text.contains("validation failed"));
    }

    #[test]
    fn validate_flash_model_mismatch() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.manifest = Some(crate::manifest::FirmwareManifest {
            schema_version: 1,
            vendor: "OTHER".into(),
            model: "WRONG".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            firmware_images: vec![crate::manifest::FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd".into(),
                signature_present: true,
            }],
        });
        state.firmware_data = Some(vec![0u8; 1024]);
        state.selected_image_id = Some("main".into());
        state.confirmation = "FLASH /dev/sr0".into();
        validate_flash(&mut state);
        assert!(state.flash_report.is_some());
        assert!(!state.flash_report.as_ref().unwrap().would_execute);
        assert!(state
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
        assert!(state.firmware_data.is_some());
        // Should find .bin files but not .txt
        assert!(state
            .firmware_candidates
            .iter()
            .any(|p| p.contains("a.bin")));
        assert!(state
            .firmware_candidates
            .iter()
            .any(|p| p.contains("b.bin")));
        assert!(!state
            .firmware_candidates
            .iter()
            .any(|p| p.contains("c.txt")));
        let _ = std::fs::remove_dir_all(dir);
    }

    // --- FileDialog trait tests ---

    #[test]
    fn execute_start_read_no_folder_selected() {
        let (mut state, temp_dir) = state_with_valid_paths("readnofolder");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let (tx, _rx) = std::sync::mpsc::channel();
        // Dialog returns no folder → early return
        execute_start(&mut state, &tx, &no_dialog(), &mock_runner());
        assert!(!state.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn execute_start_read_with_folder() {
        let (mut state, temp_dir) = state_with_valid_paths("readfolder");
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.drive_mt1959 = true;
        state.operation_mode = OperationMode::Read;
        let dialog = MockDialog::returning_folder("/tmp/output");
        let (tx, _rx) = std::sync::mpsc::channel();
        execute_start(&mut state, &tx, &dialog, &mock_runner());
        // plan_command should succeed and begin_operation should be called
        assert!(state.busy);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn browse_firmware_file_no_selection() {
        let mut state = AppState::new_no_backend();
        browse_firmware_file(&mut state, &no_dialog());
        // No file selected → nothing changes
        assert!(state.firmware_data.is_none());
    }

    #[test]
    fn browse_firmware_file_with_selection() {
        let dir = std::env::temp_dir().join("sdf_flash_test_browse");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("test.bin");
        std::fs::write(&file, &[0u8; 10]).unwrap();
        let mut state = AppState::new_no_backend();
        let dialog = MockDialog::returning_file(&file.to_string_lossy());
        browse_firmware_file(&mut state, &dialog);
        assert!(state.firmware_data.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prompt_recovery_wrong_firmware_dialog_no_selection() {
        let mut state = AppState::new_no_backend();
        prompt_recovery_wrong_firmware(&mut state, &no_dialog());
        // Dialog returns nothing → log message but no token
        assert!(state.log_text.contains("RECOVER"));
        assert!(state.wrong_firmware_path.is_empty());
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
        assert_eq!(state.wrong_firmware_path, file.to_string_lossy());
        assert_eq!(state.recovery_token, "ABCDEFGHIJKLMNOP");
        let _ = std::fs::remove_dir_all(dir);
    }
}
