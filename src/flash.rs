use crate::i18n::{t, t_with_args, L10nKey, Language};
use crate::manifest::{glob_match, DriveMatch, FirmwareManifest};
use crate::sdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashDirection {
    Upgrade,
    Downgrade,
    Same,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashPlan {
    pub drive: DriveMatch,
    pub manifest: FirmwareManifest,
    pub current_version: String,
    pub target_version: String,
    pub model_match: bool,
    pub revision_check: bool,
    pub image_checksum: bool,
    pub signature_present: bool,
    pub user_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashReport {
    pub would_execute: bool,
    pub direction: FlashDirection,
    pub checks: FlashChecks,
    pub summary: String,
    /// Advisory warnings — never block the flash, but should be shown to the user.
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashChecks {
    pub model_match: bool,
    pub revision_check: bool,
    pub image_checksum: bool,
    pub signature_present: bool,
    pub user_confirmed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FlashPlanRequest<'a> {
    pub image_id: &'a str,
    pub current_version: &'a str,
    pub firmware_size: u64,
    pub firmware_sha256: &'a str,
    pub signature_present: bool,
    pub user_confirmed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FlashError {
    #[error("firmware image not found: {0}")]
    ImageNotFound(String),
}

pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    let mut hex = String::with_capacity(64);
    for b in hash {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Metadata extracted from a firmware binary's SDF0 header (if parseable).
pub struct FirmwareSdfInfo {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
}

/// Try to parse the firmware binary as an SDF0 container and extract metadata.
/// Returns `None` if the binary is not a valid SDF0 container (e.g. encrypted
/// raw blobs), which is a common and expected case.
pub fn check_firmware_sdf(firmware_data: &[u8]) -> Option<FirmwareSdfInfo> {
    let mut cursor = std::io::Cursor::new(firmware_data);
    let container = sdf::parse_sdf0(&mut cursor).ok()?;
    Some(FirmwareSdfInfo {
        vendor: container.metadata.vendor,
        model: container.metadata.model,
        firmware_version: container.metadata.firmware_version,
    })
}

fn push_sdf_metadata_warnings(
    warnings: &mut Vec<String>,
    manifest: &FirmwareManifest,
    sdf_info: &FirmwareSdfInfo,
    lang: Language,
) {
    if let Some(fw_vendor) = sdf_info
        .vendor
        .as_ref()
        .filter(|v| !glob_match(&manifest.vendor, v))
    {
        warnings.push(t_with_args(
            L10nKey::WarnFwVendorMismatch,
            lang,
            &[
                ("fw_vendor", fw_vendor),
                ("manifest_vendor", &manifest.vendor),
            ],
        ));
    }
    if let Some(fw_model) = sdf_info
        .model
        .as_ref()
        .filter(|m| !glob_match(&manifest.model, m))
    {
        warnings.push(t_with_args(
            L10nKey::WarnFwModelMismatch,
            lang,
            &[("fw_model", fw_model), ("manifest_model", &manifest.model)],
        ));
    }
}

/// Check for advisory warnings. Returns a list of human-readable warning
/// strings. An empty list means no warnings.
///
/// These checks are intentionally softer than the five hard gates — they
/// surface potential issues and let the user decide.
pub fn check_warnings(
    manifest: &FirmwareManifest,
    drive: &DriveMatch,
    firmware_data: &[u8],
    lang: Language,
) -> Vec<String> {
    let mut warnings = Vec::new();

    // 1. Broad wildcards in manifest
    if manifest.vendor == "*" {
        warnings.push(t(L10nKey::WarnManifestAnyVendor, lang).to_string());
    }
    if manifest.model == "*" {
        warnings.push(t(L10nKey::WarnManifestAnyModel, lang).to_string());
    }
    if manifest.revision_match == "*" {
        warnings.push(t(L10nKey::WarnManifestAnyRevision, lang).to_string());
    }

    // 2. Category mismatch (if both manifest and drive specify category)
    if let Some(manifest_cat) = &manifest.category {
        // Drive category comes from the model/product string heuristics.
        // We check common patterns in the drive's model string.
        let drive_model_lower = drive.model.to_lowercase();
        let drive_category = if drive_model_lower.contains("slim")
            || drive_model_lower.contains("external")
            || drive_model_lower.starts_with("bp")
            || drive_model_lower.starts_with("bu")
            || drive_model_lower.starts_with("wp")
        {
            Some("slim")
        } else if drive_model_lower.contains("internal")
            || drive_model_lower.contains("desktop")
            || drive_model_lower.starts_with("wh")
            || drive_model_lower.starts_with("bh")
        {
            Some("internal")
        } else {
            None
        };

        let manifest_cat_lower = manifest_cat.to_lowercase();
        if let Some(dc) = drive_category {
            if (dc == "slim" && manifest_cat_lower == "internal")
                || (dc == "internal" && manifest_cat_lower == "slim")
            {
                warnings.push(t_with_args(
                    L10nKey::WarnCategoryMismatch,
                    lang,
                    &[("manifest_cat", manifest_cat), ("drive_cat", dc)],
                ));
            }
        }
    }

    // 3. Firmware binary SDF0 metadata vs target drive
    if let Some(sdf_info) = check_firmware_sdf(firmware_data) {
        push_sdf_metadata_warnings(&mut warnings, manifest, &sdf_info, lang);
    }

    warnings
}

pub fn build_flash_plan(
    manifest: &FirmwareManifest,
    drive: &DriveMatch,
    request: FlashPlanRequest<'_>,
) -> Result<FlashPlan, FlashError> {
    let image = manifest
        .firmware_images
        .iter()
        .find(|img| img.image_id == request.image_id)
        .ok_or_else(|| FlashError::ImageNotFound(request.image_id.to_string()))?;

    let vendor_match = glob_match(&manifest.vendor, &drive.vendor);
    let model_match = glob_match(&manifest.model, &drive.model);
    let revision_match = glob_match(&manifest.revision_match, &drive.revision);
    let model_match_full = vendor_match && model_match && revision_match;

    let image_checksum =
        image.size == request.firmware_size && image.sha256 == request.firmware_sha256;

    Ok(FlashPlan {
        drive: drive.clone(),
        manifest: manifest.clone(),
        current_version: request.current_version.to_string(),
        target_version: image.target_version.clone(),
        model_match: model_match_full,
        revision_check: revision_match,
        image_checksum,
        signature_present: request.signature_present,
        user_confirmed: request.user_confirmed,
    })
}

pub fn dry_run(plan: &FlashPlan, firmware_data: &[u8], lang: Language) -> FlashReport {
    let direction = compare_versions(&plan.current_version, &plan.target_version);

    let checks = FlashChecks {
        model_match: plan.model_match,
        revision_check: plan.revision_check,
        image_checksum: plan.image_checksum,
        signature_present: plan.signature_present,
        user_confirmed: plan.user_confirmed,
    };

    let would_execute = checks.model_match
        && checks.revision_check
        && checks.image_checksum
        && checks.signature_present
        && checks.user_confirmed;

    let direction_str = match direction {
        FlashDirection::Upgrade => t(L10nKey::DirUpgrade, lang),
        FlashDirection::Downgrade => t(L10nKey::DirDowngrade, lang),
        FlashDirection::Same => t(L10nKey::DirSameVersion, lang),
    };

    let summary = if would_execute {
        t_with_args(
            L10nKey::FlashReadySummary,
            lang,
            &[
                ("vendor", &plan.drive.vendor),
                ("model", &plan.drive.model),
                ("current", &plan.current_version),
                ("target", &plan.target_version),
                ("direction", direction_str),
            ],
        )
    } else {
        let mut failures = Vec::new();
        if !checks.model_match {
            failures.push(t(L10nKey::FailModelMismatch, lang));
        }
        if !checks.revision_check {
            failures.push(t(L10nKey::FailRevisionMismatch, lang));
        }
        if !checks.image_checksum {
            failures.push(t(L10nKey::FailChecksumFailed, lang));
        }
        if !checks.signature_present {
            failures.push(t(L10nKey::FailNoSignature, lang));
        }
        if !checks.user_confirmed {
            failures.push(t(L10nKey::FailNotConfirmed, lang));
        }
        t_with_args(
            L10nKey::FlashBlockedSummary,
            lang,
            &[("failures", &failures.join(", "))],
        )
    };

    let warnings = check_warnings(&plan.manifest, &plan.drive, firmware_data, lang);

    FlashReport {
        would_execute,
        direction,
        checks,
        summary,
        warnings,
    }
}

fn compare_versions(current: &str, target: &str) -> FlashDirection {
    if current == target {
        return FlashDirection::Same;
    }
    let c_parts: Option<Vec<u32>> = current.split(['.', '-']).map(|p| p.parse().ok()).collect();
    let t_parts: Option<Vec<u32>> = target.split(['.', '-']).map(|p| p.parse().ok()).collect();
    if let (Some(cp), Some(tp)) = (&c_parts, &t_parts) {
        let max_len = cp.len().max(tp.len());
        for i in 0..max_len {
            let c = cp.get(i).copied().unwrap_or(0);
            let t = tp.get(i).copied().unwrap_or(0);
            match c.cmp(&t) {
                std::cmp::Ordering::Less => return FlashDirection::Upgrade,
                std::cmp::Ordering::Greater => return FlashDirection::Downgrade,
                std::cmp::Ordering::Equal => continue,
            }
        }
        return FlashDirection::Same;
    }
    // At least one version contains non-numeric segments; fall back to lexicographic.
    if current < target {
        FlashDirection::Upgrade
    } else {
        FlashDirection::Downgrade
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::manifest::{DriveMatch, FirmwareImage, FirmwareManifest};

    fn test_manifest() -> FirmwareManifest {
        FirmwareManifest {
            schema_version: 1,
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision_match: "1.0*".into(),
            capabilities: vec![],
            category: None,
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
    fn sha256_hex_known_input() {
        // SHA-256 of empty input
        let hash = sha256_hex(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_hello() {
        let hash = sha256_hex(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn build_flash_plan_success() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(plan.model_match);
        assert!(plan.image_checksum);
        assert!(plan.signature_present);
        assert_eq!(plan.target_version, "1.04");
    }

    #[test]
    fn build_flash_plan_image_not_found() {
        let manifest = test_manifest();
        let drive = test_drive();
        let err = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "nonexistent",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, FlashError::ImageNotFound(_)));
    }

    #[test]
    fn build_flash_plan_checksum_mismatch() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 999, // wrong size
                firmware_sha256: "wrong",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(!plan.image_checksum);
    }

    #[test]
    fn dry_run_all_pass() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(report.would_execute);
        assert_eq!(report.direction, FlashDirection::Upgrade);
        assert!(report.summary.contains("Flash ready"));
    }

    #[test]
    fn dry_run_blocks_on_checksum() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1,
                firmware_sha256: "bad",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("checksum failed"));
    }

    #[test]
    fn dry_run_blocks_on_no_signature() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: false,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("no signature"));
    }

    #[test]
    fn dry_run_blocks_on_not_confirmed() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: false,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("not confirmed"));
    }

    #[test]
    fn dry_run_blocks_on_model_mismatch() {
        let manifest = test_manifest();
        // Drive with wrong model — build_flash_plan sets model_match = false
        let bad_drive = DriveMatch {
            vendor: "OTHER".into(),
            model: "WRONG".into(),
            revision: "1.03".into(),
        };
        let plan = build_flash_plan(
            &manifest,
            &bad_drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(!plan.model_match);
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("model mismatch"));
    }

    #[test]
    fn dry_run_blocks_on_revision_mismatch() {
        let manifest = test_manifest();
        let drive = test_drive();
        let mut plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        // Force revision_check to false to cover that branch
        plan.revision_check = false;
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("revision mismatch"));
    }

    #[test]
    fn compare_versions_upgrade() {
        assert_eq!(compare_versions("1.03", "1.04"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("1.0", "2.0"), FlashDirection::Upgrade);
    }

    #[test]
    fn compare_versions_downgrade() {
        assert_eq!(compare_versions("1.04", "1.03"), FlashDirection::Downgrade);
        assert_eq!(compare_versions("2.0", "1.0"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_same() {
        assert_eq!(compare_versions("1.03", "1.03"), FlashDirection::Same);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), FlashDirection::Same);
    }

    #[test]
    fn compare_versions_same_numeric_different_text() {
        // "1.0" and "1.00" differ as strings but parse to [1, 0] vs [1, 0].
        // Exercises the post-loop return (line 190).
        assert_eq!(compare_versions("1.0", "1.00"), FlashDirection::Same);
    }

    #[test]
    fn compare_versions_different_lengths() {
        assert_eq!(compare_versions("1.0", "1.0.1"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("1.0.1", "1.0"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_non_numeric_fallback() {
        // Falls back to string comparison when parts aren't numeric
        assert_eq!(compare_versions("abc", "def"), FlashDirection::Upgrade);
        assert_eq!(compare_versions("def", "abc"), FlashDirection::Downgrade);
    }

    #[test]
    fn compare_versions_mixed_numeric_non_numeric() {
        // One parses as numeric, the other doesn't — falls back to string comparison
        assert_eq!(compare_versions("1.0", "abc"), FlashDirection::Upgrade);
    }

    #[test]
    fn build_flash_plan_vendor_mismatch() {
        let manifest = test_manifest();
        let mut drive = test_drive();
        drive.vendor = "LG".into();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(!plan.model_match); // vendor mismatch
    }

    #[test]
    fn build_flash_plan_model_mismatch() {
        let manifest = test_manifest();
        let mut drive = test_drive();
        drive.model = "WH16NS40".into();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(!plan.model_match);
    }

    #[test]
    fn build_flash_plan_revision_mismatch() {
        let manifest = test_manifest();
        let mut drive = test_drive();
        drive.revision = "2.00".into(); // doesn't match "1.0*"
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "2.00",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        assert!(!plan.model_match); // revision glob "1.0*" doesn't match "2.00"
    }

    #[test]
    fn dry_run_multiple_failures() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1,     // wrong size
                firmware_sha256: "x", // wrong hash
                signature_present: false,
                user_confirmed: false,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(!report.would_execute);
        assert!(report.summary.contains("checksum failed"));
        assert!(report.summary.contains("no signature"));
        assert!(report.summary.contains("not confirmed"));
    }

    #[test]
    fn dry_run_downgrade() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.05",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(report.would_execute);
        assert_eq!(report.direction, FlashDirection::Downgrade);
    }

    #[test]
    fn dry_run_same_version() {
        let manifest = test_manifest();
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.04",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert_eq!(report.direction, FlashDirection::Same);
    }

    #[test]
    fn check_warnings_empty_when_all_clean() {
        let manifest = test_manifest();
        let drive = test_drive();
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(warnings.is_empty());
    }

    #[test]
    fn check_warnings_broad_vendor_wildcard() {
        let mut manifest = test_manifest();
        manifest.vendor = "*".into();
        let drive = test_drive();
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(warnings.iter().any(|w| w.contains("ANY vendor")));
    }

    #[test]
    fn check_warnings_broad_model_wildcard() {
        let mut manifest = test_manifest();
        manifest.model = "*".into();
        let drive = test_drive();
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(warnings.iter().any(|w| w.contains("ANY model")));
    }

    #[test]
    fn check_warnings_broad_revision_wildcard() {
        let mut manifest = test_manifest();
        manifest.revision_match = "*".into();
        let drive = test_drive();
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(warnings.iter().any(|w| w.contains("ANY revision")));
    }

    #[test]
    fn check_warnings_category_mismatch_internal_vs_slim() {
        let mut manifest = test_manifest();
        manifest.category = Some("slim".into());
        let drive = DriveMatch {
            vendor: "HL-DT-ST".into(),
            model: "WH16NS40".into(),
            revision: "1.00".into(),
        };
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(
            warnings.iter().any(|w| w.contains("Category mismatch")),
            "expected category mismatch warning, got: {warnings:?}"
        );
    }

    #[test]
    fn check_warnings_internal_drive_matching_category() {
        let mut manifest = test_manifest();
        manifest.category = Some("internal".into());
        let drive = DriveMatch {
            vendor: "HL-DT-ST".into(),
            model: "BH16NS40".into(),
            revision: "1.00".into(),
        };
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(
            warnings.is_empty(),
            "matching internal category should not warn, got: {warnings:?}"
        );
    }

    #[test]
    fn check_warnings_unknown_drive_category_skips_category_check() {
        let mut manifest = test_manifest();
        manifest.category = Some("slim".into());
        let drive = DriveMatch {
            vendor: "PIONEER".into(),
            model: "DVD-RW DVR-218".into(),
            revision: "1.00".into(),
        };
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(
            !warnings.iter().any(|w| w.contains("Category mismatch")),
            "unknown drive category should not emit category warnings, got: {warnings:?}"
        );
    }

    #[test]
    fn check_warnings_category_mismatch_slim_vs_internal() {
        let mut manifest = test_manifest();
        manifest.category = Some("internal".into());
        let drive = DriveMatch {
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(), // BU starts with "bu" → slim heuristic
            revision: "1.00".into(),
        };
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(
            warnings.iter().any(|w| w.contains("Category mismatch")),
            "expected category mismatch warning, got: {warnings:?}"
        );
    }

    #[test]
    fn check_warnings_no_category_no_warning() {
        let manifest = test_manifest(); // category is None
        let drive = test_drive();
        let warnings = check_warnings(&manifest, &drive, &[], Language::English);
        assert!(warnings.is_empty());
    }

    #[test]
    fn dry_run_includes_warnings() {
        let mut manifest = test_manifest();
        manifest.vendor = "*".into(); // triggers warning
        let drive = test_drive();
        let plan = build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: "1.03",
                firmware_size: 1024,
                firmware_sha256: "abcd1234",
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();
        let report = dry_run(&plan, &[], Language::English);
        assert!(
            !report.warnings.is_empty(),
            "dry_run should include warnings"
        );
    }

    #[test]
    fn check_firmware_sdf_non_sdf_data() {
        // Encrypted raw blobs are not SDF0 — should return None
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(&[0x85, 0x4a, 0xc0, 0x75]);
        assert!(check_firmware_sdf(&data).is_none());
    }

    #[test]
    fn check_firmware_sdf_valid_sdf0() {
        // Build a minimal SDF0 container with metadata
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_le_bytes()); // version
        data.extend_from_slice(&24u32.to_le_bytes()); // header_size
        data.extend_from_slice(&24u32.to_le_bytes()); // table_offset
        data.extend_from_slice(&0u32.to_le_bytes()); // flags
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_le_bytes());
        data.extend_from_slice(metadata);

        let info = check_firmware_sdf(&data).unwrap();
        assert_eq!(info.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }

    fn build_sdf0_firmware_bytes(vendor: &str, model: &str) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let metadata = format!("Vendor\0{vendor}\0Model\0{model}\0");
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_le_bytes());
        data.extend_from_slice(metadata.as_bytes());
        data
    }

    #[test]
    fn check_warnings_fw_vendor_mismatch_from_sdf() {
        let manifest = test_manifest();
        let drive = test_drive();
        let firmware = build_sdf0_firmware_bytes("OtherVendor", "BU40N");
        let info = check_firmware_sdf(&firmware).expect("sdf metadata");
        assert_eq!(info.vendor.as_deref(), Some("OtherVendor"));
        let warnings = check_warnings(&manifest, &drive, &firmware, Language::English);
        assert!(warnings.iter().any(|w| w.contains("vendor")));
    }

    #[test]
    fn check_warnings_fw_model_mismatch_from_sdf() {
        let manifest = test_manifest();
        let drive = test_drive();
        let firmware = build_sdf0_firmware_bytes("HL-DT-ST", "OTHER");
        let info = check_firmware_sdf(&firmware).expect("sdf metadata");
        assert_eq!(info.model.as_deref(), Some("OTHER"));
        let warnings = check_warnings(&manifest, &drive, &firmware, Language::English);
        assert!(warnings.iter().any(|w| w.contains("model")));
    }

    #[test]
    fn push_sdf_metadata_warnings_skips_matching_vendor_and_model() {
        let manifest = test_manifest();
        let mut warnings = Vec::new();
        let sdf_info = FirmwareSdfInfo {
            vendor: Some("HL-DT-ST".into()),
            model: Some("BU40N".into()),
            firmware_version: None,
        };
        push_sdf_metadata_warnings(&mut warnings, &manifest, &sdf_info, Language::English);
        assert!(warnings.is_empty());
    }

    #[test]
    fn check_warnings_fw_vendor_and_model_match_from_sdf() {
        let manifest = test_manifest();
        let drive = test_drive();
        let firmware = build_sdf0_firmware_bytes("HL-DT-ST", "BU40N");
        let warnings = check_warnings(&manifest, &drive, &firmware, Language::English);
        assert!(!warnings.iter().any(|w| w.contains("vendor")));
        assert!(!warnings.iter().any(|w| w.contains("model")));
    }

    #[test]
    fn check_warnings_fw_vendor_match_model_mismatch_from_sdf() {
        let manifest = test_manifest();
        let drive = test_drive();
        let firmware = build_sdf0_firmware_bytes("HL-DT-ST", "WRONG-MODEL");
        let warnings = check_warnings(&manifest, &drive, &firmware, Language::English);
        assert!(!warnings.iter().any(|w| w.contains("vendor")));
        assert!(warnings.iter().any(|w| w.contains("model")));
    }

    #[test]
    fn check_firmware_sdf_padded_metadata_region() {
        let metadata = b"Vendor\0TestVendor\0Model\0TestModel\0";
        let payload_offset = 256u32;
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&payload_offset.to_le_bytes());
        data.extend_from_slice(metadata);
        data.resize(payload_offset as usize, 0xAA);
        data.extend(vec![0u8; 64]);

        let info = check_firmware_sdf(&data).unwrap();
        assert_eq!(info.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(info.model.as_deref(), Some("TestModel"));
    }
}
