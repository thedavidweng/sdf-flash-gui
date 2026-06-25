// Worker message types, dispatch, and thread spawning.

use crate::command::{self, Command};
use crate::drive::Drive;
use crate::process;

use super::process_runner::ProcessRunner;

use super::state::AppState;

use eframe::egui;
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Debug)]
pub enum WorkerMsg {
    Log(String),
    Progress(f32),
    Status {
        message: String,
        progress: f32,
    },
    ProbeComplete {
        drive_idx: usize,
        mt1959: bool,
        encrypted_firmware: bool,
        error: Option<String>,
    },
    OperationComplete {
        success: bool,
        status: String,
        progress: f32,
    },
    DrivesListed(Vec<Drive>),
}

/// Whether the user should be notified after processing messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    Informational,
    Critical,
}

/// Process a single worker message against app state.
/// Returns an optional attention request (for viewport notification).
fn handle_worker_msg(msg: WorkerMsg, state: &mut AppState) -> Option<Attention> {
    match msg {
        WorkerMsg::Log(line) => {
            state.log(&line);
            None
        }
        WorkerMsg::Progress(p) => {
            state.progress_indeterminate = false;
            state.progress = p;
            None
        }
        WorkerMsg::Status { message, progress } => {
            state.status_message = message;
            state.progress = progress.clamp(0.0, 100.0);
            None
        }
        WorkerMsg::ProbeComplete {
            drive_idx,
            mt1959,
            encrypted_firmware,
            error,
        } => {
            if state.selected_drive == Some(drive_idx) {
                state.drive_probed = error.is_none();
                state.drive_mt1959 = mt1959;
                state.drive_encrypted_firmware = encrypted_firmware;
                if error.is_none() {
                    state.encrypted_write = encrypted_firmware;
                }
                state.last_probed_drive = Some(drive_idx);
            }
            state.probing = false;
            if let Some(err) = error {
                state.log(&format!("ERROR: {err}"));
                state.set_status("Drive probe failed", 0.0);
            } else {
                state.log(&format!(
                    "MT1959: {mt1959} | Encrypted FW: {encrypted_firmware}"
                ));
                state.set_status("Ready", 0.0);
            }
            None
        }
        WorkerMsg::OperationComplete {
            success,
            status,
            progress,
        } => {
            state.busy = false;
            state.progress_indeterminate = false;
            state.set_status(status, progress);
            let is_success = success && progress >= 100.0;
            if is_success {
                state.log("Operation completed successfully.");
            }
            Some(if is_success {
                Attention::Informational
            } else {
                Attention::Critical
            })
        }
        WorkerMsg::DrivesListed(drives) => {
            let count = drives.len();
            state.drives = drives;
            state.last_probed_drive = None;
            if state.selected_drive.is_none() && count > 0 {
                state.selected_drive = Some(0);
            }
            state.busy = false;
            state.progress_indeterminate = false;
            state.set_status("Ready", 0.0);
            state.log(&format!("Found {count} drive(s)."));
            None
        }
    }
}

/// Drain the worker channel and apply each message to app state.
/// Returns (needs_repaint, attention_request).
pub fn drain_worker_messages(
    state: &mut AppState,
    worker_rx: &std::sync::mpsc::Receiver<WorkerMsg>,
) -> (bool, Option<Attention>) {
    let mut repaint = false;
    let mut attention = None;
    while let Ok(msg) = worker_rx.try_recv() {
        repaint = true;
        if let Some(at) = handle_worker_msg(msg, state) {
            attention = Some(at);
        }
    }
    (repaint, attention)
}

/// Drain the worker channel, apply messages, and trigger egui side effects.
pub fn poll_worker(
    state: &mut AppState,
    worker_rx: &std::sync::mpsc::Receiver<WorkerMsg>,
    ctx: &egui::Context,
) {
    let (repaint, attention) = drain_worker_messages(state, worker_rx);
    if let Some(at) = attention {
        let egui_at = match at {
            Attention::Informational => egui::UserAttentionType::Informational,
            Attention::Critical => egui::UserAttentionType::Critical,
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui_at));
    }
    if repaint {
        ctx.request_repaint();
    }
}

