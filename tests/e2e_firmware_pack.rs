// End-to-end tests against the MartyMcNuts firmware pack.
//
// These tests require real firmware files that cannot be distributed in the
// repository. They are marked `#[ignore]` so CI skips them automatically.
//
// To run locally (with the firmware pack):
//   cargo test --test e2e_firmware_pack -- --ignored
//
// To run with a custom path:
//   FIRMWARE_PACK_DIR="/path/to/All You Need Firmware Pack (MartyMcNuts)" \
//     cargo test --test e2e_firmware_pack -- --ignored

use sdf_flash_gui::command::{self, Backend, Operation, PlanRequest};
use sdf_flash_gui::flash::{self, FlashPlanRequest};
use sdf_flash_gui::manifest::{DriveMatch, FirmwareImage, FirmwareManifest};
use sdf_flash_gui::orchestration;
use sdf_flash_gui::sdf;

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Firmware info and filename parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct FirmwareInfo {
    vendor: String,
    model: String,
    version: String,
    is_modified: bool,
    filename: String,
    path: PathBuf,
}

/// Parse the MartyMcNuts naming convention:
///   DE_{Vendor}_{Model}_{Version}[_-MK].bin
///
/// Vendor mapping: LG → HL-DT-ST, ASUS and Buffalo keep their names.
fn parse_firmware_filename(path: &Path) -> Option<FirmwareInfo> {
    let filename = path.file_name()?.to_str()?;
    if !filename.ends_with(".bin") {
        return None;
    }

    let stem = filename.strip_prefix("DE_")?.strip_suffix(".bin")?;

    let parts: Vec<&str> = stem.splitn(2, '_').collect();
    if parts.len() < 2 {
        return None;
    }

    let vendor_short = parts[0];
    let rest = parts[1];
    let segments: Vec<&str> = rest.split('_').collect();

    let (model, version, is_modified) = if segments.len() >= 2 {
        let last = segments.last().unwrap();

        if *last == "MK" {
            let ver_idx = segments.len() - 2;
            let version_raw = segments[ver_idx];
            let version = version_raw.trim_end_matches("-MK");
            let model_parts = &segments[..ver_idx];
            (model_parts.join("_"), version.to_string(), true)
        } else if last.ends_with("-MK") || last.ends_with("_MK") {
            let version = last.trim_end_matches("-MK").trim_end_matches("_MK");
            let model_parts = &segments[..segments.len() - 1];
            (model_parts.join("_"), version.to_string(), true)
        } else {
            let version = *last;
            let model_parts = &segments[..segments.len() - 1];
            (model_parts.join("_"), version.to_string(), false)
        }
    } else {
        (rest.to_string(), String::new(), false)
    };

    let vendor = match vendor_short {
        "LG" => "HL-DT-ST",
        "ASUS" => "ASUS",
        "Buffalo" => "Buffalo",
        other => other,
    }
    .to_string();

    Some(FirmwareInfo {
        vendor,
        model,
        version,
        is_modified,
        filename: filename.to_string(),
        path: path.to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// Filename parser tests — no real firmware needed, always runs in CI
// ---------------------------------------------------------------------------

#[test]
fn parse_filename_lg_standard() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_BU40N_1.00.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "BU40N");
    assert_eq!(info.version, "1.00");
    assert!(!info.is_modified);
}

#[test]
fn parse_filename_lg_mk_suffix() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_BU40N_1.03_MK.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "BU40N");
    assert_eq!(info.version, "1.03");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_asus_hyphen_model() {
    let info = parse_firmware_filename(Path::new("/pack/DE_ASUS_BW-16D1HT_3.02.bin")).unwrap();
    assert_eq!(info.vendor, "ASUS");
    assert_eq!(info.model, "BW-16D1HT");
    assert_eq!(info.version, "3.02");
    assert!(!info.is_modified);
}

