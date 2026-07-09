//! Production process runner adapter.
//!
//! This module is **excluded from coverage metrics** (see `codecov.yml`, CI
//! `cargo llvm-cov --ignore-filename-regex`, and `Agents.md`).
//!
//! Rationale (same class as `gui/file_dialog.rs`):
//! - Thin delegation to real OS process I/O (`run_command_*`).
//! - Behaviour is already covered via the [`ProcessRunner`] trait with mocks
//!   in orchestration / GUI tests, and via direct `process::` unit tests.
//! - Measuring NativeRunner itself only inflates noise (spawn paths, OS diffs)
//!   without increasing confidence in flash/probe domain logic.

use crate::process::{
    run_command_cancellable, run_command_streaming_cancellable, CommandRunOutcome,
    OperationControl, ProcessRunner,
};

/// Production adapter that delegates to real process execution.
pub struct NativeRunner;

impl ProcessRunner for NativeRunner {
    fn run_command(
        &self,
        program: &str,
        args: &[String],
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        run_command_cancellable(program, args, control)
    }

    fn run_command_streaming(
        &self,
        program: &str,
        args: &[String],
        on_line: &dyn Fn(&str),
        control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        run_command_streaming_cancellable(program, args, on_line, control)
    }
}
