//! Shared test double for the [`ProcessRunner`] seam.

use crate::process::{CommandOutput, CommandRunOutcome, OperationControl, ProcessRunner};

fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

enum FakeOutcome {
    Exit {
        code: i32,
        stdout: String,
        stderr: String,
    },
    Cancelled,
    NeedsForceKill,
    SpawnError(String),
}

/// In-process backend stub — never execs a real tool (Linux ETXTBSY-safe).
pub struct FakeRunner {
    outcome: FakeOutcome,
}

impl FakeRunner {
    pub fn stdout(stdout: &str) -> Self {
        Self::exit(0, stdout, "")
    }

    pub fn exit(code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            outcome: FakeOutcome::Exit {
                code,
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
            },
        }
    }

    pub fn cancelled() -> Self {
        Self {
            outcome: FakeOutcome::Cancelled,
        }
    }

    pub fn needs_force_kill() -> Self {
        Self {
            outcome: FakeOutcome::NeedsForceKill,
        }
    }

    pub fn spawn_error(message: &str) -> Self {
        Self {
            outcome: FakeOutcome::SpawnError(message.to_string()),
        }
    }
}

impl ProcessRunner for FakeRunner {
    fn run_command(
        &self,
        _program: &str,
        _args: &[String],
        _control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        match &self.outcome {
            FakeOutcome::Exit {
                code,
                stdout,
                stderr,
            } => Ok(CommandRunOutcome::Completed(CommandOutput {
                status: exit_status(*code),
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            })),
            FakeOutcome::Cancelled => Ok(CommandRunOutcome::Cancelled),
            FakeOutcome::NeedsForceKill => Ok(CommandRunOutcome::NeedsForceKill),
            FakeOutcome::SpawnError(e) => Err(e.clone()),
        }
    }

    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        if let FakeOutcome::Exit { stdout, .. } = &self.outcome {
            for line in stdout.lines() {
                on_line(line);
            }
        }
        self.run_command(program, args, control)
    }
}