#[test]
fn parse_filename_asus_mk_suffix() {
    let info = parse_firmware_filename(Path::new("/pack/DE_ASUS_BW-16D1HT_3.10_MK.bin")).unwrap();
    assert_eq!(info.vendor, "ASUS");
    assert_eq!(info.model, "BW-16D1HT");
    assert_eq!(info.version, "3.10");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_buffalo_letter_version() {
    let info = parse_firmware_filename(Path::new("/pack/DE_Buffalo_BRUHD-PU3_BU10.bin")).unwrap();
    assert_eq!(info.vendor, "Buffalo");
    assert_eq!(info.model, "BRUHD-PU3");
    assert_eq!(info.version, "BU10");
    assert!(!info.is_modified);
}

#[test]
fn parse_filename_buffalo_mk_suffix() {
    let info =
        parse_firmware_filename(Path::new("/pack/DE_Buffalo_BRUHD-PU3_BU12-MK.bin")).unwrap();
    assert_eq!(info.vendor, "Buffalo");
    assert_eq!(info.model, "BRUHD-PU3");
    assert_eq!(info.version, "BU12");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_hyphenated_model_with_mk() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_BP60NB10-NB12_1.02-MK.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "BP60NB10-NB12");
    assert_eq!(info.version, "1.02");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_simple_mk_variant() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_BP60NB10_1.00_MK.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "BP60NB10");
    assert_eq!(info.version, "1.00");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_multi_model_segments() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_WH16NS40-NS50_1.02.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "WH16NS40-NS50");
    assert_eq!(info.version, "1.02");
    assert!(!info.is_modified);
}

#[test]
fn parse_filename_slim_wp_model() {
    let info = parse_firmware_filename(Path::new("/pack/DE_LG_WP50NB40-NB50_1.03_MK.bin")).unwrap();
    assert_eq!(info.vendor, "HL-DT-ST");
    assert_eq!(info.model, "WP50NB40-NB50");
    assert_eq!(info.version, "1.03");
    assert!(info.is_modified);
}

#[test]
fn parse_filename_not_bin_extension() {
    assert!(parse_firmware_filename(Path::new("/pack/DE_LG_BU40N_1.00.txt")).is_none());
}

#[test]
fn parse_filename_no_de_prefix() {
    assert!(parse_firmware_filename(Path::new("/pack/LG_BU40N_1.00.bin")).is_none());
}

#[test]
fn parse_filename_unknown_vendor_passthrough() {
    let info = parse_firmware_filename(Path::new("/pack/DE_Pioneer_BDR-212_1.00.bin")).unwrap();
    assert_eq!(info.vendor, "Pioneer");
    assert_eq!(info.model, "BDR-212");
    assert_eq!(info.version, "1.00");
}

// ---------------------------------------------------------------------------
// Firmware pack location and discovery helpers
// ---------------------------------------------------------------------------

fn firmware_pack_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FIRMWARE_PACK_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Downloads/IA/All You Need Firmware Pack (MartyMcNuts)")
}

fn collect_firmware_files() -> Vec<FirmwareInfo> {
    let pack_dir = firmware_pack_dir();
    if !pack_dir.exists() {
        return Vec::new();
    }

    let mut files = Vec::new();
    walk_dir(&pack_dir, &mut files);
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

fn walk_dir(dir: &Path, out: &mut Vec<FirmwareInfo>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, out);
            } else if path.extension().is_some_and(|e| e == "bin") {
                if let Some(info) = parse_firmware_filename(&path) {
                    out.push(info);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Real firmware tests — #[ignore] so CI skips them
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn firmware_pack_discovery() {
    let files = collect_firmware_files();
    assert!(
        !files.is_empty(),
        "firmware pack not found at {}. Set FIRMWARE_PACK_DIR.",
        firmware_pack_dir().display()
    );
    assert!(
        files.len() >= 18,
        "expected at least 18 firmware files, found {}",
        files.len()
    );
}

#[test]
#[ignore]
fn firmware_pack_all_files_are_2mb() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        assert_eq!(
            data.len(),
            2_097_152,
            "{}: expected 2MB, got {} bytes",
            info.filename,
            data.len()
        );
    }
}

#[test]
#[ignore]
fn firmware_pack_all_files_have_unique_sha256() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let mut hashes: Vec<(String, String)> = Vec::new();
    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        hashes.push((info.filename.clone(), flash::sha256_hex(&data)));
    }

    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i].1, hashes[j].1,
                "duplicate SHA-256: {} and {}",
                hashes[i].0, hashes[j].0
            );
        }
    }
}

#[test]
#[ignore]
fn firmware_pack_not_sdf0_containers() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        let mut cursor = std::io::Cursor::new(&data);
        let err = sdf::parse_sdf0(&mut cursor).unwrap_err();
        assert!(
            matches!(err, sdf::SdfError::InvalidMagic { .. }),
            "{}: expected InvalidMagic, got: {err}",
            info.filename
        );
    }
}

