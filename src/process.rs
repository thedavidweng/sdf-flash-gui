use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Outcome of a cancellable backend command.
pub enum CommandRunOutcome {
    Completed(CommandOutput),
    Cancelled,
    NeedsForceKill,
}

/// Shared handle for cancelling a running backend process.
#[derive(Clone)]
pub struct OperationControl {
    cancel_requested: Arc<AtomicBool>,
    force_kill: Arc<AtomicBool>,
    child_exited: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl OperationControl {
    pub fn new() -> Self {
        Self {
            cancel_requested: Arc::new(AtomicBool::new(false)),
            force_kill: Arc::new(AtomicBool::new(false)),
            child_exited: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    pub fn is_force_kill_requested(&self) -> bool {
        self.force_kill.load(Ordering::SeqCst)
    }

    /// Ask the backend to stop gracefully (e.g. SIGINT).
    pub fn request_graceful_cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
        let _ = self
            .child
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().map(|child| try_graceful_terminate(child)));
    }

    /// Forcibly terminate the backend process.
    pub fn request_force_kill(&self) {
        self.force_kill.store(true, Ordering::SeqCst);
        self.cancel_requested.store(true, Ordering::SeqCst);
        let _ = self
            .child
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().map(|child| child.kill()));
    }

    /// Block until the registered child is reaped, force-killing first if needed.
    pub fn reap_registered_child(&self) {
        let Some(mut child) = self.child.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
        self.child_exited.store(true, Ordering::SeqCst);
    }

    /// Returns true while the registered child process is still running.
    pub fn is_child_running(&self) -> bool {
        if self.child_exited.load(Ordering::SeqCst) {
            return false;
        }
        self.child.lock().ok().is_some_and(|mut guard| {
            guard.as_mut().is_some_and(|child| {
                let running = matches!(child.try_wait(), Ok(None));
                if !running {
                    self.child_exited.store(true, Ordering::SeqCst);
                }
                running
            })
        })
    }

    fn register_child(&self, child: Child) {
        self.child_exited.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }
    }

    fn take_child(&self) -> Option<Child> {
        self.child.lock().ok().and_then(|mut guard| guard.take())
    }

    fn child_exit_status(&self) -> Option<ExitStatus> {
        self.child.lock().ok().and_then(|mut guard| {
            guard
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
        })
    }

    #[cfg(test)]
    pub(crate) fn set_cancel_requested_for_test(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn register_child_for_test(&self, child: Child) {
        self.child_exited.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }
    }
}

impl Default for OperationControl {
    fn default() -> Self {
        Self::new()
    }
}

const GRACEFUL_CANCEL_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_millis(800)
} else {
    Duration::from_secs(5)
};
const POLL_INTERVAL: Duration = if cfg!(test) {
    Duration::from_millis(5)
} else {
    Duration::from_millis(100)
};

fn require_completed(outcome: CommandRunOutcome) -> Result<CommandOutput, String> {
    match outcome {
        CommandRunOutcome::Completed(output) => Ok(output),
        CommandRunOutcome::Cancelled => Err("operation cancelled".into()),
        CommandRunOutcome::NeedsForceKill => {
            Err("backend did not stop; force kill required".into())
        }
    }
}

#[cfg(test)]
thread_local! {
    static SKIP_GRACEFUL_TERMINATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn try_graceful_terminate(child: &Child) -> Result<(), String> {
    #[cfg(test)]
    if SKIP_GRACEFUL_TERMINATE.with(|skip| skip.get()) {
        let _ = child;
        return Ok(());
    }

    let pid = child.id();
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("failed to send SIGINT to {pid}: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("kill -INT {pid} exited with {status}"))
        }
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("failed to request graceful stop for {pid}: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill /PID {pid} exited with {status}"))
        }
    }
}

