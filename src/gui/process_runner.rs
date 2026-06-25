// Abstraction over process execution for testability.

use crate::process::{self, CommandOutput};

/// Trait for running external processes.
pub trait ProcessRunner: Send + Sync + 'static {
    fn run_command(&self, program: &str, args: &[String]) -> Result<CommandOutput, String>;
    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
    ) -> Result<CommandOutput, String>;
}

/// Production implementation that delegates to real process execution.
pub struct NativeRunner;

impl ProcessRunner for NativeRunner {
    fn run_command(&self, program: &str, args: &[String]) -> Result<CommandOutput, String> {
        process::run_command(program, args)
    }

    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
    ) -> Result<CommandOutput, String> {
        process::run_command_streaming(program, args, on_line)
    }
}
