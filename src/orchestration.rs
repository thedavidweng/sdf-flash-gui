use crate::command::{self, Backend, Command, Operation, Plan, PlanError, PlanRequest};
use crate::drive::{self, DriveIdentity, DriveSafety};
use crate::process::{CommandOutput, CommandRunOutcome, OperationControl, ProcessRunner};
use crate::process_runner::NativeRunner;

// ── Confirmation ───────────────────────────────────────────────────

/// How the user confirmed a destructive firmware write/recover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashConfirm {
    /// Dry-run / not confirmed.
    None,
    /// CLI `--confirm`: treated as typing the required `FLASH <device>` string.
    Flag,
    /// GUI typed confirmation string.
    Typed(String),
}

impl FlashConfirm {
    pub fn is_confirmed(&self, device: &str) -> bool {
        match self {
            Self::None => false,
            Self::Flag => true,
            Self::Typed(s) => command::confirmation_matches(device, s),
        }
    }

    pub fn plan_confirmation(&self, device: &str) -> String {
        match self {
            Self::None => String::new(),
            Self::Flag => command::required_flash_confirmation(device),
            Self::Typed(s) => s.clone(),
        }
    }
}

// ── Backend outcome ────────────────────────────────────────────────

/// Failure modes shared by every cancellable backend operation (probe, list,
/// streaming write/read, execute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendOpError {
    Failed(String),
    Cancelled,
    NeedsForceKill,
}

impl std::fmt::Display for BackendOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(msg) => write!(f, "{msg}"),
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::NeedsForceKill => write!(f, "backend did not stop; force kill required"),
        }
    }
}

// ── Probe ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub safety: DriveSafety,
    pub identity: DriveIdentity,
    pub output: String,
}

/// Build a [`ProbeResult`] from tool output (no process I/O).
pub fn probe_from_output(device: &str, output: &str) -> ProbeResult {
    let drive::ProbeInterpretation { safety, identity } = drive::interpret_info(device, output);
    ProbeResult {
        safety,
        identity,
        output: output.to_string(),
    }
}

/// Probe a drive via the process runner seam (shared by CLI and GUI).
pub fn probe_drive_with(
    backend: Backend,
    tool_path: &str,
    device: &str,
    runner: &dyn ProcessRunner,
    control: Option<&OperationControl>,
) -> Result<ProbeResult, BackendOpError> {
    let cmd = command::plan_drive_info(backend, tool_path, device);
    match runner.run_command(&cmd.program, &cmd.args, control) {
        Ok(CommandRunOutcome::Completed(out)) => {
            let combined = out.combined();
            if !out.success() {
                return Err(BackendOpError::Failed(if combined.is_empty() {
                    "probe command failed".into()
                } else {
                    combined
                }));
            }
            Ok(probe_from_output(device, &combined))
        }
        Ok(CommandRunOutcome::Cancelled) => Err(BackendOpError::Cancelled),
        Ok(CommandRunOutcome::NeedsForceKill) => Err(BackendOpError::NeedsForceKill),
        Err(e) => Err(BackendOpError::Failed(e)),
    }
}

/// Convenience probe using the native process runner (CLI / tests).
pub fn probe_drive(backend: Backend, tool_path: &str, device: &str) -> Result<ProbeResult, String> {
    probe_drive_with(backend, tool_path, device, &NativeRunner, None)
        .map_err(|e| format!("cannot probe drive: {e}"))
}

// ── Plan helpers (read / dump / list) ──────────────────────────────

/// Plan a firmware dump (read) operation.
pub fn plan_read(
    backend: Backend,
    tool_path: &str,
    sdf_path: &str,
    device: &str,
    output_dir: &str,
    drive_is_mt1959: bool,
) -> Result<Plan, String> {
    command::plan_command(PlanRequest {
        backend,
        tool_path: tool_path.to_string(),
        sdf_path: sdf_path.to_string(),
        drive: device.to_string(),
        drive_is_mt1959,
        confirmation: String::new(),
        operation: Operation::Read {
            output_dir: output_dir.to_string(),
        },
    })
    .map_err(|e| format!("cannot plan dump: {e}"))
}

pub fn run_dump(
    backend: Backend,
    tool_path: &str,
    sdf_path: &str,
    device: &str,
    output_dir: &str,
) -> Result<(), String> {
    let plan = plan_read(backend, tool_path, sdf_path, device, output_dir, true)?;
    execute_command(&plan.command)
}