fn finish_cancelled_child(
    control: &OperationControl,
    stdout: String,
    stderr: String,
) -> Result<CommandRunOutcome, String> {
    if !control.is_cancel_requested() && !control.is_force_kill_requested() {
        if let Some(status) = control.child_exit_status() {
            let _ = control.take_child();
            return Ok(CommandRunOutcome::Completed(CommandOutput {
                status,
                stdout,
                stderr,
            }));
        }
    }

    if !control.is_force_kill_requested() {
        let _ = control
            .child
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().map(|child| try_graceful_terminate(child)));
    }

    let deadline = Instant::now() + GRACEFUL_CANCEL_TIMEOUT;
    loop {
        if control.child_exit_status().is_some() {
            let _ = control.take_child();
            return Ok(CommandRunOutcome::Cancelled);
        }
        if control.is_force_kill_requested() {
            let _ = control
                .child
                .lock()
                .ok()
                .and_then(|mut guard| guard.as_mut().map(|child| child.kill()));
            continue;
        }
        if Instant::now() >= deadline && !control.is_force_kill_requested() {
            return Ok(CommandRunOutcome::NeedsForceKill);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Result of running an external command.
pub struct CommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn combined(&self) -> String {
        let out = self.stdout.trim();
        let err = self.stderr.trim();
        match (out.is_empty(), err.is_empty()) {
            (true, true) => String::new(),
            (false, true) => out.to_string(),
            (true, false) => err.to_string(),
            (false, false) => format!("{out}\n{err}"),
        }
    }
}

/// Run an external command and return stdout+stderr.
pub fn run_command(program: &str, args: &[String]) -> Result<CommandOutput, String> {
    require_completed(run_command_cancellable(program, args, None)?)
}

/// Cancellable variant of [`run_command`].
pub fn run_command_cancellable(
    program: &str,
    args: &[String],
    control: Option<&OperationControl>,
) -> Result<CommandRunOutcome, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;

    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        reader
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
            .join("\n")
    });
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        reader
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
            .join("\n")
    });

    let mut owned_child = Some(child);
    if let Some(control) = control {
        let child = owned_child
            .take()
            .ok_or_else(|| "child process handle missing".to_string())?;
        control.register_child(child);
        loop {
            if control.is_cancel_requested() || control.is_force_kill_requested() {
                let outcome = finish_cancelled_child(control, String::new(), String::new())?;
                join_pipe_readers(stdout_handle, stderr_handle, &outcome);
                return Ok(outcome);
            }
            if child_has_exited(control) {
                break;
            }
            thread::sleep(POLL_INTERVAL);
        }
        let stdout = stdout_handle
            .join()
            .map_err(|_| "stdout reader panicked".to_string())?;
        let stderr = stderr_handle
            .join()
            .map_err(|_| "stderr reader panicked".to_string())?;
        return completed_or_cancelled_after_pipes(control, stdout, stderr);
    }

    let status = owned_child
        .take()
        .ok_or_else(|| "child process handle missing".to_string())?
        .wait()
        .map_err(|e| e.to_string())?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| "stdout reader panicked".to_string())?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| "stderr reader panicked".to_string())?;

    Ok(CommandRunOutcome::Completed(CommandOutput {
        status,
        stdout,
        stderr,
    }))
}

/// Run a command, invoking `on_line` for each stdout/stderr line as it arrives.
pub fn run_command_streaming<F>(
    program: &str,
    args: &[String],
    on_line: F,
) -> Result<CommandOutput, String>
where
    F: FnMut(&str),
{
    require_completed(run_command_streaming_cancellable(
        program, args, on_line, None,
    )?)
}

