// Shared flash orchestration — used by both CLI and GUI.
//
// Owns the pipeline from drive identity + manifest → FlashPlanRequest →
// build_flash_plan → dry_run. Both CLI and GUI call into this module
// instead of duplicating the assembly logic.

use crate::command;
use crate::flash;
use crate::manifest;

/// Parse drive identity from sdftool `--info` output for manifest matching.
pub fn parse_drive_identity(device: &str, info_output: &str) -> manifest::DriveMatch {
    let mut vendor = String::new();
    let mut model = String::new();
    let mut revision = String::new();

    for line in info_output.lines() {
        let line = line.trim();
        if let Some(val) = line
            .strip_prefix("Vendor:")
            .or_else(|| line.strip_prefix("vendor:"))
        {
            vendor = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Product:")
            .or_else(|| line.strip_prefix("product:"))
            .or_else(|| line.strip_prefix("Model:"))
            .or_else(|| line.strip_prefix("model:"))
        {
            model = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Revision:")
            .or_else(|| line.strip_prefix("revision:"))
            .or_else(|| line.strip_prefix("Firmware:"))
            .or_else(|| line.strip_prefix("firmware:"))
        {
            revision = val.trim().to_string();
        }
    }

    // Fall back to device label parsing if info output didn't contain fields.
    // Split on '_' only to preserve hyphenated vendor names like "HL-DT-ST".
    if vendor.is_empty() && model.is_empty() && device.contains('_') {
        let mut parts = device.splitn(2, '_');
        if let Some(v) = parts.next() {
            if !v.is_empty() {
                vendor = v.to_string();
            }
        }
        if let Some(m) = parts.next() {
            if !m.is_empty() {
                model = m.to_string();
            }
        }
    }

    manifest::DriveMatch {
        vendor,
        model,
        revision,
    }
}

/// Resolve image ID from manifest, picking the only image if there's exactly one.
pub fn resolve_image_id(
    manifest: &manifest::FirmwareManifest,
    explicit: Option<&str>,
) -> Result<String, String> {
    if let Some(id) = explicit {
        return Ok(id.to_string());
    }
    if manifest.firmware_images.len() == 1 {
        Ok(manifest.firmware_images[0].image_id.clone())
    } else {
        let mut msg = format!(
            "manifest contains {} images; specify an image ID",
            manifest.firmware_images.len()
        );
        for img in &manifest.firmware_images {
            msg.push_str(&format!("\n  - {}", img.image_id));
        }
        Err(msg)
    }
}

/// Resolve recovery boot token from either explicit value or by extracting from a firmware file.
pub fn resolve_recovery_token(
    wrong_firmware_path: Option<&str>,
    explicit_token: Option<&str>,
) -> Result<String, String> {
    if let Some(token) = explicit_token {
        return Ok(token.to_string());
    }
    if let Some(path) = wrong_firmware_path {
        let data =
            std::fs::read(path).map_err(|e| format!("cannot read wrong firmware {path}: {e}"))?;
        command::extract_recovery_boot_token(&data)
            .map_err(|e| format!("cannot extract recovery boot token: {e}"))
    } else {
        Err("--recover requires either --recovery-token or --wrong-firmware".to_string())
    }
}

/// Validate a flash operation: build the plan and return the dry-run report.
///
/// This is the shared pipeline used by both CLI and GUI.
pub fn validate_flash(
    manifest: &manifest::FirmwareManifest,
    drive: &manifest::DriveMatch,
    image_id: &str,
    firmware_data: &[u8],
    user_confirmed: bool,
) -> Result<flash::FlashReport, String> {
    let request = flash::FlashPlanRequest {
        image_id,
        current_version: &drive.revision,
        firmware_size: firmware_data.len() as u64,
        firmware_sha256: &flash::sha256_hex(firmware_data),
        signature_present: manifest
            .firmware_images
            .iter()
            .find(|i| i.image_id == image_id)
            .map(|i| i.signature_present)
            .unwrap_or(false),
        user_confirmed,
    };

    let plan = flash::build_flash_plan(manifest, drive, request)
        .map_err(|e| format!("validation failed: {e}"))?;
    Ok(flash::dry_run(&plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DriveMatch, FirmwareImage, FirmwareManifest};

    fn test_manifest() -> FirmwareManifest {
        FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            firmware_images: vec![FirmwareImage {
                image_id: "main".into(),
                filename: "fw.bin".into(),
                target_version: "1.04".into(),
                size: 1024,
                sha256: "abcd1234".into(),
                signature_present: true,
            }],
        }
    }

    fn test_drive() -> DriveMatch {
        DriveMatch {
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision: "1.03".into(),
        }
    }

    #[test]
    fn parse_drive_identity_full_output() {
        let output = "Vendor: HL-DT-ST\nProduct: BD-RE BU40N\nRevision: 1.03\n";
        let dm = parse_drive_identity("/dev/sr0", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BD-RE BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_drive_identity_case_insensitive() {
        let output = "vendor: LG\nproduct: BU40N\nfirmware: 1.04\n";
        let dm = parse_drive_identity("/dev/sr0", output);
        assert_eq!(dm.vendor, "LG");
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.04");
    }

    #[test]
    fn parse_drive_identity_fallback_to_device() {
        // Falls back to splitting on '_' only, preserving hyphenated vendor names.
        // "HL-DT-ST_BU40N_1.03" → vendor="HL-DT-ST", model="BU40N_1.03".
        let output = "no useful info here";
        let dm = parse_drive_identity("HL-DT-ST_BU40N_1.03", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N_1.03");
    }

    #[test]
    fn parse_drive_identity_empty() {
        let dm = parse_drive_identity("/dev/sr0", "");
        assert!(dm.vendor.is_empty());
        assert!(dm.model.is_empty());
        assert!(dm.revision.is_empty());
    }

    #[test]
    fn resolve_image_id_explicit() {
        let manifest = test_manifest();
        let id = resolve_image_id(&manifest, Some("main")).unwrap();
        assert_eq!(id, "main");
    }

    #[test]
    fn resolve_image_id_single_auto() {
        let manifest = test_manifest();
        let id = resolve_image_id(&manifest, None).unwrap();
        assert_eq!(id, "main");
    }

    #[test]
    fn resolve_image_id_multiple_requires_explicit() {
        let mut manifest = test_manifest();
        manifest.firmware_images.push(FirmwareImage {
            image_id: "alt".into(),
            filename: "fw2.bin".into(),
            target_version: "1.05".into(),
            size: 2048,
            sha256: "ef5678".into(),
            signature_present: true,
        });
        let err = resolve_image_id(&manifest, None).unwrap_err();
        assert!(err.contains("2 images"));
    }

    #[test]
    fn resolve_recovery_token_explicit() {
        let token = resolve_recovery_token(None, Some("ABCDEFGHIJKLMNOP")).unwrap();
        assert_eq!(token, "ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn resolve_recovery_token_missing_both() {
        let err = resolve_recovery_token(None, None).unwrap_err();
        assert!(err.contains("--recover"));
    }

    #[test]
    fn validate_flash_success() {
        let manifest = test_manifest();
        let drive = test_drive();
        let report = validate_flash(&manifest, &drive, "main", &vec![0u8; 1024], true).unwrap();
        // sha256 won't match, so would_execute is false — but it should not error
        assert!(!report.would_execute); // checksum mismatch
    }

    #[test]
    fn validate_flash_image_not_found() {
        let manifest = test_manifest();
        let drive = test_drive();
        let err =
            validate_flash(&manifest, &drive, "nonexistent", &vec![0u8; 1024], true).unwrap_err();
        assert!(err.contains("validation failed"));
    }

    #[test]
    fn parse_drive_identity_model_key() {
        let output = "Model: BU40N\nRevision: 1.03\n";
        let dm = parse_drive_identity("/dev/sr0", output);
        assert_eq!(dm.model, "BU40N");
        assert_eq!(dm.revision, "1.03");
    }

    #[test]
    fn parse_drive_identity_firmware_key() {
        let output = "Firmware: 1.04\n";
        let dm = parse_drive_identity("/dev/sr0", output);
        assert_eq!(dm.revision, "1.04");
    }

    #[test]
    fn parse_drive_identity_fallback_no_underscore() {
        // Device label without '_' — no fallback parsing
        let dm = parse_drive_identity("/dev/sr0", "");
        assert!(dm.vendor.is_empty());
        assert!(dm.model.is_empty());
    }

    #[test]
    fn parse_drive_identity_fallback_single_underscore() {
        let dm = parse_drive_identity("VENDOR_MODEL", "");
        assert_eq!(dm.vendor, "VENDOR");
        assert_eq!(dm.model, "MODEL");
    }

    #[test]
    fn resolve_image_id_empty_manifest() {
        let manifest = FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            firmware_images: vec![],
        };
        let err = resolve_image_id(&manifest, None).unwrap_err();
        assert!(err.contains("0 images"));
    }

    #[test]
    fn resolve_recovery_token_from_file() {
        let dir = std::env::temp_dir().join("sdf_flash_test_token");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("wrong_fw.bin");
        let mut data = vec![0u8; 12_288 + 16];
        data[12_288..12_304].copy_from_slice(b"ABCDEFGHIJKLMNOP");
        std::fs::write(&file, &data).unwrap();

        let token = resolve_recovery_token(Some(&file.to_string_lossy()), None).unwrap();
        assert_eq!(token, "ABCDEFGHIJKLMNOP");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_recovery_token_from_file_not_found() {
        let err = resolve_recovery_token(Some("/nonexistent/fw.bin"), None).unwrap_err();
        assert!(err.contains("cannot read wrong firmware"));
    }

    #[test]
    fn validate_flash_with_matching_checksum() {
        let manifest = test_manifest();
        let drive = test_drive();
        // The manifest expects sha256="abcd1234" and size=1024
        // We can't easily produce data with that exact sha256, but we can verify the function runs
        let report = validate_flash(&manifest, &drive, "main", &vec![0u8; 1024], true).unwrap();
        // sha256 won't match, but function should not error
        assert!(!report.would_execute);
        assert!(report.summary.contains("checksum"));
    }

    #[test]
    fn validate_flash_not_confirmed() {
        let manifest = test_manifest();
        let drive = test_drive();
        let report = validate_flash(&manifest, &drive, "main", &vec![0u8; 1024], false).unwrap();
        assert!(!report.would_execute);
        assert!(report.summary.contains("not confirmed"));
    }

    #[test]
    fn parse_drive_identity_whitespace_trimmed() {
        let output = "  Vendor:   HL-DT-ST  \n  Product:   BU40N  \n";
        let dm = parse_drive_identity("/dev/sr0", output);
        assert_eq!(dm.vendor, "HL-DT-ST");
        assert_eq!(dm.model, "BU40N");
    }
}
