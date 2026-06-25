// External process execution and progress parsing.
//
// General-purpose utilities for running external commands and parsing
// progress output. Used by drive enumeration, CLI commands, and the GUI.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

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
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    Ok(CommandOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Run a command, invoking `on_line` for each stdout/stderr line as it arrives.
pub fn run_command_streaming<F>(
    program: &str,
    args: &[String],
    mut on_line: F,
) -> Result<CommandOutput, String>
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

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    while let Ok(msg) = rx.recv() {
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

    stdout_handle
        .join()
        .map_err(|_| "stdout reader panicked".to_string())?;
    stderr_handle
        .join()
        .map_err(|_| "stderr reader panicked".to_string())?;

    let status = child.wait().map_err(|e| e.to_string())?;

    Ok(CommandOutput {
        status,
        stdout: stdout_buf,
        stderr: stderr_buf,
    })
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
        let digits: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(n) = digits.parse::<f32>() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