pub fn spawn_probe(
    tx: &Sender<WorkerMsg>,
    state: &AppState,
    drive_idx: usize,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    let Some(drive) = state.drives.get(drive_idx) else {
        return;
    };
    if state.tool_path.is_empty() {
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx,
            mt1959: false,
            encrypted_firmware: false,
            error: Some("Configure backend in Settings".into()),
        });
        return;
    }

    let tx = tx.clone();
    let tool_path = state.tool_path.clone();
    let backend = state.backend;
    let device = drive.device.clone();
    let runner = runner.clone();

    let _ = tx.send(WorkerMsg::Status {
        message: "Probing drive".into(),
        progress: 0.0,
    });

    thread::spawn(move || {
        let cmd = command::plan_drive_info(backend, &tool_path, &device);
        let _ = tx.send(WorkerMsg::Log(format!(
            "> {}",
            process::format_command(&cmd)
        )));
        match runner.run_command(&cmd.program, &cmd.args) {
            Ok(out) => {
                let combined = out.combined();
                if !combined.is_empty() {
                    let _ = tx.send(WorkerMsg::Log(combined.clone()));
                }
                let safety = command::classify_drive_safety(&device, &combined);
                let _ = tx.send(WorkerMsg::ProbeComplete {
                    drive_idx,
                    mt1959: safety.mt1959,
                    encrypted_firmware: safety.encrypted_firmware,
                    error: if out.success() {
                        None
                    } else {
                        Some("Drive probe failed".into())
                    },
                });
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::ProbeComplete {
                    drive_idx,
                    mt1959: false,
                    encrypted_firmware: false,
                    error: Some(e),
                });
            }
        }
    });
}

pub fn spawn_streaming_command(
    tx: &Sender<WorkerMsg>,
    cmd: Command,
    initial_status: &str,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    let tx = tx.clone();
    let program = cmd.program;
    let args = cmd.args;
    let initial_status = initial_status.to_string();
    let runner = runner.clone();

    let _ = tx.send(WorkerMsg::Status {
        message: initial_status,
        progress: 0.0,
    });
    let _ = tx.send(WorkerMsg::Log(format!(
        "> {}",
        process::format_command(&Command {
            program: program.clone(),
            args: args.clone(),
        })
    )));

    thread::spawn(move || {
        let result = runner.run_command_streaming(&program, &args, &|line| {
            let _ = tx.send(WorkerMsg::Log(line.to_string()));
            if let Some(p) = process::parse_progress_percent(line) {
                let _ = tx.send(WorkerMsg::Progress(p));
            }
        });

        match result {
            Ok(out) => {
                let success = out.success();
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success,
                    status: if success {
                        "100% Operation finished. Please wait…".into()
                    } else {
                        "Operation failed".into()
                    },
                    progress: if success { 100.0 } else { 0.0 },
                });
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Log(format!("ERROR: {e}")));
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success: false,
                    status: "Operation failed".into(),
                    progress: 0.0,
                });
            }
        }
    });
}

pub fn spawn_list_drives(
    tx: &Sender<WorkerMsg>,
    state: &mut AppState,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    let cmd = command::plan_drive_list(state.backend, &state.tool_path);
    state.begin_operation("Listing drives");
    state.log(&format!("> {}", process::format_command(&cmd)));

    let tx = tx.clone();
    let program = cmd.program;
    let args = cmd.args;
    let runner = runner.clone();
    thread::spawn(move || match runner.run_command(&program, &args) {
        Ok(out) => {
            if !out.combined().is_empty() {
                let _ = tx.send(WorkerMsg::Log(out.combined()));
            }
            let drives = parse_drive_list(&out.stdout);
            let _ = tx.send(WorkerMsg::Log(format!(
                "Parsed {} drive(s) from output.",
                drives.len()
            )));
            let _ = tx.send(WorkerMsg::DrivesListed(drives));
        }
        Err(e) => {
            let _ = tx.send(WorkerMsg::Log(format!("ERROR: {e}")));
            let _ = tx.send(WorkerMsg::OperationComplete {
                success: false,
                status: "Drive list failed".into(),
                progress: 0.0,
            });
        }
    });
}

