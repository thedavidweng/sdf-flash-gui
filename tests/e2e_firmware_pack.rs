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
//
// Per ADR 0001 (docs/adr/0001-no-filename-based-firmware-logic.md), firmware
// properties must never be derived from filenames. This test harness collects
// `.bin` files by extension only and uses the binary content for all
// identification. Filenames appear solely in assertion messages for
// human-readable diagnostics.

use sdf_flash_gui::command::{self, Backend, Operation, PlanRequest};
use sdf_flash_gui::flash;
use sdf_flash_gui::sdf;

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Firmware pack location and discovery helpers
// ---------------------------------------------------------------------------

/// A discovered firmware file: its path on disk and basename for display.
/// No firmware metadata is parsed from the filename (ADR 0001).
#[derive(Debug, Clone)]
struct FirmwareFile {
    /// Basename for display in assertion messages only.
    filename: String,
    /// Absolute or relative path to the `.bin` file.
    path: PathBuf,
}

fn firmware_pack_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FIRMWARE_PACK_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Downloads/IA/All You Need Firmware Pack (MartyMcNuts)")
}

fn collect_firmware_files() -> Vec<FirmwareFile> {
    let pack_dir = firmware_pack_dir();
    if !pack_dir.exists() {
        return Vec::new();
    }

    let mut files = Vec::new();
    walk_dir(&pack_dir, &mut files);
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

fn walk_dir(dir: &Path, out: &mut Vec<FirmwareFile>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, out);
            } else if path.extension().is_some_and(|e| e == "bin") {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                out.push(FirmwareFile {
                    filename,
                    path: path.to_path_buf(),
                });
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

            // sha256 should be computable
            let _sha256 = flash::sha256_hex(&data);

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
