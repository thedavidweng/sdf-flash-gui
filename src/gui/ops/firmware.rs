//! Firmware load and recovery-token extraction.
use crate::firmware_db;
use crate::gui::file_dialog::FileDialog;
use crate::gui::state::AppState;
use crate::i18n::{log_error, t, t_with_args, L10nKey};
use crate::orchestration;

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
    state.flash.cross_flash_confirmed = false;
    state.flash.firmware_resolved = None;
    state.flash.firmware_form_factor = crate::platform::DriveFormFactor::Unknown;
    state.flash.firmware_file_encrypted = None;
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
                let resolved = firmware_db::identify(&data);
                state.flash.firmware_form_factor = resolved.form_factor;
                state.flash.firmware_file_encrypted = resolved.encrypted;
                state.flash.firmware_resolved = Some(resolved);
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

    // Recompute encrypted_write: drive_encrypted OR firmware_file_encrypted.
    state.recompute_encrypted_write();

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
                ("hash", &firmware_db::sha256_hex(data)[..16]),
            ],
        ));
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
