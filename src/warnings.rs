//! Flash-safety warnings decided from drive state and resolved firmware.
//! The GUI renders the returned list; no decisions live in views.

use crate::drive::Drive;
use crate::firmware_db::{self, FlashDirection, ResolvedFirmware};
use crate::platform::{self, DriveFormFactor};

/// A pending-flash advisory decided by [`flash_warnings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashWarning {
    /// Firmware form factor differs from the drive's — requires the explicit
    /// cross-flash confirmation checkbox.
    CrossFlash {
        firmware: DriveFormFactor,
        drive: DriveFormFactor,
    },
    TwoStepFlash,
    Downgrade {
        current: String,
        target: String,
    },
    ModelMismatch {
        firmware: String,
        drive: String,
    },
}

/// True when drive and firmware form factors are both known and differ.
pub fn cross_flash_mismatch(drive_ff: DriveFormFactor, firmware_ff: DriveFormFactor) -> bool {
    drive_ff != DriveFormFactor::Unknown
        && firmware_ff != DriveFormFactor::Unknown
        && drive_ff != firmware_ff
}

/// Decide the safety warnings for flashing the loaded firmware to `drive`.
pub fn flash_warnings(
    drive: &Drive,
    firmware_form_factor: DriveFormFactor,
    resolved: Option<&ResolvedFirmware>,
) -> Vec<FlashWarning> {
    let mut warnings = Vec::new();

    let drive_ff = platform::classify_drive(&drive.product);
    if cross_flash_mismatch(drive_ff, firmware_form_factor) {
        warnings.push(FlashWarning::CrossFlash {
            firmware: firmware_form_factor,
            drive: drive_ff,
        });
    }

    if platform::needs_two_step_flash(&drive.product) {
        warnings.push(FlashWarning::TwoStepFlash);
    }

    if let Some(resolved) = resolved {
        if let Some(known) = resolved.identification.known {
            if firmware_db::compare_versions(&drive.revision, known.version)
                == FlashDirection::Downgrade
            {
                warnings.push(FlashWarning::Downgrade {
                    current: drive.revision.clone(),
                    target: known.version.to_string(),
                });
            }
        }
        if let Some(firmware_model) = &resolved.model {
            if !drive.product.contains(firmware_model.as_str())
                && !firmware_model.contains(&drive.product)
            {
                warnings.push(FlashWarning::ModelMismatch {
                    firmware: firmware_model.clone(),
                    drive: drive.product.clone(),
                });
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firmware_db::{FirmwareBinaryInfo, FirmwareIdentification, KnownFirmware};

    fn test_drive(product: &str, revision: &str) -> Drive {
        Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: product.into(),
            revision: revision.into(),
            ..Default::default()
        }
    }

    fn resolved(known: Option<&'static KnownFirmware>, model: Option<&str>) -> ResolvedFirmware {
        ResolvedFirmware {
            identification: FirmwareIdentification {
                sha256: String::new(),
                known,
                binary_info: FirmwareBinaryInfo {
                    pcb_type: None,
                    model: None,
                    form_factor: DriveFormFactor::Unknown,
                },
            },
            sdf_info: None,
            form_factor: DriveFormFactor::Unknown,
            model: model.map(str::to_string),
            encrypted: None,
        }
    }

    fn known_entry(model: &str) -> &'static KnownFirmware {
        firmware_db::KNOWN_FIRMWARES
            .iter()
            .find(|k| k.model == model)
            .expect("known firmware entry")
    }

    #[test]
    fn cross_flash_mismatch_requires_both_known() {
        assert!(cross_flash_mismatch(
            DriveFormFactor::Desktop,
            DriveFormFactor::Slim
        ));
        assert!(!cross_flash_mismatch(
            DriveFormFactor::Desktop,
            DriveFormFactor::Desktop
        ));
        assert!(!cross_flash_mismatch(
            DriveFormFactor::Unknown,
            DriveFormFactor::Slim
        ));
        assert!(!cross_flash_mismatch(
            DriveFormFactor::Desktop,
            DriveFormFactor::Unknown
        ));
    }

    #[test]
    fn warns_on_cross_flash() {
        let drive = test_drive("BW-16D1HT", "3.10");
        let warnings = flash_warnings(&drive, DriveFormFactor::Slim, None);
        assert_eq!(
            warnings,
            vec![FlashWarning::CrossFlash {
                firmware: DriveFormFactor::Slim,
                drive: DriveFormFactor::Desktop,
            }]
        );
    }

    #[test]
    fn warns_on_two_step_flash_model() {
        let drive = test_drive("BP50NB40", "1.00");
        let warnings = flash_warnings(&drive, DriveFormFactor::Slim, None);
        assert_eq!(warnings, vec![FlashWarning::TwoStepFlash]);
    }

    #[test]
    fn warns_on_downgrade_to_known_firmware() {
        let known = known_entry("BW-16D1HT");
        let drive = test_drive("BW-16D1HT", "9.99");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Desktop,
            Some(&resolved(Some(known), Some("BW-16D1HT"))),
        );
        assert_eq!(
            warnings,
            vec![FlashWarning::Downgrade {
                current: "9.99".into(),
                target: known.version.into(),
            }]
        );
    }

    #[test]
    fn no_downgrade_warning_on_upgrade() {
        let known = known_entry("BW-16D1HT");
        let drive = test_drive("BW-16D1HT", "0.01");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Desktop,
            Some(&resolved(Some(known), Some("BW-16D1HT"))),
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_on_model_mismatch_in_neither_direction() {
        let drive = test_drive("BW-16D1HT", "3.10");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Unknown,
            Some(&resolved(None, Some("BU40N"))),
        );
        assert_eq!(
            warnings,
            vec![FlashWarning::ModelMismatch {
                firmware: "BU40N".into(),
                drive: "BW-16D1HT".into(),
            }]
        );
    }

    #[test]
    fn no_model_mismatch_when_either_contains_other() {
        let drive = test_drive("BD-RE BU40N", "1.03");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Unknown,
            Some(&resolved(None, Some("BU40N"))),
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warnings_without_resolved_firmware() {
        let drive = test_drive("BW-16D1HT", "3.10");
        assert!(flash_warnings(&drive, DriveFormFactor::Unknown, None).is_empty());
    }

    #[test]
    fn no_model_mismatch_when_model_unresolved() {
        let drive = test_drive("BW-16D1HT", "3.10");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Unknown,
            Some(&resolved(None, None)),
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn stacks_independent_warnings() {
        let drive = test_drive("BP50NB40", "1.00");
        let warnings = flash_warnings(
            &drive,
            DriveFormFactor::Desktop,
            Some(&resolved(None, Some("BW-16D1HT"))),
        );
        assert_eq!(
            warnings,
            vec![
                FlashWarning::CrossFlash {
                    firmware: DriveFormFactor::Desktop,
                    drive: DriveFormFactor::Slim,
                },
                FlashWarning::TwoStepFlash,
                FlashWarning::ModelMismatch {
                    firmware: "BW-16D1HT".into(),
                    drive: "BP50NB40".into(),
                },
            ]
        );
    }
}