#[test]
#[ignore]
fn firmware_pack_recovery_token_extraction() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        match command::extract_recovery_boot_token(&data) {
            Ok(token) => {
                assert_eq!(token.len(), 16);
                assert!(token.bytes().all(|b| b.is_ascii_graphic()));
            }
            Err(command::PlanError::InvalidRecoveryBootToken) => {} // expected for encrypted blobs
            Err(other) => panic!("{}: unexpected error: {other}", info.filename),
        }
    }
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_with_matching_manifest() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        let sha256 = flash::sha256_hex(&data);

        let manifest = FirmwareManifest {
            schema_version: 1,
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision_match: "*".to_string(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![FirmwareImage {
                image_id: "main".to_string(),
                filename: info.filename.clone(),
                target_version: info.version.clone(),
                size: data.len() as u64,
                sha256: sha256.clone(),
                signature_present: true,
            }],
        };

        let drive = DriveMatch {
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision: info.version.clone(),
        };

        let plan = flash::build_flash_plan(
            &manifest,
            &drive,
            FlashPlanRequest {
                image_id: "main",
                current_version: &info.version,
                firmware_size: data.len() as u64,
                firmware_sha256: &sha256,
                signature_present: true,
                user_confirmed: true,
            },
        )
        .unwrap();

        assert!(plan.model_match, "{}: model should match", info.filename);
        assert!(
            plan.image_checksum,
            "{}: checksum should match",
            info.filename
        );

        let report = flash::dry_run(&plan, &data, sdf_flash_gui::i18n::Language::English);
        assert!(
            report.would_execute,
            "{}: dry_run should pass. Summary: {}",
            info.filename, report.summary
        );
    }
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_checksum_mismatch_rejected() {
    let files = collect_firmware_files();
    assert!(files.len() >= 2);

    let a = &files[0];
    let b = &files[1];
    let data_a = std::fs::read(&a.path).unwrap();
    let sha256_a = flash::sha256_hex(&data_a);
    let data_b = std::fs::read(&b.path).unwrap();
    let sha256_b = flash::sha256_hex(&data_b);

    let manifest = FirmwareManifest {
        schema_version: 1,
        vendor: a.vendor.clone(),
        model: a.model.clone(),
        revision_match: "*".to_string(),
        capabilities: vec![],
        category: None,
        firmware_images: vec![FirmwareImage {
            image_id: "main".to_string(),
            filename: a.filename.clone(),
            target_version: a.version.clone(),
            size: data_a.len() as u64,
            sha256: sha256_a,
            signature_present: true,
        }],
    };

    let drive = DriveMatch {
        vendor: a.vendor.clone(),
        model: a.model.clone(),
        revision: a.version.clone(),
    };

    let plan = flash::build_flash_plan(
        &manifest,
        &drive,
        FlashPlanRequest {
            image_id: "main",
            current_version: &a.version,
            firmware_size: data_b.len() as u64,
            firmware_sha256: &sha256_b,
            signature_present: true,
            user_confirmed: true,
        },
    )
    .unwrap();

    assert!(!plan.image_checksum);
    let report = flash::dry_run(&plan, &data_b, sdf_flash_gui::i18n::Language::English);
    assert!(!report.would_execute);
    assert!(report.summary.contains("checksum"));
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_vendor_mismatch_rejected() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let lg = files.iter().find(|f| f.vendor == "HL-DT-ST").unwrap();
    let asus = files.iter().find(|f| f.vendor == "ASUS").unwrap();
    let data = std::fs::read(&lg.path).unwrap();
    let sha256 = flash::sha256_hex(&data);

    let manifest = FirmwareManifest {
        schema_version: 1,
        vendor: lg.vendor.clone(),
        model: lg.model.clone(),
        revision_match: "*".to_string(),
        capabilities: vec![],
        category: None,
        firmware_images: vec![FirmwareImage {
            image_id: "main".to_string(),
            filename: lg.filename.clone(),
            target_version: lg.version.clone(),
            size: data.len() as u64,
            sha256: sha256.clone(),
            signature_present: true,
        }],
    };

    let drive = DriveMatch {
        vendor: asus.vendor.clone(),
        model: asus.model.clone(),
        revision: asus.version.clone(),
    };

    let plan = flash::build_flash_plan(
        &manifest,
        &drive,
        FlashPlanRequest {
            image_id: "main",
            current_version: &asus.version,
            firmware_size: data.len() as u64,
            firmware_sha256: &sha256,
            signature_present: true,
            user_confirmed: true,
        },
    )
    .unwrap();

    assert!(!plan.model_match);
    let report = flash::dry_run(&plan, &data, sdf_flash_gui::i18n::Language::English);
    assert!(!report.would_execute);
    assert!(report.summary.contains("model"));
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_model_mismatch_rejected() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let bu40n = files.iter().find(|f| f.model == "BU40N").unwrap();
    let wh16ns60 = files.iter().find(|f| f.model == "WH16NS60").unwrap();
    let data = std::fs::read(&bu40n.path).unwrap();
    let sha256 = flash::sha256_hex(&data);

    let manifest = FirmwareManifest {
        schema_version: 1,
        vendor: bu40n.vendor.clone(),
        model: bu40n.model.clone(),
        revision_match: "*".to_string(),
        capabilities: vec![],
        category: None,
        firmware_images: vec![FirmwareImage {
            image_id: "main".to_string(),
            filename: bu40n.filename.clone(),
            target_version: bu40n.version.clone(),
            size: data.len() as u64,
            sha256: sha256.clone(),
            signature_present: true,
        }],
    };

    let drive = DriveMatch {
        vendor: wh16ns60.vendor.clone(),
        model: wh16ns60.model.clone(),
        revision: wh16ns60.version.clone(),
    };

    let plan = flash::build_flash_plan(
        &manifest,
        &drive,
        FlashPlanRequest {
            image_id: "main",
            current_version: &wh16ns60.version,
            firmware_size: data.len() as u64,
            firmware_sha256: &sha256,
            signature_present: true,
            user_confirmed: true,
        },
    )
    .unwrap();

    assert!(!plan.model_match);
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_no_signature_blocked() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let info = &files[0];
    let data = std::fs::read(&info.path).unwrap();
    let sha256 = flash::sha256_hex(&data);

    let manifest = FirmwareManifest {
        schema_version: 1,
        vendor: info.vendor.clone(),
        model: info.model.clone(),
        revision_match: "*".to_string(),
        capabilities: vec![],
        category: None,
        firmware_images: vec![FirmwareImage {
            image_id: "main".to_string(),
            filename: info.filename.clone(),
            target_version: info.version.clone(),
            size: data.len() as u64,
            sha256: sha256.clone(),
            signature_present: false,
        }],
    };

    let plan = flash::build_flash_plan(
        &manifest,
        &DriveMatch {
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision: info.version.clone(),
        },
        FlashPlanRequest {
            image_id: "main",
            current_version: &info.version,
            firmware_size: data.len() as u64,
            firmware_sha256: &sha256,
            signature_present: false,
            user_confirmed: true,
        },
    )
    .unwrap();

    let report = flash::dry_run(&plan, &data, sdf_flash_gui::i18n::Language::English);
    assert!(!report.would_execute);
    assert!(report.summary.contains("no signature"));
}

