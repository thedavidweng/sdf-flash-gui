use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    SdfTool,
    MakeMkvCon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Read {
        output_dir: String,
    },
    Write {
        firmware_path: String,
        encrypted: bool,
        include_boot_loader: bool,
    },
    Recover {
        firmware_path: String,
        recovery_boot_token: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRequest {
    pub backend: Backend,
    pub tool_path: String,
    pub sdf_path: String,
    pub drive: String,
    pub drive_is_mt1959: bool,
    pub confirmation: String,
    pub operation: Operation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub command: Command,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("SDFtool executable path is required")]
    MissingToolPath,

    #[error("drive must be selected before planning a firmware operation")]
    MissingDrive,

    #[error("selected drive is not an MT1959 drive")]
    UnsupportedPlatform,

    #[error("firmware path is required for this operation")]
    MissingFirmware,

    #[error("output directory is required for firmware dump")]
    MissingOutputDirectory,

    #[error(
        "recover mode requires a 16-byte boot token from the currently installed wrong firmware"
    )]
    MissingRecoveryBootToken,

    #[error("recover mode boot token must be exactly 16 printable ASCII bytes")]
    InvalidRecoveryBootToken,

    #[error("confirmation mismatch: type '{expected}' to continue")]
    ConfirmationMismatch { expected: String },

    #[error("encrypted rawflash and boot-loader rawflash cannot be combined")]
    ConflictingWriteModes,
}

pub fn required_flash_confirmation(drive: &str) -> String {
    format!("FLASH {}", drive.trim())
}

/// True when encrypted rawflash and full boot-loader modes are both selected.
pub fn write_modes_conflict(encrypted: bool, include_boot_loader: bool) -> bool {
    encrypted && include_boot_loader
}

/// True when the typed confirmation matches the required `FLASH <device>` string.
pub fn confirmation_matches(device: &str, typed: &str) -> bool {
    typed.trim() == required_flash_confirmation(device)
}

fn require_confirmation(drive: &str, typed: &str) -> Result<(), PlanError> {
    let expected = required_flash_confirmation(drive);
    if typed.trim() != expected {
        return Err(PlanError::ConfirmationMismatch { expected });
    }
    Ok(())
}

pub fn plan_command(request: PlanRequest) -> Result<Plan, PlanError> {
    let tool_path = request.tool_path.trim();
    if tool_path.is_empty() {
        return Err(PlanError::MissingToolPath);
    }

    let drive = request.drive.trim();
    if drive.is_empty() {
        return Err(PlanError::MissingDrive);
    }

    if !request.drive_is_mt1959 {
        return Err(PlanError::UnsupportedPlatform);
    }

    let mut args = backend_prefix(request.backend);

    let sdf_path = request.sdf_path.trim();
    if !sdf_path.is_empty() {
        args.extend(["-f".into(), sdf_path.into()]);
    }

    match request.operation {
        Operation::Read { output_dir } => {
            let output_dir = output_dir.trim();
            if output_dir.is_empty() {
                return Err(PlanError::MissingOutputDirectory);
            }
            args.extend([
                "-d".into(),
                drive.into(),
                "dump".into(),
                "auto".into(),
                "-o".into(),
                output_dir.into(),
            ]);
        }
        Operation::Write {
            firmware_path,
            encrypted,
            include_boot_loader,
        } => {
            let firmware_path = firmware_path.trim();
            if firmware_path.is_empty() {
                return Err(PlanError::MissingFirmware);
            }
            if write_modes_conflict(encrypted, include_boot_loader) {
                return Err(PlanError::ConflictingWriteModes);
            }
            require_confirmation(drive, &request.confirmation)?;
            args.extend([
                "--all-yes".into(),
                "-d".into(),
                drive.into(),
                "rawflash".into(),
            ]);
            if encrypted {
                args.push("enc".into());
            } else if include_boot_loader {
                args.push("full".into());
            }
            args.extend(["-i".into(), firmware_path.into()]);
        }
        Operation::Recover {
            firmware_path,
            recovery_boot_token,
        } => {
            let firmware_path = firmware_path.trim();
            if firmware_path.is_empty() {
                return Err(PlanError::MissingFirmware);
            }
            validate_recovery_boot_token(&recovery_boot_token)?;
            require_confirmation(drive, &request.confirmation)?;
            args.extend([
                "--all-yes".into(),
                "-d".into(),
                drive.into(),
                "rawflash".into(),
                format!("main,nowait,nocheck,boot={recovery_boot_token}"),
                "-i".into(),
                firmware_path.into(),
            ]);
        }
    }

    Ok(Plan {
        command: Command {
            program: tool_path.into(),
            args,
        },
    })
}