/// List drives via backend `-l`, using the shared process runner seam.
pub fn run_list_backend_with(
    backend: Backend,
    tool_path: &str,
    runner: &dyn ProcessRunner,
    control: Option<&OperationControl>,
) -> Result<CommandOutput, BackendOpError> {
    let cmd = command::plan_drive_list(backend, tool_path);
    match runner.run_command(&cmd.program, &cmd.args, control) {
        Ok(CommandRunOutcome::Completed(out)) if out.success() => Ok(out),
        Ok(CommandRunOutcome::Completed(out)) => Err(BackendOpError::Failed(out.combined())),
        Ok(CommandRunOutcome::Cancelled) => Err(BackendOpError::Cancelled),
        Ok(CommandRunOutcome::NeedsForceKill) => Err(BackendOpError::NeedsForceKill),
        Err(e) => Err(BackendOpError::Failed(e)),
    }
}

pub fn execute_command(cmd: &Command) -> Result<(), String> {
    execute_command_with(&NativeRunner, cmd).map_err(|e| e.to_string())
}

pub fn execute_command_with(
    runner: &dyn ProcessRunner,
    cmd: &Command,
) -> Result<(), BackendOpError> {
    match runner.run_command(&cmd.program, &cmd.args, None) {
        Ok(CommandRunOutcome::Completed(out)) if out.success() => Ok(()),
        Ok(CommandRunOutcome::Completed(out)) => Err(BackendOpError::Failed(out.combined())),
        Ok(CommandRunOutcome::Cancelled) => Err(BackendOpError::Cancelled),
        Ok(CommandRunOutcome::NeedsForceKill) => Err(BackendOpError::NeedsForceKill),
        Err(e) => Err(BackendOpError::Failed(e)),
    }
}

/// Run a planned command with per-line streaming via the process runner seam.
pub fn run_streaming_with(
    cmd: &Command,
    runner: &dyn ProcessRunner,
    on_line: &dyn Fn(&str),
    control: Option<&OperationControl>,
) -> Result<CommandOutput, BackendOpError> {
    match runner.run_command_streaming(&cmd.program, &cmd.args, on_line, control) {
        Ok(CommandRunOutcome::Completed(out)) => Ok(out),
        Ok(CommandRunOutcome::Cancelled) => Err(BackendOpError::Cancelled),
        Ok(CommandRunOutcome::NeedsForceKill) => Err(BackendOpError::NeedsForceKill),
        Err(e) => Err(BackendOpError::Failed(e)),
    }
}

/// Extract a 0–100 progress value from a tool output line (MakeMKV PRGV, `NN%`, etc.).
pub fn parse_progress_percent(line: &str) -> Option<f32> {
    let line = line.trim();

    if let Some(rest) = line.strip_prefix("PRGV:") {
        let mut parts = rest.split(',');
        let current: f32 = parts.next()?.parse().ok()?;
        let total: f32 = parts.next()?.parse().ok()?;
        if total > 0.0 {
            return Some((current / total * 100.0).clamp(0.0, 100.0));
        }
    }

    if let Some(idx) = line.rfind('%') {
        let before = &line[..idx];
        let start = before
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |i| i + 1);
        if let Ok(n) = before[start..].parse::<f32>() {
            return Some(n.clamp(0.0, 100.0));
        }
    }

    None
}

// ── Firmware operation (write / recover) — no re-probe ─────────────

/// Inputs for planning a write/recover after identity is known.
/// Used by both GUI (pre-probed) and [`FlashSession::prepare`] (after probe).
#[derive(Debug)]
pub struct FirmwareOpRequest<'a> {
    pub backend: Backend,
    pub tool_path: &'a str,
    pub sdf_path: &'a str,
    pub device: &'a str,
    pub drive_is_mt1959: bool,
    pub firmware_path: &'a str,
    pub encrypted: bool,
    pub include_boot_loader: bool,
    pub recover: bool,
    pub wrong_firmware: Option<&'a str>,
    pub recovery_token: Option<&'a str>,
    pub confirm: FlashConfirm,
}

#[derive(Debug)]
pub struct PreparedFirmwareOp {
    pub plan: Option<Plan>,
    pub would_execute: bool,
}