#[test]
#[ignore]
fn firmware_pack_flash_plan_not_confirmed_blocked() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let info = &files[0];
    let data = std::fs::read(&info.path).unwrap();
    let sha256 = flash::sha256_hex(&data);

    let manifest = FirmwareManifest {
        schema_version: 1,
        vendor: info.vendor.clone(),
        model: info.model.clone(),
        revision_match: "*".to_string(),
        capabilities: vec![],
        category: None,
        firmware_images: vec![FirmwareImage {
            image_id: "main".to_string(),
            filename: info.filename.clone(),
            target_version: info.version.clone(),
            size: data.len() as u64,
            sha256: sha256.clone(),
            signature_present: true,
        }],
    };

    let plan = flash::build_flash_plan(
        &manifest,
        &DriveMatch {
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision: info.version.clone(),
        },
        FlashPlanRequest {
            image_id: "main",
            current_version: &info.version,
            firmware_size: data.len() as u64,
            firmware_sha256: &sha256,
            signature_present: true,
            user_confirmed: false,
        },
    )
    .unwrap();

    let report = flash::dry_run(&plan, &data, sdf_flash_gui::i18n::Language::English);
    assert!(!report.would_execute);
    assert!(report.summary.contains("not confirmed"));
}

#[test]
#[ignore]
fn firmware_pack_orchestration_validate_flash() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let data = std::fs::read(&info.path).unwrap();
        let sha256 = flash::sha256_hex(&data);

        let manifest = FirmwareManifest {
            schema_version: 1,
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision_match: "*".to_string(),
            capabilities: vec![],
            category: None,
            firmware_images: vec![FirmwareImage {
                image_id: "main".to_string(),
                filename: info.filename.clone(),
                target_version: info.version.clone(),
                size: data.len() as u64,
                sha256: sha256.clone(),
                signature_present: true,
            }],
        };

        let drive = DriveMatch {
            vendor: info.vendor.clone(),
            model: info.model.clone(),
            revision: info.version.clone(),
        };

        let report = orchestration::validate_flash(
            &manifest,
            &drive,
            "main",
            &data,
            true,
            sdf_flash_gui::i18n::Language::English,
        )
        .unwrap();

        assert!(
            report.would_execute,
            "{}: validate_flash should pass. Summary: {}",
            info.filename, report.summary
        );
        assert_eq!(report.direction, flash::FlashDirection::Same);
    }
}

