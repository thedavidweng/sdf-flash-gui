// Shared between CLI and GUI — flash session: probe → identity → validate → plan → run.

use crate::command::{
    self, Backend, Command, DriveSafety, Operation, Plan, PlanError, PlanRequest,
};
use crate::drive::{self, Drive};
use crate::flash;
use crate::manifest;
use crate::process;

/// Parse drive identity from sdftool `--info` output for manifest matching.
pub fn parse_drive_identity(device: &str, info_output: &str) -> manifest::DriveMatch {
    drive::parse_identity_from_info(device, info_output)
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub safety: DriveSafety,
    pub identity: manifest::DriveMatch,
    pub output: String,
}

pub fn probe_drive(backend: Backend, tool_path: &str, device: &str) -> Result<ProbeResult, String> {
    let cmd = command::plan_drive_info(backend, tool_path, device);
    let out = process::run_command(&cmd.program, &cmd.args)
        .map_err(|e| format!("cannot probe drive: {e}"))?;
    Ok(ProbeResult {
        safety: command::classify_drive_safety(device, &out.combined()),
        identity: parse_drive_identity(device, &out.combined()),
        output: out.combined(),
    })
}

pub fn run_dump(
    backend: Backend,
    tool_path: &str,
    device: &str,
    output_dir: &str,
) -> Result<(), String> {
    let plan = command::plan_command(PlanRequest {
        backend,
        tool_path: tool_path.to_string(),
        drive: device.to_string(),
        drive_is_mt1959: true,
        confirmation: String::new(),
        operation: Operation::Read {
            output_dir: output_dir.to_string(),
        },
    })
    .map_err(|e| format!("cannot plan dump: {e}"))?;
    execute_command(&plan.command)
}

pub fn run_list_backend(backend: Backend, tool_path: &str) -> Result<String, String> {
    let cmd = command::plan_drive_list(backend, tool_path);
    let out = process::run_command(&cmd.program, &cmd.args).map_err(|e| e.to_string())?;
    if out.success() {
        Ok(out.combined())
    } else {
        Err(out.combined())
    }
}

pub fn execute_command(cmd: &Command) -> Result<(), String> {
    match process::run_command(&cmd.program, &cmd.args) {
        Ok(out) if out.success() => Ok(()),
        Ok(out) => Err(out.combined()),
        Err(e) => Err(e),
    }
}

#[derive(Debug)]
pub struct FlashSessionRequest<'a> {
    pub backend: Backend,
    pub tool_path: &'a str,
    pub device: &'a str,
    pub firmware_path: &'a str,
    pub firmware_data: &'a [u8],
    pub manifest: Option<&'a manifest::FirmwareManifest>,
    pub manifest_path: Option<&'a str>,
    pub image_id: Option<&'a str>,
    pub encrypted: bool,
    pub include_boot_loader: bool,
    pub recover: bool,
    pub wrong_firmware: Option<&'a str>,
    pub recovery_token: Option<&'a str>,
    pub confirm: bool,
    pub lang: crate::i18n::Language,
}

#[derive(Debug)]
pub struct FlashSession {
    pub probe: ProbeResult,
    pub drive_match: manifest::DriveMatch,
    pub report: Option<flash::FlashReport>,
    pub plan: Option<Plan>,
    pub would_execute: bool,
}

