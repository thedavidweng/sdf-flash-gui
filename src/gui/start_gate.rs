//! Structured start preconditions for write/read/recover.
//!
//! Non-UI module relative to views: returns reasons that ops maps through i18n.
//! Single place for "may Start enable" so can_start and tooltips cannot drift.
//!
//! Plan-level rules (mt1959, mode conflict, confirmation, recover token,
//! firmware path) are delegated to [`command::plan_command`] — the same deep
//! module that `prepare_firmware_op` and the CLI use. The gate maps
//! `PlanError` variants to `StartBlock` reasons. GUI-only pre-checks (busy,
//! probing, cross-flash confirmation, path validation, firmware loaded) stay
//! here because they are UI state, not plan rules.

use crate::command::{self, Backend, Operation, PlanError, PlanRequest};

use super::validation::{validate_sdf_path, validate_tool_path};
use crate::i18n::Language;

/// Why Start is disabled. `None` from [`evaluate`] means start is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartBlock {
    Busy,
    Probing,
    NoDrive,
    NotMt1959 {
        is_mt1939: bool,
    },
    InvalidToolPath(String),
    InvalidSdfPath(String),
    NoFirmware,
    WriteModeConflict,
    CrossFlashNotConfirmed,
    /// Typed confirmation or recover boot token missing/wrong.
    NeedConfirmation,
}

/// Snapshot of fields needed to evaluate the gate (no AppState dependency).
#[derive(Debug, Clone)]
pub struct StartGateInput<'a> {
    pub busy: bool,
    pub probing: bool,
    pub has_drive: bool,
    pub drive_mt1959: bool,
    pub drive_mt1939: bool,
    pub tool_path: &'a str,
    pub backend: Backend,
    pub sdf_path: &'a str,
    /// Language for path-validation error fragments (GUI resolved lang).
    pub lang: Language,
    pub mode: StartMode,
    pub firmware_path: &'a str,
    pub has_firmware_data: bool,
    pub encrypted_write: bool,
    pub include_boot_loader: bool,
    pub cross_flash_required: bool,
    pub cross_flash_confirmed: bool,
    pub confirmation: &'a str,
    pub device: &'a str,
    pub recovery_token: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMode {
    Read,
    Write,
    Recover,
}

/// Evaluate whether an operation may start. `None` = allowed.
pub fn evaluate(input: &StartGateInput<'_>) -> Option<StartBlock> {
    if input.busy {
        return Some(StartBlock::Busy);
    }
    if input.probing {
        return Some(StartBlock::Probing);
    }
    if !input.has_drive {
        return Some(StartBlock::NoDrive);
    }
    if !input.drive_mt1959 {
        return Some(StartBlock::NotMt1959 {
            is_mt1939: input.drive_mt1939,
        });
    }
    if let Err(e) = validate_tool_path(input.tool_path, input.backend, input.lang) {
        return Some(StartBlock::InvalidToolPath(e));
    }
    if let Err(e) = validate_sdf_path(input.sdf_path, input.lang) {
        return Some(StartBlock::InvalidSdfPath(e));
    }
    if input.cross_flash_required && !input.cross_flash_confirmed {
        return Some(StartBlock::CrossFlashNotConfirmed);
    }
    match input.mode {
        StartMode::Read => None,
        StartMode::Write => {
            if !input.has_firmware_data {
                return Some(StartBlock::NoFirmware);
            }
            plan_block(
                input,
                Operation::Write {
                    firmware_path: input.firmware_path.to_string(),
                    encrypted: input.encrypted_write,
                    include_boot_loader: input.include_boot_loader,
                },
            )
        }
        StartMode::Recover => plan_block(
            input,
            Operation::Recover {
                firmware_path: input.firmware_path.to_string(),
                recovery_boot_token: input.recovery_token.to_string(),
            },
        ),
    }
}

