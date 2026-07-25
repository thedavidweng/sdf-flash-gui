use super::super::state::AppState;
use super::super::OperationMode;
use super::firmware::firmware_picker_label;
use crate::drive::Drive;
use crate::i18n::{t, L10nKey};

pub fn drive_label(drive: &Drive) -> String {
    if drive.vendor.is_empty() {
        drive.device.clone()
    } else {
        [
            drive.device.as_str(),
            drive.vendor.as_str(),
            drive.product.as_str(),
            drive.revision.as_str(),
            drive.serial.as_str(),
        ]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
    }
}

/// Human-readable flash/write mode for the confirmation summary.
pub fn flash_mode_label(state: &AppState) -> String {
    let lang = state.chrome.resolved_lang;
    match state.operation_mode {
        OperationMode::Read => t(L10nKey::TabRead, lang).to_string(),
        OperationMode::Recover => t(L10nKey::FlashModeRecover, lang).to_string(),
        OperationMode::Write => {
            if state.flash.include_boot_loader {
                t(L10nKey::FlashModeBootloader, lang).to_string()
            } else if state.flash.encrypted_write {
                t(L10nKey::FlashModeEncrypted, lang).to_string()
            } else {
                t(L10nKey::FlashModeStandard, lang).to_string()
            }
        }
    }
}

/// First 8 hex chars of the loaded firmware SHA-256 (from the stored
/// identification — never recomputed per frame), or localized N/A.
pub fn firmware_sha_prefix(state: &AppState) -> String {
    state
        .flash
        .firmware_resolved
        .as_ref()
        .map(|resolved| resolved.identification.sha256[..8].to_string())
        .unwrap_or_else(|| t(L10nKey::LabelNotAvailable, state.chrome.resolved_lang).to_string())
}

/// Basename of the selected firmware path, or localized N/A.
pub fn firmware_basename(state: &AppState) -> String {
    if state.flash.firmware_path.is_empty() {
        return t(L10nKey::LabelNotAvailable, state.chrome.resolved_lang).to_string();
    }
    firmware_picker_label(&state.flash.firmware_path)
}