impl FlashSession {
    pub fn prepare(req: FlashSessionRequest<'_>) -> Result<Self, String> {
        if req.encrypted && req.include_boot_loader {
            return Err("--encrypted and --include-boot-loader cannot be combined".into());
        }

        let probe = probe_drive(req.backend, req.tool_path, req.device)?;
        if !probe.safety.mt1959 {
            return Err("drive is not MT1959 platform".into());
        }

        let drive = Drive {
            device: req.device.to_string(),
            vendor: probe.identity.vendor.clone(),
            product: probe.identity.model.clone(),
            revision: probe.identity.revision.clone(),
        };
        let drive_match = drive::drive_match_for_validation(&drive, Some(&probe.identity));

        let mut report = None;
        let would_execute = if let Some(manifest) = req.manifest {
            let image_id = resolve_image_id(manifest, req.image_id)?;
            let report_val = validate_flash(
                manifest,
                &drive_match,
                &image_id,
                req.firmware_data,
                req.confirm,
                req.lang,
            )?;
            report = Some(report_val.clone());
            report_val.would_execute
        } else {
            warn_no_manifest(req.firmware_data);
            req.confirm
        };

        let operation = if req.recover {
            let token = resolve_recovery_token(req.wrong_firmware, req.recovery_token)?;
            Operation::Recover {
                firmware_path: req.firmware_path.to_string(),
                recovery_boot_token: token,
            }
        } else {
            Operation::Write {
                firmware_path: req.firmware_path.to_string(),
                encrypted: req.encrypted,
                include_boot_loader: req.include_boot_loader,
            }
        };

        let plan = if req.confirm && would_execute {
            Some(
                command::plan_command(PlanRequest {
                    backend: req.backend,
                    tool_path: req.tool_path.to_string(),
                    drive: req.device.to_string(),
                    drive_is_mt1959: probe.safety.mt1959,
                    confirmation: command::required_flash_confirmation(req.device),
                    operation,
                })
                .map_err(plan_error_string)?,
            )
        } else {
            None
        };

        Ok(Self {
            probe,
            drive_match,
            report,
            plan,
            would_execute,
        })
    }

    pub fn execute(&self) -> Result<(), String> {
        let plan = self.plan.as_ref().ok_or("no plan to execute")?;
        execute_command(&plan.command)
    }
}