/// Shared write/recover planning through [`command::plan_command`], the single
/// source of plan gates.
///
/// Every gate runs even when unconfirmed, so a dry-run validates the full
/// request; a missing confirmation alone yields `plan: None`.
pub fn prepare_firmware_op(req: FirmwareOpRequest<'_>) -> Result<PreparedFirmwareOp, String> {
    let would_execute = req.confirm.is_confirmed(req.device);

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

    match command::plan_command(PlanRequest {
        backend: req.backend,
        tool_path: req.tool_path.to_string(),
        sdf_path: req.sdf_path.to_string(),
        drive: req.device.to_string(),
        drive_is_mt1959: req.drive_is_mt1959,
        confirmation: req.confirm.plan_confirmation(req.device),
        operation,
    }) {
        Ok(plan) => Ok(PreparedFirmwareOp {
            plan: Some(plan),
            would_execute,
        }),
        Err(PlanError::ConfirmationMismatch { .. }) if !would_execute => Ok(PreparedFirmwareOp {
            plan: None,
            would_execute: false,
        }),
        Err(e) => Err(plan_error_string(e)),
    }
}

// ── Full session (probe + prepare) — CLI and full GUI execute ──────

#[derive(Debug)]
pub struct FlashSessionRequest<'a> {
    pub backend: Backend,
    pub tool_path: &'a str,
    pub sdf_path: &'a str,
    pub device: &'a str,
    pub firmware_path: &'a str,
    pub encrypted: bool,
    pub include_boot_loader: bool,
    pub recover: bool,
    pub wrong_firmware: Option<&'a str>,
    pub recovery_token: Option<&'a str>,
    pub confirm: FlashConfirm,
}

#[derive(Debug)]
pub struct FlashSession {
    pub probe: ProbeResult,
    pub plan: Option<Plan>,
    pub would_execute: bool,
}

impl FlashSession {
    pub fn prepare(req: FlashSessionRequest<'_>) -> Result<Self, String> {
        Self::prepare_with(req, &NativeRunner, None)
    }

    pub fn prepare_with(
        req: FlashSessionRequest<'_>,
        runner: &dyn ProcessRunner,
        control: Option<&OperationControl>,
    ) -> Result<Self, String> {
        let probe = probe_drive_with(req.backend, req.tool_path, req.device, runner, control)
            .map_err(|e| e.to_string())?;

        let encrypted =
            req.encrypted || detect_encrypted_write(&probe.safety, req.firmware_path, req.recover);

        let prepared = prepare_firmware_op(FirmwareOpRequest {
            backend: req.backend,
            tool_path: req.tool_path,
            sdf_path: req.sdf_path,
            device: req.device,
            drive_is_mt1959: probe.safety.mt1959,
            firmware_path: req.firmware_path,
            encrypted,
            include_boot_loader: req.include_boot_loader,
            recover: req.recover,
            wrong_firmware: req.wrong_firmware,
            recovery_token: req.recovery_token,
            confirm: req.confirm,
        })?;

        Ok(Self {
            probe,
            plan: prepared.plan,
            would_execute: prepared.would_execute,
        })
    }

    pub fn execute(&self) -> Result<(), String> {
        self.execute_with(&NativeRunner)
    }

    pub fn execute_with(&self, runner: &dyn ProcessRunner) -> Result<(), String> {
        let plan = self.plan.as_ref().ok_or("no plan to execute")?;
        execute_command_with(runner, &plan.command).map_err(|e| e.to_string())
    }
}

fn plan_error_string(e: PlanError) -> String {
    format!("cannot plan flash: {e}")
}