pub fn plan_drive_list(backend: Backend, tool_path: &str) -> Command {
    let mut args = backend_prefix(backend);
    args.push("-l".into());
    Command {
        program: tool_path.into(),
        args,
    }
}

pub fn plan_drive_info(backend: Backend, tool_path: &str, drive: &str) -> Command {
    let mut args = backend_prefix(backend);
    args.extend(["-d".into(), drive.into(), "--info".into()]);
    Command {
        program: tool_path.into(),
        args,
    }
}

fn backend_prefix(backend: Backend) -> Vec<String> {
    match backend {
        Backend::SdfTool => Vec::new(),
        Backend::MakeMkvCon => vec!["f".into()],
    }
}

pub fn extract_recovery_boot_token(firmware: &[u8]) -> Result<String, PlanError> {
    let start = 12_288;
    let end = start + 16;
    if firmware.len() < end {
        return Err(PlanError::MissingRecoveryBootToken);
    }
    let token = std::str::from_utf8(&firmware[start..end])
        .map_err(|_| PlanError::InvalidRecoveryBootToken)?;
    validate_recovery_boot_token(token)?;
    Ok(token.into())
}

fn validate_recovery_boot_token(token: &str) -> Result<(), PlanError> {
    if token.is_empty() {
        return Err(PlanError::MissingRecoveryBootToken);
    }
    if token.len() != 16 || !token.as_bytes().iter().all(u8::is_ascii_graphic) {
        return Err(PlanError::InvalidRecoveryBootToken);
    }
    Ok(())
}