/// Cancellable variant of [`run_command_streaming`].
pub fn run_command_streaming_cancellable<F>(
    program: &str,
    args: &[String],
    mut on_line: F,
    control: Option<&OperationControl>,
) -> Result<CommandRunOutcome, String>
where
    F: FnMut(&str),
{
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;

    enum PipeLine {
        Out(String),
        Err(String),
    }

    let (tx, rx) = mpsc::channel::<PipeLine>();
    let tx_out = tx.clone();

    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_out.send(PipeLine::Out(line));
        }
    });

    let tx_err = tx.clone();
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_err.send(PipeLine::Err(line));
        }
    });

    drop(tx);

    let mut owned_child = Some(child);
    if let Some(control) = control {
        let child = owned_child
            .take()
            .ok_or_else(|| "child process handle missing".to_string())?;
        control.register_child(child);
    }

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    let mut cancelled = false;
    loop {
        if let Some(control) = control {
            if control.is_cancel_requested() || control.is_force_kill_requested() {
                cancelled = true;
                break;
            }
        }
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(msg) => {
                let line = match msg {
                    PipeLine::Out(line) => {
                        if !stdout_buf.is_empty() {
                            stdout_buf.push('\n');
                        }
                        stdout_buf.push_str(&line);
                        line
                    }
                    PipeLine::Err(line) => {
                        if !stderr_buf.is_empty() {
                            stderr_buf.push('\n');
                        }
                        stderr_buf.push_str(&line);
                        line
                    }
                };
                on_line(&line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    if let Some(control) = control {
        if cancelled || control.is_cancel_requested() || control.is_force_kill_requested() {
            let outcome = finish_cancelled_child(control, stdout_buf, stderr_buf)?;
            join_pipe_readers(stdout_handle, stderr_handle, &outcome);
            return Ok(outcome);
        }
        let status = wait_for_registered_child(control)?;
        stdout_handle
            .join()
            .map_err(|_| "stdout reader panicked".to_string())?;
        stderr_handle
            .join()
            .map_err(|_| "stderr reader panicked".to_string())?;
        return Ok(CommandRunOutcome::Completed(CommandOutput {
            status,
            stdout: stdout_buf,
            stderr: stderr_buf,
        }));
    }

    stdout_handle
        .join()
        .map_err(|_| "stdout reader panicked".to_string())?;
    stderr_handle
        .join()
        .map_err(|_| "stderr reader panicked".to_string())?;

    let status = owned_child
        .take()
        .ok_or_else(|| "child process handle missing".to_string())?
        .wait()
        .map_err(|e| e.to_string())?;

    Ok(CommandRunOutcome::Completed(CommandOutput {
        status,
        stdout: stdout_buf,
        stderr: stderr_buf,
    }))
}

fn child_has_exited(control: &OperationControl) -> bool {
    if control.child_exited.load(Ordering::SeqCst) {
        return true;
    }
    let exited = control
        .child
        .lock()
        .ok()
        .and_then(|mut guard| {
            guard
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
        })
        .is_some();
    if exited {
        control.child_exited.store(true, Ordering::SeqCst);
    }
    exited
}

fn completed_or_cancelled_after_pipes(
    control: &OperationControl,
    stdout: String,
    stderr: String,
) -> Result<CommandRunOutcome, String> {
    if control.is_cancel_requested() || control.is_force_kill_requested() {
        finish_cancelled_child(control, stdout, stderr)
    } else {
        let status = wait_for_registered_child(control)?;
        Ok(CommandRunOutcome::Completed(CommandOutput {
            status,
            stdout,
            stderr,
        }))
    }
}

fn wait_for_registered_child(control: &OperationControl) -> Result<ExitStatus, String> {
    let mut child = control
        .take_child()
        .ok_or_else(|| "child handle missing".to_string())?;
    child.wait().map_err(|e| e.to_string())
}

/// Join pipe reader threads unless the child still needs a force-kill prompt.
/// Joining while the backend is alive would block on pipe EOF indefinitely.
fn join_pipe_readers<T>(
    stdout: thread::JoinHandle<T>,
    stderr: thread::JoinHandle<T>,
    outcome: &CommandRunOutcome,
) {
    if matches!(outcome, CommandRunOutcome::NeedsForceKill) {
        drop(stdout);
        drop(stderr);
    } else {
        let _ = stdout.join();
        let _ = stderr.join();
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

/// Format a command for display in the log.
pub fn format_command(cmd: &crate::command::Command) -> String {
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

/// Seam for backend process execution. Production uses [`NativeRunner`]; tests inject mocks.
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

#[cfg(test)]
mod tests {
    use super::*;

    struct RestoreGracefulTerminate(());

    impl RestoreGracefulTerminate {
        fn disable() -> Self {
            SKIP_GRACEFUL_TERMINATE.with(|skip| skip.set(true));
            Self(())
        }
    }

    impl Drop for RestoreGracefulTerminate {
        fn drop(&mut self) {
            SKIP_GRACEFUL_TERMINATE.with(|skip| skip.set(false));
        }
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

    #[cfg(unix)]
    fn make_exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }

    #[test]
    #[cfg(unix)]
    fn combined_both_empty() {
        let out = CommandOutput {
            status: make_exit_status(0),
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(out.combined(), "");
    }

    #[test]
    #[cfg(unix)]
    fn combined_only_stdout() {
        let out = CommandOutput {
            status: make_exit_status(0),
            stdout: "hello\n".into(),
            stderr: String::new(),
        };
        assert_eq!(out.combined(), "hello");
    }

    #[test]
    #[cfg(unix)]
    fn combined_only_stderr() {
        let out = CommandOutput {
            status: make_exit_status(0),
            stdout: String::new(),
            stderr: "error\n".into(),
        };
        assert_eq!(out.combined(), "error");
    }

    #[test]
    #[cfg(unix)]
    fn combined_both_present() {
        let out = CommandOutput {
            status: make_exit_status(0),
            stdout: "out\n".into(),
            stderr: "err\n".into(),
        };
        let c = out.combined();
        assert!(c.contains("out"));
        assert!(c.contains("err"));
    }

    #[test]
    #[cfg(unix)]
    fn combined_whitespace_only_stdout() {
        let out = CommandOutput {
            status: make_exit_status(0),
            stdout: "   \n  ".into(),
            stderr: "err\n".into(),
        };
        assert_eq!(out.combined(), "err");
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

    #[test]
    fn parse_progress_percent_prgv_partial() {
        // PRGV with only one value — should return None
        assert_eq!(parse_progress_percent("PRGV:50"), None);
    }

    #[test]
    fn parse_progress_percent_prgv_over_100() {
        // Should clamp to 100
        let p = parse_progress_percent("PRGV:200,100").unwrap();
        assert!((p - 100.0).abs() < 0.01);
    }

    #[test]
    fn parse_progress_percent_prgv_negative() {
        // Should clamp to 0
        let p = parse_progress_percent("PRGV:-10,100").unwrap();
        assert!((p - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_progress_percent_multiple_percent_signs() {
        // Should use the last % sign
        assert_eq!(parse_progress_percent("10% then 90%"), Some(90.0));
    }

    #[test]
    fn parse_progress_percent_no_digits_before_percent() {
        assert_eq!(parse_progress_percent("%"), None);
    }

    #[test]
    fn run_command_echo() {
        let result = run_command("echo", &["hello".into()]);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.success());
        assert!(out.stdout.contains("hello"));
    }

    #[test]
    fn run_command_fails_for_nonexistent() {
        let result = run_command("definitely_not_a_real_command_12345", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn run_command_exit_code() {
        // Use a command that exits with a non-zero code
        let result = run_command("sh", &["-c".into(), "exit 42".into()]);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(!out.success());
    }

    #[test]
    fn run_command_stderr() {
        let result = run_command("sh", &["-c".into(), "echo error >&2".into()]);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.stderr.contains("error"));
    }

    #[test]
    fn run_command_streaming_echo() {
        let mut lines = Vec::new();
        let result = run_command_streaming("echo", &["hello".into()], |line| {
            lines.push(line.to_string());
        });
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.success());
        assert!(lines.iter().any(|l| l.contains("hello")));
    }

    #[test]
    fn run_command_streaming_fails() {
        let result = run_command_streaming("definitely_not_real_12345", &[], |_| {});
        assert!(result.is_err());
    }

    #[test]
    fn run_command_streaming_stderr() {
        let mut lines = Vec::new();
        let result = run_command_streaming(
            "sh",
            &["-c".into(), "echo out; echo err >&2".into()],
            |line| {
                lines.push(line.to_string());
            },
        );
        assert!(result.is_ok());
        assert!(lines.iter().any(|l| l.contains("out")));
        assert!(lines.iter().any(|l| l.contains("err")));
    }

    #[test]
    fn run_command_streaming_multi_line_stdout() {
        let mut lines = Vec::new();
        let result = run_command_streaming(
            "sh",
            &["-c".into(), "echo line1; echo line2".into()],
            |line| {
                lines.push(line.to_string());
            },
        );
        assert!(result.is_ok());
        let out = result.unwrap();
        // Both lines should appear in stdout buffer (joined by newline)
        assert!(out.stdout.contains("line1"));
        assert!(out.stdout.contains("line2"));
        assert!(lines.iter().any(|l| l.contains("line1")));
        assert!(lines.iter().any(|l| l.contains("line2")));
    }

    #[test]
    fn run_command_streaming_multi_line_stderr() {
        let mut lines = Vec::new();
        let result = run_command_streaming(
            "sh",
            &["-c".into(), "echo err1 >&2; echo err2 >&2".into()],
            |line| {
                lines.push(line.to_string());
            },
        );
        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.stderr.contains("err1"));
        assert!(out.stderr.contains("err2"));
    }

    #[test]
    fn operation_control_default_is_fresh() {
        let control = OperationControl::default();
        assert!(!control.is_cancel_requested());
        assert!(!control.is_force_kill_requested());
    }

    #[test]
    fn operation_control_tracks_cancel_and_force_kill() {
        let control = OperationControl::new();
        assert!(!control.is_cancel_requested());
        assert!(!control.is_force_kill_requested());
        control.request_graceful_cancel();
        assert!(control.is_cancel_requested());
        control.request_force_kill();
        assert!(control.is_force_kill_requested());
    }

    #[test]
    fn run_command_cancellable_completes_without_control() {
        let outcome = run_command_cancellable("echo", &["cancellable".into()], None).unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout.contains("cancellable")
        ));
    }

    #[test]
    fn run_command_streaming_cancellable_completes_without_control() {
        let mut lines = Vec::new();
        let outcome = run_command_streaming_cancellable(
            "echo",
            &["stream".into()],
            |line| {
                lines.push(line.to_string());
            },
            None,
        )
        .unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && lines.iter().any(|l| l.contains("stream"))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_cancellable_graceful_cancel() {
        use std::thread;
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            ctrl.request_graceful_cancel();
        });
        let outcome = run_command_cancellable("sleep", &["5".into()], Some(&control)).unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::Completed(_)
        ));
    }

    #[test]
    fn join_pipe_readers_skips_join_on_needs_force_kill() {
        let stdout = thread::spawn(|| "stdout".to_string());
        let stderr = thread::spawn(|| "stderr".to_string());
        join_pipe_readers(stdout, stderr, &CommandRunOutcome::NeedsForceKill);
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_stops_running_process() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        control.register_child(child);
        control.request_graceful_cancel();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("cancel");
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
        thread::sleep(Duration::from_millis(50));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_graceful_cancel() {
        use std::thread;
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            ctrl.request_graceful_cancel();
        });
        let outcome =
            run_command_streaming_cancellable("sleep", &["5".into()], |_| {}, Some(&control))
                .unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::Completed(_)
        ));
    }

    #[test]
    fn require_completed_maps_non_success_outcomes() {
        assert!(matches!(
            require_completed(CommandRunOutcome::Cancelled),
            Err(ref msg) if msg == "operation cancelled"
        ));
        assert!(matches!(
            require_completed(CommandRunOutcome::NeedsForceKill),
            Err(ref msg) if msg == "backend did not stop; force kill required"
        ));
    }

    #[test]
    fn operation_control_request_force_kill() {
        let control = OperationControl::new();
        control.request_force_kill();
        assert!(control.is_force_kill_requested());
        assert!(control.is_cancel_requested());
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_request_graceful_cancel_with_registered_child() {
        use std::process::Command;

        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let control = OperationControl::new();
        control.register_child_for_test(child);
        control.request_graceful_cancel();
        assert!(control.is_cancel_requested());
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_request_force_kill_with_registered_child() {
        use std::process::Command;
        use std::time::Duration;

        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let control = OperationControl::new();
        control.register_child_for_test(child);
        control.request_force_kill();
        thread::sleep(Duration::from_millis(50));
        assert!(!control.is_child_running());
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_graceful_terminate_before_timeout() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        control.set_cancel_requested_for_test();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("cancel");
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_force_kill_after_kill_when_child_exits() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        control.request_force_kill();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("force kill");
        assert!(matches!(outcome, CommandRunOutcome::Cancelled));
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_skips_early_completed_when_child_still_running() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("still running");
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(_) | CommandRunOutcome::Cancelled
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_cancellable_cancel_set_during_pipe_join() {
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(80));
            ctrl.set_cancel_requested_for_test();
        });
        let outcome = run_command_cancellable(
            "sh",
            &["-c".into(), "yes | head -n 500".into()],
            Some(&control),
        )
        .unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::Completed(_)
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_breaks_when_child_exits_and_pipe_drained() {
        let control = OperationControl::new();
        let mut lines = Vec::new();
        let outcome = run_command_streaming_cancellable(
            "sh",
            &["-c".into(), "echo drained; sleep 0.12".into()],
            |line| {
                lines.push(line.to_string());
            },
            Some(&control),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
        ));
        assert!(lines.iter().any(|l| l.contains("drained")));
    }

    #[test]
    #[cfg(unix)]
    fn try_graceful_terminate_on_exited_child_returns_err() {
        use std::process::Command;

        let mut child = Command::new("true").spawn().unwrap();
        child.wait().expect("wait");
        assert!(try_graceful_terminate(&mut child).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_returns_cancelled_after_mid_loop_force_kill() {
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        control.set_cancel_requested_for_test();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            ctrl.request_force_kill();
        });
        let outcome = finish_cancelled_child(&control, String::new(), String::new()).unwrap();
        trigger.join().unwrap();
        assert!(matches!(outcome, CommandRunOutcome::Cancelled));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_true_exits_without_output() {
        let control = OperationControl::new();
        let outcome =
            run_command_streaming_cancellable("true", &[], |_| {}, Some(&control)).unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
        ));
    }

    #[test]
    fn operation_control_is_child_running_without_child() {
        let control = OperationControl::new();
        assert!(!control.is_child_running());
    }

    #[test]
    #[cfg(unix)]
    fn reap_registered_child_waits_for_running_child() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        control.request_force_kill();
        control.reap_registered_child();
        assert!(!control.is_child_running());
    }

    #[test]
    fn reap_registered_child_noop_without_child() {
        let control = OperationControl::new();
        control.reap_registered_child();
        assert!(!control.is_child_running());
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_is_child_running_uses_exit_cache() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("true").spawn().unwrap();
        control.register_child_for_test(child);
        thread::sleep(Duration::from_millis(50));
        assert!(!control.is_child_running());
        assert!(!control.is_child_running());
    }

    #[test]
    #[cfg(unix)]
    fn child_has_exited_caches_exit_status() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("true").spawn().unwrap();
        control.register_child_for_test(child);
        thread::sleep(Duration::from_millis(50));
        assert!(child_has_exited(&control));
        assert!(child_has_exited(&control));
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_is_child_running_with_live_child() {
        use std::process::Command;
        use std::time::Duration;

        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let control = OperationControl::new();
        control.register_child_for_test(child);
        assert!(control.is_child_running());
        control.request_force_kill();
        thread::sleep(Duration::from_millis(50));
        assert!(!control.is_child_running());
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_graceful_cancel_with_running_child() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().expect("spawn");
        control.register_child(child);
        control.request_graceful_cancel();
        assert!(control.is_cancel_requested());
        control.request_force_kill();
        let _ = finish_cancelled_child(&control, String::new(), String::new());
    }

    #[test]
    #[cfg(unix)]
    fn operation_control_requests_apply_to_registered_child() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().expect("spawn");
        control.register_child(child);
        control.request_graceful_cancel();
        thread::sleep(Duration::from_millis(50));
        control.request_force_kill();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("cancelled");
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_completes_when_child_already_exited_without_cancel() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("true").spawn().expect("spawn true");
        control.register_child(child);
        thread::sleep(Duration::from_millis(100));
        let outcome =
            finish_cancelled_child(&control, "stdout".into(), "stderr".into()).expect("completed");
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout == "stdout"
                && out.stderr == "stderr"
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_completes_with_control() {
        let control = OperationControl::new();
        let outcome = run_command_streaming_cancellable(
            "echo",
            &["stream-control".into()],
            |_| {},
            Some(&control),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout.contains("stream-control")
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_cancel_during_output() {
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(80));
            ctrl.request_graceful_cancel();
        });
        let outcome = run_command_streaming_cancellable(
            "sh",
            &["-c".into(), "echo line1; sleep 2; echo line2".into()],
            |_| {},
            Some(&control),
        )
        .unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
    }

    #[test]
    #[cfg(unix)]
    fn completed_or_cancelled_after_pipes_honours_cancel() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        control.register_child_for_test(child);
        control.set_cancel_requested_for_test();
        let outcome =
            completed_or_cancelled_after_pipes(&control, "out".into(), "err".into()).unwrap();
        assert!(matches!(outcome, CommandRunOutcome::Cancelled));
    }

    #[test]
    #[cfg(unix)]
    fn completed_or_cancelled_after_pipes_completes_when_child_exited() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("true").spawn().unwrap();
        control.register_child_for_test(child);
        thread::sleep(Duration::from_millis(50));
        let outcome =
            completed_or_cancelled_after_pipes(&control, "out".into(), "err".into()).unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout == "out"
                && out.stderr == "err"
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_breaks_on_child_exit_with_open_channel() {
        let control = OperationControl::new();
        let mut lines = Vec::new();
        let outcome = run_command_streaming_cancellable(
            "sh",
            &["-c".into(), "sleep 0.2; echo done >&2".into()],
            |line| {
                lines.push(line.to_string());
            },
            Some(&control),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
        ));
        assert!(lines.iter().any(|l| l.contains("done")));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_child_exit_breaks_read_loop() {
        let control = OperationControl::new();
        let mut lines = Vec::new();
        let outcome = run_command_streaming_cancellable(
            "sh",
            &["-c".into(), "echo streamed; sleep 0.05".into()],
            |line| {
                lines.push(line.to_string());
            },
            Some(&control),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
        ));
        assert!(lines.iter().any(|l| l.contains("streamed")));
    }

    #[test]
    fn join_pipe_readers_joins_on_cancelled() {
        let stdout = thread::spawn(|| "stdout".to_string());
        let stderr = thread::spawn(|| "stderr".to_string());
        join_pipe_readers(stdout, stderr, &CommandRunOutcome::Cancelled);
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_returns_completed_when_child_exited() {
        use std::process::Command;
        use std::time::Duration;

        let control = OperationControl::new();
        let child = Command::new("true").spawn().expect("spawn true");
        control.register_child(child);
        thread::sleep(Duration::from_millis(50));
        let outcome = finish_cancelled_child(&control, "out".into(), "err".into()).unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout == "out"
                && out.stderr == "err"
        ));
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_force_kill_stops_stubborn_process() {
        use std::process::Command;

        let control = OperationControl::new();
        let child = Command::new("sh")
            .args(["-c", "trap '' INT; while true; do sleep 1; done"])
            .spawn()
            .expect("spawn stubborn shell");
        control.register_child(child);
        control.request_force_kill();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("force kill");
        assert!(matches!(outcome, CommandRunOutcome::Cancelled));
    }

    #[test]
    #[cfg(unix)]
    fn finish_cancelled_child_needs_force_kill_when_sigint_ignored() {
        use std::process::Command;

        let _guard = RestoreGracefulTerminate::disable();
        let control = OperationControl::new();
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        control.register_child(child);
        control.set_cancel_requested_for_test();
        let outcome =
            finish_cancelled_child(&control, String::new(), String::new()).expect("timed out");
        assert!(matches!(outcome, CommandRunOutcome::NeedsForceKill));
        control.request_force_kill();
        let _ = finish_cancelled_child(&control, String::new(), String::new());
    }

    #[test]
    #[cfg(unix)]
    fn run_command_cancellable_completes_with_control() {
        let control = OperationControl::new();
        let outcome =
            run_command_cancellable("echo", &["with-control".into()], Some(&control)).unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Completed(ref out) if out.success()
                && out.stdout.contains("with-control")
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_cancellable_force_kill() {
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            ctrl.request_force_kill();
        });
        let outcome = run_command_cancellable("sleep", &["30".into()], Some(&control)).unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_streaming_cancellable_force_kill() {
        use std::time::Duration;

        let control = OperationControl::new();
        let ctrl = control.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            ctrl.request_force_kill();
        });
        let outcome =
            run_command_streaming_cancellable("sleep", &["30".into()], |_| {}, Some(&control))
                .unwrap();
        trigger.join().unwrap();
        assert!(matches!(
            outcome,
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill
        ));
    }
}