fn plan_block(input: &StartGateInput<'_>, operation: Operation) -> Option<StartBlock> {
    let request = PlanRequest {
        backend: input.backend,
        tool_path: input.tool_path.to_string(),
        sdf_path: input.sdf_path.to_string(),
        drive: input.device.to_string(),
        drive_is_mt1959: input.drive_mt1959,
        confirmation: input.confirmation.to_string(),
        operation,
    };
    match command::plan_command(request) {
        Ok(_) => None,
        Err(PlanError::MissingFirmware) => Some(StartBlock::NoFirmware),
        Err(PlanError::ConflictingWriteModes) => Some(StartBlock::WriteModeConflict),
        Err(PlanError::ConfirmationMismatch { .. }) => Some(StartBlock::NeedConfirmation),
        Err(PlanError::MissingRecoveryBootToken) | Err(PlanError::InvalidRecoveryBootToken) => {
            Some(StartBlock::NeedConfirmation)
        }
        Err(PlanError::UnsupportedPlatform) => Some(StartBlock::NotMt1959 {
            is_mt1939: input.drive_mt1939,
        }),
        Err(PlanError::MissingDrive) => Some(StartBlock::NoDrive),
        Err(PlanError::MissingToolPath) => Some(StartBlock::InvalidToolPath(String::new())),
        Err(PlanError::MissingOutputDirectory) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn with_tool(f: impl FnOnce(&str)) {
        let dir = std::env::temp_dir().join(format!(
            "sdf-gate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sdftool");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"x").unwrap();
        drop(file);
        let s = path.to_string_lossy().to_string();
        f(&s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn base<'a>(tool: &'a str) -> StartGateInput<'a> {
        StartGateInput {
            busy: false,
            probing: false,
            has_drive: true,
            drive_mt1959: true,
            drive_mt1939: false,
            tool_path: tool,
            backend: Backend::SdfTool,
            sdf_path: "",
            lang: Language::English,
            mode: StartMode::Read,
            firmware_path: "",
            has_firmware_data: false,
            encrypted_write: false,
            include_boot_loader: false,
            cross_flash_required: false,
            cross_flash_confirmed: false,
            confirmation: "",
            device: "/dev/sr0",
            recovery_token: "",
        }
    }

    #[test]
    fn read_allowed_when_idle_with_drive() {
        with_tool(|tool| {
            assert_eq!(evaluate(&base(tool)), None);
        });
    }

    #[test]
    fn busy_blocks() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.busy = true;
            assert_eq!(evaluate(&i), Some(StartBlock::Busy));
        });
    }

    #[test]
    fn write_needs_firmware_and_confirm() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.mode = StartMode::Write;
            assert_eq!(evaluate(&i), Some(StartBlock::NoFirmware));
            i.has_firmware_data = true;
            i.firmware_path = "fw.bin";
            assert_eq!(evaluate(&i), Some(StartBlock::NeedConfirmation));
            i.confirmation = "FLASH /dev/sr0";
            assert_eq!(evaluate(&i), None);
        });
    }

    #[test]
    fn write_mode_conflict() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.mode = StartMode::Write;
            i.has_firmware_data = true;
            i.firmware_path = "fw.bin";
            i.encrypted_write = true;
            i.include_boot_loader = true;
            i.confirmation = "FLASH /dev/sr0";
            assert_eq!(evaluate(&i), Some(StartBlock::WriteModeConflict));
        });
    }

    #[test]
    fn cross_flash_gate() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.mode = StartMode::Write;
            i.has_firmware_data = true;
            i.firmware_path = "fw.bin";
            i.cross_flash_required = true;
            i.cross_flash_confirmed = false;
            i.confirmation = "FLASH /dev/sr0";
            assert_eq!(evaluate(&i), Some(StartBlock::CrossFlashNotConfirmed));
            i.cross_flash_confirmed = true;
            assert_eq!(evaluate(&i), None);
        });
    }

    #[test]
    fn recover_needs_token() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.mode = StartMode::Recover;
            i.firmware_path = "fw.bin";
            i.confirmation = "FLASH /dev/sr0";
            assert_eq!(evaluate(&i), Some(StartBlock::NeedConfirmation));
            i.recovery_token = "0123456789ABCDEF";
            assert_eq!(evaluate(&i), None);
        });
    }

    #[test]
    fn recover_rejects_non_graphic_token() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.mode = StartMode::Recover;
            i.firmware_path = "fw.bin";
            i.confirmation = "FLASH /dev/sr0";
            // 16 spaces: len ok, plan would reject as non-graphic.
            i.recovery_token = "                ";
            assert_eq!(evaluate(&i), Some(StartBlock::NeedConfirmation));
            i.recovery_token = "ABCDEFGHIJKLMNO\0";
            assert_eq!(evaluate(&i), Some(StartBlock::NeedConfirmation));
        });
    }

    #[test]
    fn mt1939_flag() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.drive_mt1959 = false;
            i.drive_mt1939 = true;
            assert_eq!(
                evaluate(&i),
                Some(StartBlock::NotMt1959 { is_mt1939: true })
            );
        });
    }

    // --- plan_block defensive branch tests ---
    // These exercise the PlanError → StartBlock mapping in plan_block directly,
    // bypassing evaluate's pre-checks so the defensive branches are covered.

    #[test]
    fn plan_block_unsupported_platform_maps_to_not_mt1959() {
        with_tool(|tool| {
            let mut i = base(tool);
            i.drive_mt1959 = false;
            i.drive_mt1939 = true;
            i.mode = StartMode::Write;
            i.has_firmware_data = true;
            i.firmware_path = "fw.bin";
            i.confirmation = "FLASH /dev/sr0";
            assert_eq!(
                plan_block(
                    &i,
                    Operation::Write {
                        firmware_path: "fw.bin".into(),
                        encrypted: false,
                        include_boot_loader: false,
                    }
                ),
                Some(StartBlock::NotMt1959 { is_mt1939: true })
            );
        });
    }

    #[test]
    fn plan_block_missing_drive_maps_to_no_drive() {
        with_tool(|tool| {
            let i = StartGateInput {
                device: "",
                ..base(tool)
            };
            assert_eq!(
                plan_block(
                    &i,
                    Operation::Write {
                        firmware_path: "fw.bin".into(),
                        encrypted: false,
                        include_boot_loader: false,
                    }
                ),
                Some(StartBlock::NoDrive)
            );
        });
    }

    #[test]
    fn plan_block_missing_tool_path_maps_to_invalid_tool_path() {
        let i = StartGateInput {
            tool_path: "",
            device: "/dev/sr0",
            drive_mt1959: true,
            confirmation: "FLASH /dev/sr0",
            ..base("")
        };
        assert_eq!(
            plan_block(
                &i,
                Operation::Write {
                    firmware_path: "fw.bin".into(),
                    encrypted: false,
                    include_boot_loader: false,
                }
            ),
            Some(StartBlock::InvalidToolPath(String::new()))
        );
    }

    #[test]
    fn plan_block_missing_output_directory_maps_to_none() {
        with_tool(|tool| {
            let i = base(tool);
            assert_eq!(
                plan_block(
                    &i,
                    Operation::Read {
                        output_dir: String::new()
                    }
                ),
                None
            );
        });
    }
}