/// Best-effort encrypted-write detection for the probe-then-plan path: the
/// drive's probed firmware state OR the firmware file's own encryption
/// resolved by [`firmware_db::identify`]. An unreadable file detects nothing.
fn detect_encrypted_write(safety: &DriveSafety, firmware_path: &str, recover: bool) -> bool {
    if recover {
        return false;
    }
    let firmware_file_encrypted = std::fs::read(firmware_path)
        .ok()
        .and_then(|data| crate::firmware_db::identify(&data).encrypted);
    crate::firmware_db::encrypted_write_required(safety.encrypted_firmware, firmware_file_encrypted)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive::DriveIdentity;
    use crate::test_support::FakeRunner;

    fn test_identity() -> DriveIdentity {
        DriveIdentity {
            vendor: "HL-DT-ST".into(),
            model: "BU40N".into(),
            revision: "1.03".into(),
        }
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
    fn probe_drive_parses_mock_output() {
        let runner = stdout_runner(
            "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n",
        );
        let probe = probe_drive_with(
            crate::command::Backend::SdfTool,
            "/mock/sdftool",
            "/dev/sr0",
            &runner,
            None,
        )
        .expect("probe");
        assert!(probe.safety.mt1959);
        assert_eq!(probe.identity.vendor, "HL-DT-ST");
        assert_eq!(probe.identity.model, "BU40N");
    }

    #[test]
    fn probe_drive_native_wrapper_maps_spawn_error() {
        let err = probe_drive(
            crate::command::Backend::SdfTool,
            "/nonexistent/sdftool_coverage_probe_xyz",
            "/dev/sr0",
        )
        .unwrap_err();
        assert!(
            err.contains("cannot probe") || err.contains("failed"),
            "err={err}"
        );
    }

    #[test]
    fn run_list_backend_success() {
        let runner = stdout_runner("0:/dev/sr0 HL-DT-ST BU40N 1.03\n");
        let out = run_list_backend_with(
            crate::command::Backend::SdfTool,
            "/mock/sdftool",
            &runner,
            None,
        )
        .expect("list");
        assert!(out.stdout.contains("/dev/sr0") || out.combined().contains("/dev/sr0"));
    }

    #[test]
    fn run_dump_with_mock_tool() {
        let runner = stdout_runner("");
        let out_dir = std::env::temp_dir().join(format!(
            "sdf_flash_dump_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&out_dir).unwrap();
        let plan = plan_read(
            crate::command::Backend::SdfTool,
            "/mock/sdftool",
            "",
            "/dev/sr0",
            &out_dir.to_string_lossy(),
            true,
        )
        .expect("plan dump");
        execute_command_with(&runner, &plan.command).expect("dump");
        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn run_dump_native_wrapper_maps_spawn_error() {
        let out_dir =
            std::env::temp_dir().join(format!("sdf_flash_dump_err_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&out_dir);
        let err = run_dump(
            crate::command::Backend::SdfTool,
            "/nonexistent/sdftool_coverage_dump_xyz",
            "",
            "/dev/sr0",
            &out_dir.to_string_lossy(),
        )
        .unwrap_err();
        let _ = std::fs::remove_dir_all(&out_dir);
        assert!(err.contains("failed") || !err.is_empty(), "err={err}");
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
    fn plan_error_string_formats_plan_errors() {
        let msg = plan_error_string(PlanError::MissingFirmware);
        assert!(msg.starts_with("cannot plan flash:"));
        assert!(msg.contains("firmware"));
    }

    #[test]
    fn flash_session_execute_without_plan() {
        let session = FlashSession {
            probe: ProbeResult {
                safety: crate::drive::DriveSafety {
                    mt1959: true,
                    mt1939: false,
                    encrypted_firmware: false,
                    firmware_date_prefix: None,
                    mtk_mode: None,
                    libredrive: crate::drive::LibreDriveStatus::Unknown,
                    sdf_version: None,
                },
                identity: test_identity(),
                output: String::new(),
            },
            plan: None,
            would_execute: false,
        };
        let err = session.execute().unwrap_err();
        assert!(err.contains("no plan to execute"));
    }

    #[test]
    fn flash_session_prepare_rejects_encrypted_and_bootloader() {
        let runner = stdout_runner(mt1959_probe_stdout());
        let err = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/tmp/fw.bin",
                encrypted: true,
                include_boot_loader: true,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::None,
            },
            &runner,
            None,
        )
        .unwrap_err();
        assert!(err.contains("cannot be combined"));
    }

    #[test]
    fn backend_op_error_display() {
        assert_eq!(BackendOpError::Failed("boom".into()).to_string(), "boom");
        assert_eq!(BackendOpError::Cancelled.to_string(), "operation cancelled");
        assert!(BackendOpError::NeedsForceKill
            .to_string()
            .contains("force kill"));
    }

    #[test]
    fn flash_confirm_flag_and_typed() {
        assert!(!FlashConfirm::None.is_confirmed("/dev/sr0"));
        assert!(FlashConfirm::Flag.is_confirmed("/dev/sr0"));
        assert!(FlashConfirm::Typed("FLASH /dev/sr0".into()).is_confirmed("/dev/sr0"));
        assert!(!FlashConfirm::Typed("nope".into()).is_confirmed("/dev/sr0"));
        assert_eq!(FlashConfirm::None.plan_confirmation("/dev/sr0"), "");
        assert_eq!(
            FlashConfirm::Flag.plan_confirmation("/dev/sr0"),
            command::required_flash_confirmation("/dev/sr0")
        );
        assert_eq!(
            FlashConfirm::Typed("FLASH /dev/sr0".into()).plan_confirmation("/dev/sr0"),
            "FLASH /dev/sr0"
        );
    }

    fn stdout_runner(stdout: &str) -> FakeRunner {
        FakeRunner::stdout(stdout)
    }

    fn mt1959_probe_stdout() -> &'static str {
        "Drive platform: MT1959\nVendor: HL-DT-ST\nProduct: BU40N\nRevision: 1.03\n"
    }

    #[test]
    fn probe_drive_with_empty_failure_output() {
        let runner = FakeRunner::exit(1, "", "");
        let err = probe_drive_with(
            crate::command::Backend::SdfTool,
            "/usr/bin/sdftool",
            "/dev/sr0",
            &runner,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, BackendOpError::Failed(ref m) if m.contains("probe command failed")));
    }

    #[test]
    fn probe_drive_with_cancelled_and_force_kill() {
        let cancelled = FakeRunner::cancelled();
        assert!(matches!(
            probe_drive_with(
                crate::command::Backend::SdfTool,
                "/usr/bin/sdftool",
                "/dev/sr0",
                &cancelled,
                None,
            ),
            Err(BackendOpError::Cancelled)
        ));
        let force = FakeRunner::needs_force_kill();
        assert!(matches!(
            probe_drive_with(
                crate::command::Backend::SdfTool,
                "/usr/bin/sdftool",
                "/dev/sr0",
                &force,
                None,
            ),
            Err(BackendOpError::NeedsForceKill)
        ));
    }

    #[test]
    fn execute_command_with_cancel_outcomes() {
        let cmd = crate::command::Command {
            program: "echo".into(),
            args: vec![],
        };
        let cancelled = FakeRunner::cancelled();
        assert!(execute_command_with(&cancelled, &cmd)
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        let force = FakeRunner::needs_force_kill();
        assert!(execute_command_with(&force, &cmd)
            .unwrap_err()
            .to_string()
            .contains("force kill"));
        let failed = FakeRunner::exit(1, "nope", "");
        assert_eq!(
            execute_command_with(&failed, &cmd).unwrap_err().to_string(),
            "nope"
        );
    }

    #[test]
    fn run_list_backend_with_typed_errors() {
        let cancelled = FakeRunner::cancelled();
        assert!(matches!(
            run_list_backend_with(
                crate::command::Backend::SdfTool,
                "/usr/bin/sdftool",
                &cancelled,
                None,
            ),
            Err(BackendOpError::Cancelled)
        ));
        let force = FakeRunner::needs_force_kill();
        assert!(matches!(
            run_list_backend_with(
                crate::command::Backend::SdfTool,
                "/usr/bin/sdftool",
                &force,
                None,
            ),
            Err(BackendOpError::NeedsForceKill)
        ));
        let spawn_err = FakeRunner::spawn_error("boom");
        assert!(matches!(
            run_list_backend_with(
                crate::command::Backend::SdfTool,
                "/usr/bin/sdftool",
                &spawn_err,
                None,
            ),
            Err(BackendOpError::Failed(ref m)) if m == "boom"
        ));
        let _ = spawn_err.run_command_streaming("x", &[], &|_| {}, None);
    }

    #[test]
    fn prepare_firmware_op_requires_confirm() {
        let prepared = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "/tmp/fw.bin",
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: FlashConfirm::None,
        })
        .expect("prepare");
        assert!(!prepared.would_execute);
        assert!(prepared.plan.is_none());
    }

    #[test]
    fn prepare_firmware_op_with_flag_plans() {
        let prepared = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "/tmp/fw.bin",
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: FlashConfirm::Flag,
        })
        .expect("prepare");
        assert!(prepared.would_execute);
        assert!(prepared.plan.is_some());
    }

    #[test]
    fn probe_from_output_detects_libredrive() {
        let probe = probe_from_output(
            "/dev/sr0",
            "SDF.bin version: 0x00A6\n\nDrive Specific SDF present\n",
        );
        assert_eq!(
            probe.safety.libredrive,
            crate::drive::LibreDriveStatus::Enabled
        );
        assert_eq!(probe.safety.sdf_version.as_deref(), Some("0x00A6"));
    }

    #[test]
    fn probe_from_output_no_libredrive_when_absent() {
        let probe = probe_from_output(
            "/dev/sr0",
            "SDF.bin version: 0x00A6\n\nDrive Specific SDF not present\n",
        );
        assert!(!probe.safety.libredrive.is_enabled());
    }

    #[test]
    fn prepare_firmware_op_rejects_non_mt1959() {
        let err = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: false,
            firmware_path: "/tmp/fw.bin",
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: FlashConfirm::Flag,
        })
        .unwrap_err();
        assert!(err.contains("MT1959"));
    }

    #[test]
    fn prepare_firmware_op_dry_run_validates_missing_firmware() {
        let err = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "",
            encrypted: false,
            include_boot_loader: false,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: FlashConfirm::None,
        })
        .unwrap_err();
        assert!(err.contains("firmware path is required"));
    }

    #[test]
    fn prepare_firmware_op_dry_run_validates_recovery_token() {
        let err = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "/tmp/fw.bin",
            encrypted: false,
            include_boot_loader: false,
            recover: true,
            wrong_firmware: None,
            recovery_token: Some("short"),
            confirm: FlashConfirm::None,
        })
        .unwrap_err();
        assert!(err.contains("16 printable ASCII"));
    }

    #[test]
    fn prepare_firmware_op_mode_conflict() {
        let err = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "/tmp/fw.bin",
            encrypted: true,
            include_boot_loader: true,
            recover: false,
            wrong_firmware: None,
            recovery_token: None,
            confirm: FlashConfirm::Flag,
        })
        .unwrap_err();
        assert!(err.contains("cannot be combined"));
    }

    #[test]
    fn prepare_firmware_op_recover_with_token() {
        let prepared = prepare_firmware_op(FirmwareOpRequest {
            backend: crate::command::Backend::SdfTool,
            tool_path: "/usr/bin/sdftool",
            sdf_path: "",
            device: "/dev/sr0",
            drive_is_mt1959: true,
            firmware_path: "/tmp/fw.bin",
            encrypted: false,
            include_boot_loader: false,
            recover: true,
            wrong_firmware: None,
            recovery_token: Some("ABCDEFGHIJKLMNOP"),
            confirm: FlashConfirm::Flag,
        })
        .expect("recover prepare");
        assert!(prepared.would_execute);
        assert!(prepared.plan.is_some());
    }

    #[test]
    fn flash_session_prepare_respects_confirm() {
        let runner = stdout_runner(mt1959_probe_stdout());
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/tmp/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("prepare should succeed");
        assert!(session.probe.safety.mt1959);
        assert!(session.would_execute);
        assert!(session.plan.is_some());
    }

    #[test]
    fn flash_session_prepare_rejects_non_mt1959() {
        let runner = stdout_runner("Vendor: Old\nProduct: Drive\n");
        let err = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/tmp/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::None,
            },
            &runner,
            None,
        )
        .unwrap_err();
        assert!(err.contains("MT1959"));
    }

    fn temp_firmware_file(name: &str, date_stamp: &[u8]) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("sdf_flash_enc_{}_{}", std::process::id(), name));
        let mut data = vec![0u8; 4096];
        data[100..100 + date_stamp.len()].copy_from_slice(date_stamp);
        std::fs::write(&path, &data).unwrap();
        path
    }

    #[test]
    fn flash_session_detects_encrypted_firmware_file() {
        let fw = temp_firmware_file("file_enc", b"212005070917");
        let runner = stdout_runner(mt1959_probe_stdout());
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: &fw.to_string_lossy(),
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("prepare");
        let _ = std::fs::remove_file(&fw);
        let plan = session.plan.expect("plan");
        assert!(plan.command.args.contains(&"enc".to_string()));
    }

    #[test]
    fn flash_session_detects_encrypted_drive_from_probe() {
        let runner = stdout_runner(
            "Drive platform: MT1959\nfirmware 212005070917\nVendor: HL-DT-ST\nProduct: BU40N\n",
        );
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/nonexistent/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("prepare");
        assert!(session.probe.safety.encrypted_firmware);
        let plan = session.plan.expect("plan");
        assert!(plan.command.args.contains(&"enc".to_string()));
    }

    #[test]
    fn flash_session_unreadable_firmware_stays_plain() {
        let runner = stdout_runner(mt1959_probe_stdout());
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/nonexistent/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("prepare");
        let plan = session.plan.expect("plan");
        assert!(!plan.command.args.contains(&"enc".to_string()));
    }

    #[test]
    fn flash_session_encrypted_file_conflicts_with_bootloader() {
        let fw = temp_firmware_file("file_enc_boot", b"212005070917");
        let runner = stdout_runner(mt1959_probe_stdout());
        let err = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: &fw.to_string_lossy(),
                encrypted: false,
                include_boot_loader: true,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .unwrap_err();
        let _ = std::fs::remove_file(&fw);
        assert!(err.contains("cannot be combined"));
    }

    #[test]
    fn run_list_backend_failure() {
        let runner = FakeRunner::exit(1, "", "mock fail");
        let err = run_list_backend_with(
            crate::command::Backend::SdfTool,
            "/mock/sdftool",
            &runner,
            None,
        )
        .err()
        .expect("list should fail");
        assert!(matches!(err, BackendOpError::Failed(_)));
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
    fn flash_session_prepare_recover_operation() {
        let runner = stdout_runner(mt1959_probe_stdout());
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/tmp/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: true,
                wrong_firmware: None,
                recovery_token: Some("ABCDEFGHIJKLMNOP"),
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("recover prepare");
        assert!(session.plan.is_some());
        assert!(session.would_execute);
    }

    #[test]
    fn flash_session_execute_with_plan() {
        let runner = stdout_runner(mt1959_probe_stdout());
        let session = FlashSession::prepare_with(
            FlashSessionRequest {
                backend: crate::command::Backend::SdfTool,
                tool_path: "/mock/sdftool",
                sdf_path: "",
                device: "/dev/sr0",
                firmware_path: "/tmp/fw.bin",
                encrypted: false,
                include_boot_loader: false,
                recover: false,
                wrong_firmware: None,
                recovery_token: None,
                confirm: FlashConfirm::Flag,
            },
            &runner,
            None,
        )
        .expect("prepare");
        session.execute_with(&runner).expect("execute");
    }

    #[test]
    fn parses_prgv() {
        assert_eq!(parse_progress_percent("PRGV:50,100,0"), Some(50.0));
        assert_eq!(parse_progress_percent("PRGV:100,100,0"), Some(100.0));
    }

    #[test]
    fn parses_percent_suffix() {
        assert_eq!(parse_progress_percent("Progress: 42%"), Some(42.0));
        assert_eq!(
            parse_progress_percent("100% Operation finished"),
            Some(100.0)
        );
    }

    #[test]
    fn ignores_non_progress() {
        assert_eq!(parse_progress_percent("MSG:1005,0,2"), None);
    }

    #[test]
    fn prgv_format() {
        assert_eq!(parse_progress_percent("PRGV:50,100"), Some(50.0));
        assert_eq!(parse_progress_percent("PRGV:0,100"), Some(0.0));
        assert_eq!(parse_progress_percent("PRGV:100,100"), Some(100.0));
    }

    #[test]
    fn prgv_format_partial() {
        let p = parse_progress_percent("PRGV:33,99").unwrap();
        assert!((p - 33.33).abs() < 0.1);
    }

    #[test]
    fn percent_format() {
        assert_eq!(parse_progress_percent("42%"), Some(42.0));
        assert_eq!(parse_progress_percent("Progress: 75%"), Some(75.0));
        assert_eq!(parse_progress_percent("100%"), Some(100.0));
    }

    #[test]
    fn no_progress() {
        assert_eq!(parse_progress_percent("some random text"), None);
        assert_eq!(parse_progress_percent(""), None);
    }

    #[test]
    fn prgv_zero_total_clamps() {
        assert_eq!(parse_progress_percent("PRGV:0,0"), None);
    }

    #[test]
    fn parse_progress_percent_prgv_partial() {
        assert_eq!(parse_progress_percent("PRGV:50"), None);
    }

    #[test]
    fn parse_progress_percent_prgv_over_100() {
        let p = parse_progress_percent("PRGV:200,100").unwrap();
        assert!((p - 100.0).abs() < 0.01);
    }

    #[test]
    fn parse_progress_percent_prgv_negative() {
        let p = parse_progress_percent("PRGV:-10,100").unwrap();
        assert!((p - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_progress_percent_multiple_percent_signs() {
        assert_eq!(parse_progress_percent("10% then 90%"), Some(90.0));
    }

    #[test]
    fn parse_progress_percent_no_digits_before_percent() {
        assert_eq!(parse_progress_percent("%"), None);
    }
}