/// Format a command for display in the log.
pub fn format_command(cmd: &Command) -> String {
    std::iter::once(cmd.program.as_str())
        .chain(cmd.args.iter().map(String::as_str))
        .map(|s| {
            if s.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(b, b'.' | b'_' | b'-' | b':' | b'/' | b'\\' | b'=')
            }) {
                s.to_string()
            } else {
                format!("\"{}\"", s.replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request(operation: Operation) -> PlanRequest {
        PlanRequest {
            backend: Backend::SdfTool,
            tool_path: "sdftool64.exe".into(),
            sdf_path: String::new(),
            drive: "H:".into(),
            drive_is_mt1959: true,
            confirmation: "FLASH H:".into(),
            operation,
        }
    }

    #[test]
    fn plans_plain_rawflash() {
        let plan = plan_command(base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: false,
            include_boot_loader: false,
        }))
        .unwrap();
        assert_eq!(plan.command.program, "sdftool64.exe");
        assert_eq!(
            plan.command.args,
            ["--all-yes", "-d", "H:", "rawflash", "-i", "fw.bin"]
        );
    }

    #[test]
    fn plans_encrypted_rawflash() {
        let plan = plan_command(base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: true,
            include_boot_loader: false,
        }))
        .unwrap();
        assert_eq!(
            plan.command.args,
            ["--all-yes", "-d", "H:", "rawflash", "enc", "-i", "fw.bin"]
        );
    }

    #[test]
    fn makemkvcon_backend_prefixes_f() {
        let mut req = base_request(Operation::Read {
            output_dir: "/tmp/out".into(),
        });
        req.backend = Backend::MakeMkvCon;
        req.tool_path = "makemkvcon64.exe".into();
        req.confirmation = String::new();
        let plan = plan_command(req).unwrap();
        assert_eq!(
            plan.command.args,
            ["f", "-d", "H:", "dump", "auto", "-o", "/tmp/out"]
        );
    }

    #[test]
    fn plans_with_sdf_path_injects_f_flag() {
        let mut req = base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: false,
            include_boot_loader: false,
        });
        req.sdf_path = "/path/to/sdf.bin".into();
        let plan = plan_command(req).unwrap();
        assert_eq!(
            plan.command.args,
            [
                "-f",
                "/path/to/sdf.bin",
                "--all-yes",
                "-d",
                "H:",
                "rawflash",
                "-i",
                "fw.bin"
            ]
        );
    }

    #[test]
    fn plans_with_sdf_path_makemkvcon() {
        let mut req = base_request(Operation::Read {
            output_dir: "/tmp/out".into(),
        });
        req.backend = Backend::MakeMkvCon;
        req.tool_path = "makemkvcon".into();
        req.sdf_path = "./sdf.bin".into();
        req.confirmation = String::new();
        let plan = plan_command(req).unwrap();
        assert_eq!(
            plan.command.args,
            [
                "f",
                "-f",
                "./sdf.bin",
                "-d",
                "H:",
                "dump",
                "auto",
                "-o",
                "/tmp/out"
            ]
        );
    }

    #[test]
    fn blocks_non_mt1959() {
        let mut req = base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: false,
            include_boot_loader: false,
        });
        req.drive_is_mt1959 = false;
        assert!(matches!(
            plan_command(req),
            Err(PlanError::UnsupportedPlatform)
        ));
    }

    #[test]
    fn plans_read_operation() {
        let plan = plan_command(PlanRequest {
            confirmation: String::new(),
            ..base_request(Operation::Read {
                output_dir: "/tmp/out".into(),
            })
        })
        .unwrap();
        assert_eq!(
            plan.command.args,
            ["-d", "H:", "dump", "auto", "-o", "/tmp/out"]
        );
    }

    #[test]
    fn plans_full_boot_loader() {
        let plan = plan_command(base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: false,
            include_boot_loader: true,
        }))
        .unwrap();
        assert_eq!(
            plan.command.args,
            ["--all-yes", "-d", "H:", "rawflash", "full", "-i", "fw.bin"]
        );
    }

    #[test]
    fn plans_recover_operation() {
        let plan = plan_command(base_request(Operation::Recover {
            firmware_path: "fw.bin".into(),
            recovery_boot_token: "ABCDEFGHIJKLMNOP".into(),
        }))
        .unwrap();
        assert_eq!(
            plan.command.args,
            [
                "--all-yes",
                "-d",
                "H:",
                "rawflash",
                "main,nowait,nocheck,boot=ABCDEFGHIJKLMNOP",
                "-i",
                "fw.bin"
            ]
        );
    }

    #[test]
    fn rejects_conflicting_write_modes() {
        let result = plan_command(base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: true,
            include_boot_loader: true,
        }));
        assert_eq!(result, Err(PlanError::ConflictingWriteModes));
    }

    #[test]
    fn rejects_empty_tool_path() {
        let mut req = base_request(Operation::Read {
            output_dir: "/tmp".into(),
        });
        req.tool_path = String::new();
        req.confirmation = String::new();
        assert_eq!(plan_command(req), Err(PlanError::MissingToolPath));
    }

    #[test]
    fn rejects_empty_drive() {
        let mut req = base_request(Operation::Read {
            output_dir: "/tmp".into(),
        });
        req.drive = String::new();
        req.confirmation = String::new();
        assert_eq!(plan_command(req), Err(PlanError::MissingDrive));
    }

    #[test]
    fn rejects_empty_firmware_path() {
        let result = plan_command(base_request(Operation::Write {
            firmware_path: String::new(),
            encrypted: false,
            include_boot_loader: false,
        }));
        assert_eq!(result, Err(PlanError::MissingFirmware));
    }

    #[test]
    fn rejects_empty_output_dir() {
        let result = plan_command(PlanRequest {
            confirmation: String::new(),
            ..base_request(Operation::Read {
                output_dir: String::new(),
            })
        });
        assert_eq!(result, Err(PlanError::MissingOutputDirectory));
    }

    #[test]
    fn rejects_confirmation_mismatch() {
        let mut req = base_request(Operation::Write {
            firmware_path: "fw.bin".into(),
            encrypted: false,
            include_boot_loader: false,
        });
        req.confirmation = "WRONG".into();
        assert!(matches!(
            plan_command(req),
            Err(PlanError::ConfirmationMismatch { .. })
        ));
    }

    #[test]
    fn rejects_invalid_boot_token() {
        let result = plan_command(base_request(Operation::Recover {
            firmware_path: "fw.bin".into(),
            recovery_boot_token: "short".into(),
        }));
        assert_eq!(result, Err(PlanError::InvalidRecoveryBootToken));
    }

    #[test]
    fn extract_recovery_boot_token_valid() {
        let mut firmware = vec![0u8; 12_288 + 16];
        firmware[12_288..12_304].copy_from_slice(b"ABCDEFGHIJKLMNOP");
        let token = extract_recovery_boot_token(&firmware).unwrap();
        assert_eq!(token, "ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn extract_recovery_boot_token_too_short() {
        let firmware = vec![0u8; 100];
        assert_eq!(
            extract_recovery_boot_token(&firmware),
            Err(PlanError::MissingRecoveryBootToken)
        );
    }

    #[test]
    fn plan_drive_list_command() {
        let cmd = plan_drive_list(Backend::SdfTool, "/usr/bin/sdftool");
        assert_eq!(cmd.program, "/usr/bin/sdftool");
        assert_eq!(cmd.args, ["-l"]);
    }

    #[test]
    fn plan_drive_list_makemkvcon() {
        let cmd = plan_drive_list(Backend::MakeMkvCon, "makemkvcon");
        assert_eq!(cmd.args, ["f", "-l"]);
    }

    #[test]
    fn plan_drive_info_command() {
        let cmd = plan_drive_info(Backend::SdfTool, "/usr/bin/sdftool", "/dev/sr0");
        assert_eq!(cmd.args, ["-d", "/dev/sr0", "--info"]);
    }

    #[test]
    fn required_flash_confirmation_trims() {
        assert_eq!(required_flash_confirmation("  H:  "), "FLASH H:");
    }

    #[test]
    fn write_modes_conflict_only_when_both() {
        assert!(!write_modes_conflict(false, false));
        assert!(!write_modes_conflict(true, false));
        assert!(!write_modes_conflict(false, true));
        assert!(write_modes_conflict(true, true));
    }

    #[test]
    fn confirmation_matches_trims_and_requires_flash_device() {
        assert!(confirmation_matches("/dev/sr0", "FLASH /dev/sr0"));
        assert!(confirmation_matches("/dev/sr0", "  FLASH /dev/sr0  "));
        assert!(!confirmation_matches("/dev/sr0", "FLASH"));
        assert!(!confirmation_matches("/dev/sr0", "WRONG"));
    }

    #[test]
    fn plan_command_trims_whitespace() {
        let req = PlanRequest {
            backend: Backend::SdfTool,
            tool_path: "  sdftool64.exe  ".into(),
            sdf_path: String::new(),
            drive: "  H:  ".into(),
            drive_is_mt1959: true,
            confirmation: "FLASH H:".into(),
            operation: Operation::Read {
                output_dir: "  /tmp/out  ".into(),
            },
        };
        let plan = plan_command(req).unwrap();
        assert_eq!(plan.command.program, "sdftool64.exe");
        assert_eq!(plan.command.args[1], "H:");
        assert_eq!(plan.command.args[5], "/tmp/out");
    }

    #[test]
    fn extract_recovery_boot_token_invalid_utf8() {
        let mut firmware = vec![0u8; 12_288 + 16];
        firmware[12_288..12_304].copy_from_slice(&[0xFF; 16]);
        assert!(matches!(
            extract_recovery_boot_token(&firmware),
            Err(PlanError::InvalidRecoveryBootToken)
        ));
    }

    #[test]
    fn extract_recovery_boot_token_non_printable() {
        let mut firmware = vec![0u8; 12_288 + 16];
        firmware[12_288..12_304].copy_from_slice(b"ABCDEFGH IJKLMNO");
        assert!(matches!(
            extract_recovery_boot_token(&firmware),
            Err(PlanError::InvalidRecoveryBootToken)
        ));
    }

    #[test]
    fn plan_drive_info_makemkvcon() {
        let cmd = plan_drive_info(Backend::MakeMkvCon, "makemkvcon", "/dev/sr0");
        assert_eq!(cmd.program, "makemkvcon");
        assert_eq!(cmd.args, ["f", "-d", "/dev/sr0", "--info"]);
    }

    #[test]
    fn plan_recover_empty_firmware() {
        let result = plan_command(base_request(Operation::Recover {
            firmware_path: String::new(),
            recovery_boot_token: "ABCDEFGHIJKLMNOP".into(),
        }));
        assert_eq!(result, Err(PlanError::MissingFirmware));
    }

    #[test]
    fn plan_recover_empty_token() {
        let result = plan_command(base_request(Operation::Recover {
            firmware_path: "fw.bin".into(),
            recovery_boot_token: String::new(),
        }));
        assert_eq!(result, Err(PlanError::MissingRecoveryBootToken));
    }

    #[test]
    fn extract_recovery_boot_token_with_ascii_digits() {
        let mut firmware = vec![0u8; 12_288 + 16];
        firmware[12_288..12_304].copy_from_slice(b"ABCDEFGH12345678");
        let token = extract_recovery_boot_token(&firmware).unwrap();
        assert_eq!(token, "ABCDEFGH12345678");
    }

    #[test]
    fn format_command_simple() {
        let cmd = crate::command::Command {
            program: "/usr/bin/sdftool".into(),
            args: vec!["-d".into(), "/dev/sr0".into(), "--info".into()],
        };
        assert_eq!(format_command(&cmd), "/usr/bin/sdftool -d /dev/sr0 --info");
    }

    #[test]
    fn format_command_quoted_arg() {
        let cmd = crate::command::Command {
            program: "/usr/bin/sdftool".into(),
            args: vec!["-d".into(), "path with spaces".into()],
        };
        let result = format_command(&cmd);
        assert!(result.contains("\"path with spaces\""));
    }
}
