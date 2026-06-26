// Abstraction over process execution for testability.

use crate::process::{self, CommandRunOutcome, OperationControl};

/// Trait for running external processes.
pub trait ProcessRunner: Send + Sync + 'static {
    fn run_command(
        &self,
        program: &str,
        args: &[String],
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String>;
    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String>;
}

/// Production implementation that delegates to real process execution.
pub struct NativeRunner;

impl ProcessRunner for NativeRunner {
    fn run_command(
        &self,
        program: &str,
        args: &[String],
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        process::run_command_cancellable(program, args, control)
    }

    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        process::run_command_streaming_cancellable(program, args, on_line, control)
    }
}
