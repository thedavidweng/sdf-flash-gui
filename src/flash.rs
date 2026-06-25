// Flash safety model — validation and dry-run.

use crate::manifest::{glob_match, DriveMatch, FirmwareManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashDirection {
    Upgrade,
    Downgrade,
    Same,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashPlan {
    pub drive: DriveMatch,
    pub manifest: FirmwareManifest,
    pub image_id: String,
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
#[allow(dead_code)]
pub enum FlashError {
    #[error("model mismatch: drive {drive_vendor} {drive_model} does not match manifest {manifest_vendor} {manifest_model}")]
    ModelMismatch {
        drive_vendor: String,
        drive_model: String,
        manifest_vendor: String,
        manifest_model: String,
    },

    #[error("image checksum mismatch for image {image_id}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        image_id: String,
        expected: String,
        actual: String,
    },

    #[error("no signature present for image {image_id}")]
    NoSignature { image_id: String },

    #[error("firmware image not found: {0}")]
    ImageNotFound(String),
}

pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{b:02x}")).collect()
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
        image_id: request.image_id.to_string(),
        current_version: request.current_version.to_string(),
        target_version: image.target_version.clone(),
        model_match: model_match_full,
        revision_check: true,
        image_checksum,
        signature_present: request.signature_present,
        user_confirmed: request.user_confirmed,
    })
}

pub fn dry_run(plan: &FlashPlan) -> FlashReport {
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

    let summary = if would_execute {
        format!(
            "Flash ready: {} {} firmware {} -> {} ({:?})",
            plan.drive.vendor,
            plan.drive.model,
            plan.current_version,
            plan.target_version,
            direction,
        )
    } else {
        let mut failures = Vec::new();
        if !checks.model_match {
            failures.push("model mismatch");
        }
        if !checks.revision_check {
            failures.push("revision mismatch");
        }
        if !checks.image_checksum {
            failures.push("checksum failed");
        }
        if !checks.signature_present {
            failures.push("no signature");
        }
        if !checks.user_confirmed {
            failures.push("not confirmed");
        }
        format!("Flash blocked: {}", failures.join(", "))
    };

    FlashReport {
        would_execute,
        direction,
        checks,
        summary,
    }
}

fn compare_versions(current: &str, target: &str) -> FlashDirection {
    if current == target {
        return FlashDirection::Same;
    }
    let cp: Option<Vec<u32>> = current.split(['.', '-']).map(|p| p.parse().ok()).collect();
    let tp: Option<Vec<u32>> = target.split(['.', '-']).map(|p| p.parse().ok()).collect();
    if let (Some(cp), Some(tp)) = (&cp, &tp) {
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
    // At least one version string contains non-numeric components (parse failed).
    // Fall back to lexicographic comparison. Equal is impossible here because
    // identical strings are caught by the early return at the top of the function.
    if current < target {
        FlashDirection::Upgrade
    } else {
        FlashDirection::Downgrade
    }
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
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
        let report = dry_run(&plan);
        assert_eq!(report.direction, FlashDirection::Same);
    }

    #[test]
    fn sha256_hex_deterministic() {
        let a = sha256_hex(b"test data");
        let b = sha256_hex(b"test data");
        assert_eq!(a, b);
    }

    #[test]
    fn sha256_hex_different_inputs() {
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"world");
        assert_ne!(a, b);
    }
}