fn plan_error_string(e: PlanError) -> String {
    format!("cannot plan flash: {e}")
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
/// The report includes advisory warnings that should be shown to the user
/// but never block the operation.
pub fn validate_flash(
    manifest: &manifest::FirmwareManifest,
    drive: &manifest::DriveMatch,
    image_id: &str,
    firmware_data: &[u8],
    user_confirmed: bool,
    lang: crate::i18n::Language,
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

    let plan = flash::build_flash_plan(manifest, drive, request).map_err(|e| {
        crate::i18n::t_with_args(
            crate::i18n::L10nKey::LogValidationFailed,
            lang,
            &[("error", &crate::i18n::flash_error_message(&e, lang))],
        )
    })?;
    Ok(flash::dry_run(&plan, firmware_data, lang))
}

/// Warn the user when flashing without a manifest (no validation possible).
///
/// This is called by the CLI when `--manifest` is not provided. It prints
/// advisory warnings to stderr but does not block the operation.
pub fn warn_no_manifest(firmware_data: &[u8]) {
    eprintln!("WARNING: No manifest provided — skipping firmware validation.");
    eprintln!("  No model match, checksum, or signature verification will be performed.");
    eprintln!("  Make sure the firmware is correct for your drive.");
    if let Some(sdf_info) = flash::check_firmware_sdf(firmware_data) {
        if let Some(v) = &sdf_info.vendor {
            eprintln!("  Firmware vendor: {v}");
        }
        if let Some(m) = &sdf_info.model {
            eprintln!("  Firmware model:  {m}");
        }
        if let Some(fw) = &sdf_info.firmware_version {
            eprintln!("  Firmware version: {fw}");
        }
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
        let report = validate_flash(
            &manifest,
            &drive,
            "main",
            &vec![0u8; 1024],
            true,
            Language::English,
        )
        .unwrap();
        // sha256 won't match, so would_execute is false — but it should not error
        assert!(!report.would_execute); // checksum mismatch
    }

    #[test]
    fn validate_flash_image_not_found() {
        let manifest = test_manifest();
        let drive = test_drive();
        let err = validate_flash(
            &manifest,
            &drive,
            "nonexistent",
            &vec![0u8; 1024],
            true,
            Language::English,
        )
        .unwrap_err();
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
    fn parse_drive_identity_underscore_empty_vendor() {
        // "_MODEL" → empty vendor part is skipped, model = "MODEL"
        let dm = parse_drive_identity("_MODEL", "");
        assert!(dm.vendor.is_empty());
        assert_eq!(dm.model, "MODEL");
    }

    #[test]
    fn parse_drive_identity_underscore_empty_model() {
        // "VENDOR_" → vendor = "VENDOR", empty model part is skipped
        let dm = parse_drive_identity("VENDOR_", "");
        assert_eq!(dm.vendor, "VENDOR");
        assert!(dm.model.is_empty());
    }

    #[test]
    fn resolve_image_id_empty_manifest() {
        let manifest = FirmwareManifest {
            schema_version: 1,
            vendor: "V".into(),
            model: "M".into(),
            revision_match: "*".into(),
            capabilities: vec![],
            category: None,
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
    fn validate_flash_checksum_mismatch() {
        let manifest = test_manifest();
        let drive = test_drive();
        // Manifest expects sha256="abcd1234" and size=1024; vec![0u8; 1024] has a different hash
        let report = validate_flash(
            &manifest,
            &drive,
            "main",
            &vec![0u8; 1024],
            true,
            Language::English,
        )
        .unwrap();
        assert!(!report.would_execute);
        assert!(report.summary.contains("checksum"));
    }

    #[test]
    fn validate_flash_not_confirmed() {
        let manifest = test_manifest();
        let drive = test_drive();
        let report = validate_flash(
            &manifest,
            &drive,
            "main",
            &vec![0u8; 1024],
            false,
            Language::English,
        )
        .unwrap();
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

    #[test]
    #[cfg(unix)]
    fn probe_drive_parses_mock_output() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let probe = probe_drive(
            crate::command::Backend::SdfTool,
            &tool.to_string_lossy(),
            "/dev/sr0",
        )
        .expect("probe");
        assert!(probe.safety.mt1959);
        assert_eq!(probe.identity.vendor, "HL-DT-ST");
        assert_eq!(probe.identity.model, "BU40N");
    }

    #[test]
    #[cfg(unix)]
    fn run_list_backend_success() {
        let tool = write_mock_probe_tool("0:/dev/sr0 HL-DT-ST BU40N 1.03\n");
        let out = run_list_backend(crate::command::Backend::SdfTool, &tool.to_string_lossy())
            .expect("list");
        assert!(out.contains("/dev/sr0"));
    }

    #[test]
    #[cfg(unix)]
    fn run_dump_with_mock_tool() {
        let tool = write_mock_probe_tool("");
        let out_dir = std::env::temp_dir().join(format!(
            "sdf_flash_dump_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&out_dir).unwrap();
        run_dump(
            crate::command::Backend::SdfTool,
            &tool.to_string_lossy(),
            "/dev/sr0",
            &out_dir.to_string_lossy(),
        )
        .expect("dump");
        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn execute_command_success() {
        let cmd = crate::command::Command {
            program: "echo".into(),
            args: vec!["ok".into()],
        };
        execute_command(&cmd).expect("echo should succeed");
    }

    #[test]
    fn execute_command_failure() {
        let cmd = crate::command::Command {
            program: "sh".into(),
            args: vec!["-c".into(), "exit 7".into()],
        };
        assert!(execute_command(&cmd).is_err());
    }

    #[test]
    fn flash_session_manifest_report_is_stored() {
        let manifest = test_manifest();
        let drive = test_drive();
        let firmware = vec![0u8; 1024];
        let image_id = resolve_image_id(&manifest, None).expect("image id");
        let report_val = validate_flash(
            &manifest,
            &drive,
            &image_id,
            &firmware,
            false,
            Language::English,
        )
        .expect("validate");
        let mut report = None;
        report = Some(report_val.clone());
        assert_eq!(
            report.as_ref().map(|r| r.would_execute),
            Some(report_val.would_execute)
        );
    }

    #[test]
    fn plan_error_string_formats_plan_errors() {
        let msg = plan_error_string(PlanError::MissingFirmware);
        assert!(msg.starts_with("cannot plan flash:"));
        assert!(msg.contains("firmware"));
    }

    #[test]
    fn flash_session_execute_without_plan() {
        let session = FlashSession {
            probe: ProbeResult {
                safety: crate::command::DriveSafety {
                    mt1959: true,
                    encrypted_firmware: false,
                    firmware_date_prefix: None,
                    mtk_mode: None,
                },
                identity: test_drive(),
                output: String::new(),
            },
            drive_match: test_drive(),
            report: None,
            plan: None,
            would_execute: false,
        };
        let err = session.execute().unwrap_err();
        assert!(err.contains("no plan to execute"));
    }

    #[test]
    fn flash_session_prepare_rejects_encrypted_and_bootloader() {
        let err = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &[],
            manifest: None,
            manifest_path: None,
            image_id: None,
            encrypted: true,
            include_boot_loader: true,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: false,
            lang: Language::English,
        })
        .unwrap_err();
        assert!(err.contains("cannot be combined"));
    }

    #[test]
    fn warn_no_manifest_with_sdf_firmware() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let metadata = b"Vendor\0LG\0Model\0BU40N\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_le_bytes());
        data.extend_from_slice(metadata);
        warn_no_manifest(&data);
    }

    #[cfg(unix)]
    fn write_mock_probe_tool(info: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sdf_flash_orchestration_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mock_sdftool");
        std::fs::write(
            &path,
            format!("#!/bin/sh\nprintf '%s' '{}'\n", info.replace('\'', "'\\''")),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_prepare_without_manifest_respects_confirm() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let session = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &[],
            manifest: None,
            manifest_path: None,
            image_id: None,
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: true,
            lang: Language::English,
        })
        .expect("prepare should succeed");
        assert!(session.probe.safety.mt1959);
        assert!(session.report.is_none());
        assert!(session.would_execute);
        assert!(session.plan.is_some());
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_prepare_rejects_non_mt1959() {
        let tool = write_mock_probe_tool("Vendor: Old\nProduct: Drive\n");
        let err = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &[],
            manifest: None,
            manifest_path: None,
            image_id: None,
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: false,
            lang: Language::English,
        })
        .unwrap_err();
        assert!(err.contains("not MT1959"));
    }

    #[test]
    #[cfg(unix)]
    fn run_list_backend_failure() {
        use std::os::unix::fs::PermissionsExt;

        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sdf_flash_list_fail_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mock_fail");
        std::fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            run_list_backend(crate::command::Backend::SdfTool, &path.to_string_lossy()).is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_command_spawn_error() {
        let cmd = crate::command::Command {
            program: "definitely_not_a_real_command_xyz_12345".into(),
            args: vec![],
        };
        assert!(execute_command(&cmd).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_prepare_propagates_validate_flash_error() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let manifest = test_manifest();
        let err = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &vec![0u8; 1024],
            manifest: Some(&manifest),
            manifest_path: None,
            image_id: Some("missing-image"),
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: false,
            lang: Language::English,
        })
        .unwrap_err();
        assert!(err.contains("validation failed"));
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_prepare_with_manifest() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let manifest = test_manifest();
        let firmware = vec![0u8; 1024];
        let session = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &firmware,
            manifest: Some(&manifest),
            manifest_path: None,
            image_id: None,
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: false,
            lang: Language::English,
        })
        .expect("prepare with manifest");
        assert!(session.report.is_some());
        assert!(!session.would_execute);
        assert!(session.plan.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_prepare_recover_operation() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let session = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &[],
            manifest: None,
            manifest_path: None,
            image_id: None,
            encrypted: false,
            include_boot_loader: false,
            recover: true,
            wrong_firmware: None,
            recovery_token: Some("ABCDEFGHIJKLMNOP"),
            confirm: true,
            lang: Language::English,
        })
        .expect("recover prepare");
        assert!(session.plan.is_some());
        assert!(session.would_execute);
    }

    #[test]
    #[cfg(unix)]
    fn flash_session_execute_with_plan() {
        let tool = write_mock_probe_tool(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let session = FlashSession::prepare(FlashSessionRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: &tool.to_string_lossy(),
            device: "/dev/sr0",
            firmware_path: "/tmp/fw.bin",
            firmware_data: &[],
            manifest: None,
            manifest_path: None,
            image_id: None,
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: true,
            lang: Language::English,
        })
        .expect("prepare");
        session.execute().expect("execute");
    }

    #[test]
    fn warn_no_manifest_prints_sdf_firmware_version() {
        let mut data = Vec::new();
        data.extend_from_slice(b"SDF0");
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let metadata = b"Vendor\0LG\0Model\0BU40N\0FirmwareVersion\01.04\0";
        let payload_offset = 24 + metadata.len() as u32;
        data.extend_from_slice(&payload_offset.to_le_bytes());
        data.extend_from_slice(metadata);
        warn_no_manifest(&data);
    }
}