#[test]
#[ignore]
fn firmware_pack_command_plan_write() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    for info in &files {
        let device = "/dev/sr0";
        let confirmation = command::required_flash_confirmation(device);

        let plan = command::plan_command(PlanRequest {
            backend: Backend::SdfTool,
            tool_path: "sdftool".to_string(),
            sdf_path: String::new(),
            drive: device.to_string(),
            drive_is_mt1959: true,
            confirmation,
            operation: Operation::Write {
                firmware_path: info.path.to_string_lossy().to_string(),
                encrypted: false,
                include_boot_loader: false,
            },
        })
        .unwrap();

        assert_eq!(plan.command.program, "sdftool");
        assert_eq!(plan.command.args[5], info.path.to_string_lossy());
    }
}

#[test]
#[ignore]
fn firmware_pack_end_to_end_full_pipeline() {
    let files = collect_firmware_files();
    assert!(!files.is_empty());

    let mut passed = 0;
    let mut failed = Vec::new();

    for info in &files {
        let result = std::panic::catch_unwind(|| {
            let data = std::fs::read(&info.path).unwrap();
            assert!(!data.is_empty());
            assert_eq!(data.len(), 2_097_152);

            let sha256 = flash::sha256_hex(&data);

            let manifest = FirmwareManifest {
                schema_version: 1,
                vendor: info.vendor.clone(),
                model: info.model.clone(),
                revision_match: "*".to_string(),
                capabilities: vec![],
                category: None,
                firmware_images: vec![FirmwareImage {
                    image_id: "main".to_string(),
                    filename: info.filename.clone(),
                    target_version: info.version.clone(),
                    size: data.len() as u64,
                    sha256: sha256.clone(),
                    signature_present: true,
                }],
            };

            let image_id = orchestration::resolve_image_id(&manifest, None).unwrap();
            let drive = DriveMatch {
                vendor: info.vendor.clone(),
                model: info.model.clone(),
                revision: info.version.clone(),
            };

            let report = orchestration::validate_flash(
                &manifest,
                &drive,
                &image_id,
                &data,
                true,
                sdf_flash_gui::i18n::Language::English,
            )
            .unwrap();
            assert!(
                report.would_execute,
                "validation should pass: {}",
                report.summary
            );

            let device = "/dev/sr0";
            let confirmation = command::required_flash_confirmation(device);
            let plan = command::plan_command(PlanRequest {
                backend: Backend::SdfTool,
                tool_path: "sdftool".to_string(),
                sdf_path: String::new(),
                drive: device.to_string(),
                drive_is_mt1959: true,
                confirmation,
                operation: Operation::Write {
                    firmware_path: info.path.to_string_lossy().to_string(),
                    encrypted: false,
                    include_boot_loader: false,
                },
            })
            .unwrap();

            assert_eq!(plan.command.program, "sdftool");
            assert_eq!(plan.command.args[5], info.path.to_string_lossy());

            // Encrypted blobs should not parse as SDF0
            let mut cursor = std::io::Cursor::new(&data);
            assert!(sdf::parse_sdf0(&mut cursor).is_err());
        });

        match result {
            Ok(()) => passed += 1,
            Err(_) => failed.push(info.filename.clone()),
        }
    }

    if !failed.is_empty() {
        panic!(
            "{}/{} firmware files failed: {failed:?}",
            files.len() - passed,
            files.len()
        );
    }
    assert!(
        passed >= 18,
        "expected at least 18 to pass, only {passed} passed"
    );
}