fn parse_drive_list(output: &str) -> Vec<Drive> {
    let mut drives = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit() || c == ':') {
            let rest = rest.trim_start_matches(':').trim();
            if !rest.is_empty() && (rest.starts_with("/dev/") || rest.contains(':')) {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                let device = parts.first().unwrap_or(&"").to_string();
                let vendor = parts.get(1).unwrap_or(&"").to_string();
                let product = parts.get(2).unwrap_or(&"").to_string();
                let revision = parts.get(3).unwrap_or(&"").to_string();
                drives.push(Drive {
                    device,
                    vendor,
                    product,
                    revision,
                });
            }
        }
    }
    drives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_drive_list_four_fields() {
        // 4-field format: device vendor product revision
        let output = "0:/dev/sr0 HL-DT-ST BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BU40N");
        assert_eq!(drives[0].revision, "1.03");
    }

    #[test]
    fn parse_drive_list_five_fields_drops_last() {
        // 5-field input: device vendor product model revision
        // NOTE: parse_drive_list only reads parts[0..3], so the 5th field
        // (firmware revision "1.03") is silently dropped and parts[3] (model
        // "BU40N") ends up in Drive::revision. This is a known limitation.
        let output = "0:/dev/sr0 HL-DT-ST BD-RE BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[0].vendor, "HL-DT-ST");
        assert_eq!(drives[0].product, "BD-RE");
        assert_eq!(drives[0].revision, "BU40N"); // model, not firmware revision
    }

    #[test]
    fn parse_drive_list_windows_format() {
        // Windows format with leading digit index
        let output = "0:D: HL-DT-ST BD-RE BU40N 1.03\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "D:");
    }

    #[test]
    fn parse_drive_list_empty() {
        let drives = parse_drive_list("");
        assert!(drives.is_empty());
    }

    #[test]
    fn parse_drive_list_no_drives() {
        let output = "No drives found\n";
        let drives = parse_drive_list(output);
        assert!(drives.is_empty());
    }

    #[test]
    fn parse_drive_list_multiple() {
        let output = "0:/dev/sr0 VENDOR1 PRODUCT1 REV1\n1:/dev/sr1 VENDOR2 PRODUCT2 REV2\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 2);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert_eq!(drives[1].device, "/dev/sr1");
    }

    #[test]
    fn parse_drive_list_with_colon_prefix() {
        let output = ":/dev/sr0 VENDOR PRODUCT REV\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
    }

    #[test]
    fn parse_drive_list_whitespace_only() {
        let output = "   \n  \n  ";
        let drives = parse_drive_list(output);
        assert!(drives.is_empty());
    }

    #[test]
    fn parse_drive_list_partial_info() {
        let output = "0:/dev/sr0\n";
        let drives = parse_drive_list(output);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].device, "/dev/sr0");
        assert!(drives[0].vendor.is_empty());
    }

    // --- drain_worker_messages tests ---

    use super::super::state::AppState;
    use super::super::OperationMode;

    fn test_drive() -> crate::drive::Drive {
        crate::drive::Drive {
            device: "/dev/sr0".into(),
            vendor: "HL-DT-ST".into(),
            product: "BU40N".into(),
            revision: "1.03".into(),
        }
    }

    #[test]
    fn drain_empty_channel() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(!repaint);
        assert!(attention.is_none());
    }

    #[test]
    fn drain_log_message() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Log("hello".into()));
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert!(attention.is_none());
        assert!(state.log_text.contains("hello"));
    }

    #[test]
    fn drain_progress_message() {
        let mut state = AppState::new_no_backend();
        state.progress_indeterminate = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Progress(42.0));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(!state.progress_indeterminate);
        assert!((state.progress - 42.0).abs() < 0.01);
    }

    #[test]
    fn drain_status_message() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Status {
            message: "Working".into(),
            progress: 50.0,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert_eq!(state.status_message, "Working");
        assert!((state.progress - 50.0).abs() < 0.01);
    }

    #[test]
    fn drain_status_clamps_progress() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Status {
            message: "test".into(),
            progress: 150.0,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!((state.progress - 100.0).abs() < 0.01);
    }

    #[test]
    fn drain_probe_complete_success() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 0,
            mt1959: true,
            encrypted_firmware: true,
            error: None,
        });
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert!(attention.is_none());
        assert!(!state.probing);
        assert!(state.drive_mt1959);
        assert!(state.drive_encrypted_firmware);
        assert!(state.drive_probed);
        assert!(state.encrypted_write);
        assert!(state.log_text.contains("MT1959: true"));
    }

    #[test]
    fn drain_probe_complete_error() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 0,
            mt1959: false,
            encrypted_firmware: false,
            error: Some("probe failed".into()),
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(!state.probing);
        assert!(!state.drive_probed);
        assert!(state.log_text.contains("ERROR"));
        assert!(state.status_message.contains("failed"));
    }

    #[test]
    fn drain_probe_complete_wrong_drive_idx() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        state.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 1, // different from selected
            mt1959: true,
            encrypted_firmware: true,
            error: None,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        // drive_mt1959 should NOT be updated since drive_idx doesn't match
        assert!(!state.drive_mt1959);
        // but probing should still be cleared
        assert!(!state.probing);
    }

    #[test]
    fn drain_operation_complete_success() {
        let mut state = AppState::new_no_backend();
        state.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::OperationComplete {
            success: true,
            status: "100% Done".into(),
            progress: 100.0,
        });
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert_eq!(attention, Some(Attention::Informational));
        assert!(!state.busy);
        assert!(!state.progress_indeterminate);
        assert!(state.log_text.contains("successfully"));
    }

    #[test]
    fn drain_operation_complete_failure() {
        let mut state = AppState::new_no_backend();
        state.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::OperationComplete {
            success: false,
            status: "Failed".into(),
            progress: 0.0,
        });
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert_eq!(attention, Some(Attention::Critical));
        assert!(!state.busy);
        assert!(!state.log_text.contains("successfully"));
    }

    #[test]
    fn drain_operation_complete_partial_progress() {
        let mut state = AppState::new_no_backend();
        state.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::OperationComplete {
            success: true,
            status: "50%".into(),
            progress: 50.0,
        });
        drop(tx);
        let (_, attention) = drain_worker_messages(&mut state, &rx);
        // success=true but progress < 100 → Critical (not fully done)
        assert_eq!(attention, Some(Attention::Critical));
    }

    #[test]
    fn drain_drives_listed() {
        let mut state = AppState::new_no_backend();
        state.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::DrivesListed(vec![test_drive()]));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert_eq!(state.drives.len(), 1);
        assert_eq!(state.selected_drive, Some(0));
        assert!(!state.busy);
        assert!(!state.progress_indeterminate);
        assert!(state.log_text.contains("Found 1 drive(s)"));
    }

    #[test]
    fn drain_drives_listed_empty() {
        let mut state = AppState::new_no_backend();
        state.selected_drive = Some(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::DrivesListed(vec![]));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(state.drives.is_empty());
        // selected_drive not changed to None by DrivesListed — only set to Some(0) if was None
    }

    #[test]
    fn drain_drives_listed_preserves_existing_selection() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.selected_drive = Some(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::DrivesListed(vec![
            test_drive(),
            crate::drive::Drive {
                device: "/dev/sr1".into(),
                vendor: "V".into(),
                product: "P".into(),
                revision: "R".into(),
            },
        ]));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        // selected_drive should remain Some(0) since it was already set
        assert_eq!(state.selected_drive, Some(0));
        assert_eq!(state.drives.len(), 2);
    }

    #[test]
    fn drain_multiple_messages() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Log("line1".into()));
        let _ = tx.send(WorkerMsg::Progress(25.0));
        let _ = tx.send(WorkerMsg::Log("line2".into()));
        let _ = tx.send(WorkerMsg::Status {
            message: "Working".into(),
            progress: 50.0,
        });
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert!(attention.is_none());
        assert!(state.log_text.contains("line1"));
        assert!(state.log_text.contains("line2"));
        assert!((state.progress - 50.0).abs() < 0.01); // last Status wins
        assert_eq!(state.status_message, "Working");
    }

    #[test]
    fn attention_display_debug() {
        // Verify Attention enum variants
        assert_eq!(format!("{:?}", Attention::Informational), "Informational");
        assert_eq!(format!("{:?}", Attention::Critical), "Critical");
    }

    // --- ProcessRunner mock and spawn tests ---

    use super::super::process_runner::ProcessRunner;
    use std::sync::Arc;

    struct MockRunner {
        output: String,
        success: bool,
    }

    impl MockRunner {
        fn success(output: &str) -> Self {
            Self {
                output: output.to_string(),
                success: true,
            }
        }

        fn failing() -> Self {
            Self {
                output: String::new(),
                success: false,
            }
        }
    }

    impl ProcessRunner for MockRunner {
        fn run_command(
            &self,
            _program: &str,
            _args: &[String],
        ) -> Result<crate::process::CommandOutput, String> {
            if self.success {
                Ok(make_output(&self.output))
            } else {
                Err("mock command failed".into())
            }
        }

        fn run_command_streaming(
            &self,
            _program: &str,
            _args: &[String],
            on_line: &dyn Fn(&str),
        ) -> Result<crate::process::CommandOutput, String> {
            if self.success {
                for line in self.output.lines() {
                    on_line(line);
                }
                Ok(make_output(&self.output))
            } else {
                Err("mock streaming failed".into())
            }
        }
    }

    fn make_output(stdout: &str) -> crate::process::CommandOutput {
        // Use a real subprocess to get a valid ExitStatus
        let status = std::process::Command::new("true").status().unwrap();
        crate::process::CommandOutput {
            status,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    #[test]
    fn spawn_probe_no_drive() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::success(""));
        spawn_probe(&tx, &state, 0, &runner);
        // No drive at index 0 → early return, no messages
        drop(tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn spawn_probe_empty_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::success(""));
        spawn_probe(&tx, &state, 0, &runner);
        drop(tx);
        let msg = rx.try_recv().unwrap();
        match msg {
            WorkerMsg::ProbeComplete { error, .. } => {
                assert!(error.is_some());
                assert!(error.unwrap().contains("Settings"));
            }
            _ => panic!("expected ProbeComplete"),
        }
    }

    #[test]
    fn spawn_probe_success() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> =
            Arc::new(MockRunner::success("Vendor: HL-DT-ST\nProduct: BU40N\n"));
        spawn_probe(&tx, &state, 0, &runner);
        // Wait for thread to finish
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        // Should have: Status, Log (> command), Log (output), ProbeComplete
        assert!(messages.len() >= 3);
        let probe = messages.last().unwrap();
        match probe {
            WorkerMsg::ProbeComplete { error, .. } => {
                assert!(error.is_none());
            }
            _ => panic!("expected ProbeComplete"),
        }
    }

    #[test]
    fn spawn_probe_command_fails() {
        let mut state = AppState::new_no_backend();
        state.drives.push(test_drive());
        state.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::failing());
        spawn_probe(&tx, &state, 0, &runner);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        let probe = messages.last().unwrap();
        match probe {
            WorkerMsg::ProbeComplete { error, .. } => {
                assert!(error.is_some());
                assert!(error.as_ref().unwrap().contains("mock command failed"));
            }
            _ => panic!("expected ProbeComplete with error"),
        }
    }

    #[test]
    fn spawn_streaming_command_success() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::success("line1\nline2\n"));
        let cmd = crate::command::Command {
            program: "/bin/echo".into(),
            args: vec![],
        };
        spawn_streaming_command(&tx, cmd, "Testing", &runner);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        // Should have: Status, Log (> command), Log (line1), Log (line2), OperationComplete
        assert!(messages.len() >= 4);
        let last = messages.last().unwrap();
        match last {
            WorkerMsg::OperationComplete { success, .. } => {
                assert!(success);
            }
            _ => panic!("expected OperationComplete"),
        }
    }

    #[test]
    fn spawn_streaming_command_fails() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::failing());
        let cmd = crate::command::Command {
            program: "/bin/false".into(),
            args: vec![],
        };
        spawn_streaming_command(&tx, cmd, "Testing", &runner);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        let last = messages.last().unwrap();
        match last {
            WorkerMsg::OperationComplete { success, .. } => {
                assert!(!success);
            }
            _ => panic!("expected OperationComplete failure"),
        }
    }

    #[test]
    fn spawn_list_drives_success() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> =
            Arc::new(MockRunner::success("0:/dev/sr0 HL-DT-ST BU40N 1.03\n"));
        spawn_list_drives(&tx, &mut state, &runner);
        assert!(state.busy);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        let last = messages.last().unwrap();
        match last {
            WorkerMsg::DrivesListed(drives) => {
                assert_eq!(drives.len(), 1);
                assert_eq!(drives[0].device, "/dev/sr0");
            }
            _ => panic!("expected DrivesListed"),
        }
    }

    #[test]
    fn spawn_list_drives_fails() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::failing());
        spawn_list_drives(&tx, &mut state, &runner);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        let last = messages.last().unwrap();
        match last {
            WorkerMsg::OperationComplete {
                success, status, ..
            } => {
                assert!(!success);
                assert!(status.contains("failed"));
            }
            _ => panic!("expected OperationComplete failure"),
        }
    }
}
