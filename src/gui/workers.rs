use crate::command::{self, Command};
use crate::drive::Drive;
use crate::i18n::{log_error, t, t_with_args, L10nKey, Language};
use crate::process::{self, CommandRunOutcome, OperationControl, ProcessRunner};

use super::state::AppState;

use std::sync::Arc;

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
        identity: Option<crate::manifest::DriveMatch>,
        error: Option<String>,
    },
    OperationComplete {
        success: bool,
        status: String,
        progress: f32,
    },
    DrivesListed(Vec<Drive>),
    StopNeedsForceKill,
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
            state.runtime.progress_indeterminate = false;
            state.runtime.progress = p;
            None
        }
        WorkerMsg::Status { message, progress } => {
            state.runtime.status_message = message;
            state.runtime.progress = progress.clamp(0.0, 100.0);
            None
        }
        WorkerMsg::ProbeComplete {
            drive_idx,
            mt1959,
            encrypted_firmware,
            identity,
            error,
        } => {
            let success = error.is_none();
            if state.drive.selected_drive == Some(drive_idx) {
                state.drive.drive_mt1959 = mt1959;
                state.drive.drive_encrypted_firmware = encrypted_firmware;
                state.drive.probe_identity = if success { identity } else { None };
                if success {
                    state.flash.encrypted_write = encrypted_firmware;
                }
            }
            state.record_probe_outcome(drive_idx, success);
            state.finish_probe();
            if let Some(err) = error {
                state.log(&err);
                state.set_status_key(L10nKey::StatusProbeFailed, 0.0);
            } else {
                state.log(&t_with_args(
                    L10nKey::LogProbeResult,
                    state.chrome.resolved_lang,
                    &[
                        ("mt1959", &mt1959.to_string()),
                        ("encrypted", &encrypted_firmware.to_string()),
                    ],
                ));
                state.set_status_key(L10nKey::StatusReady, 0.0);
            }
            None
        }
        WorkerMsg::OperationComplete {
            success,
            status,
            progress,
        } => {
            let risky_op = matches!(
                state.operation_mode,
                super::OperationMode::Write | super::OperationMode::Recover
            );
            let is_success = success && progress >= 100.0;
            if !is_success && risky_op {
                state.chrome.show_flash_failure_dialog = true;
            }
            state.finish_operation();
            state.set_status(status, progress);
            if is_success {
                state.log(t(L10nKey::StatusOpSuccess, state.chrome.resolved_lang));
            }
            Some(if is_success {
                Attention::Informational
            } else {
                Attention::Critical
            })
        }
        WorkerMsg::DrivesListed(drives) => {
            let count = drives.len();
            state.drive.drives = drives;
            state.drive.last_probed_drive = None;
            if state.drive.selected_drive.is_none() && count > 0 {
                state.drive.selected_drive = Some(0);
            }
            state.finish_operation();
            let lang = state.chrome.resolved_lang;
            state.set_status_key(L10nKey::StatusReady, 0.0);
            state.log(&t_with_args(
                L10nKey::StatusDrivesFound,
                lang,
                &[("count", &count.to_string())],
            ));
            None
        }
        WorkerMsg::StopNeedsForceKill => {
            state.runtime.stop_dialog = super::state::StopDialog::ConfirmForceKill;
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

/// Drain the worker channel, apply messages, and optionally trigger egui side effects.
fn poll_waiting_backend_stop(state: &mut AppState) -> bool {
    if !state.runtime.waiting_for_backend_stop {
        return false;
    }
    let still_running = state
        .runtime
        .active_operation
        .as_ref()
        .is_some_and(|control| control.is_child_running());
    if still_running {
        return false;
    }
    state.log(t(L10nKey::LogOpCancelled, state.chrome.resolved_lang));
    state.finish_operation();
    state.set_status_key(L10nKey::StatusOpCancelled, 0.0);
    true
}

fn poll_probe_backend_stop(state: &mut AppState) -> bool {
    if state.runtime.busy || state.runtime.probe_control.is_none() {
        return false;
    }
    let still_running = state
        .runtime
        .probe_control
        .as_ref()
        .is_some_and(|control| control.is_child_running());
    if still_running {
        return false;
    }
    state.finish_probe_failure();
    true
}

pub fn poll_worker(
    state: &mut AppState,
    worker_rx: &std::sync::mpsc::Receiver<WorkerMsg>,
    ctx: Option<&egui::Context>,
) {
    let waiting_repaint = poll_waiting_backend_stop(state) || poll_probe_backend_stop(state);
    let (repaint, attention) = drain_worker_messages(state, worker_rx);
    let repaint = repaint || waiting_repaint;
    let Some(ctx) = ctx else {
        return;
    };
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
    state: &mut AppState,
    drive_idx: usize,
    runner: &std::sync::Arc<dyn ProcessRunner>,
) {
    let Some(drive) = state.drive.drives.get(drive_idx) else {
        return;
    };
    if state.config.tool_path.is_empty() {
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx,
            mt1959: false,
            encrypted_firmware: false,
            identity: None,
            error: Some(t(L10nKey::ReasonNoBackend, state.chrome.resolved_lang).into()),
        });
        return;
    }

    let tx = tx.clone();
    let tool_path = state.config.tool_path.clone();
    let backend = state.config.backend;
    let device = drive.device.clone();
    let runner = runner.clone();
    let lang = state.chrome.resolved_lang;

    let _ = tx.send(WorkerMsg::Status {
        message: t(L10nKey::StatusProbing, lang).into(),
        progress: 0.0,
    });

    let control = Arc::new(OperationControl::new());
    state.runtime.probe_control = Some(control.clone());
    state.runtime.probing_drive = Some(drive_idx);

    run_backend_command(move || {
        let cmd = command::plan_drive_info(backend, &tool_path, &device);
        let _ = tx.send(WorkerMsg::Log(format!(
            "> {}",
            process::format_command(&cmd)
        )));
        match crate::orchestration::probe_drive_with(
            backend,
            &tool_path,
            &device,
            runner.as_ref(),
            Some(control.as_ref()),
        ) {
            Ok(probe) => {
                if !probe.output.is_empty() {
                    let _ = tx.send(WorkerMsg::Log(probe.output.clone()));
                }
                let _ = tx.send(WorkerMsg::ProbeComplete {
                    drive_idx,
                    mt1959: probe.safety.mt1959,
                    encrypted_firmware: probe.safety.encrypted_firmware,
                    identity: Some(probe.identity),
                    error: None,
                });
            }
            Err(crate::orchestration::ProbeError::Cancelled) => {
                let _ = tx.send(WorkerMsg::ProbeComplete {
                    drive_idx,
                    mt1959: false,
                    encrypted_firmware: false,
                    identity: None,
                    error: Some(t(L10nKey::StatusProbeFailed, lang).into()),
                });
            }
            Err(crate::orchestration::ProbeError::NeedsForceKill) => {
                let _ = tx.send(WorkerMsg::StopNeedsForceKill);
            }
            Err(crate::orchestration::ProbeError::Failed(e)) => {
                if !e.is_empty() {
                    let _ = tx.send(WorkerMsg::Log(e.clone()));
                }
                let _ = tx.send(WorkerMsg::ProbeComplete {
                    drive_idx,
                    mt1959: false,
                    encrypted_firmware: false,
                    identity: None,
                    error: Some(if e.is_empty() {
                        t(L10nKey::StatusProbeFailed, lang).into()
                    } else {
                        e
                    }),
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
    lang: Language,
    control: Arc<OperationControl>,
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

    run_backend_command(move || {
        let result = runner.run_command_streaming(
            &program,
            &args,
            &|line| {
                let _ = tx.send(WorkerMsg::Log(line.to_string()));
                if let Some(p) = process::parse_progress_percent(line) {
                    let _ = tx.send(WorkerMsg::Progress(p));
                }
            },
            Some(control.as_ref()),
        );

        match result {
            Ok(CommandRunOutcome::Completed(out)) => {
                let success = out.success();
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success,
                    status: if success {
                        t(L10nKey::StatusOpFinished, lang).into()
                    } else {
                        t(L10nKey::StatusOpFailed, lang).into()
                    },
                    progress: if success { 100.0 } else { 0.0 },
                });
            }
            Ok(CommandRunOutcome::Cancelled) => {
                let _ = tx.send(WorkerMsg::Log(t(L10nKey::LogOpCancelled, lang).into()));
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success: false,
                    status: t(L10nKey::StatusOpCancelled, lang).into(),
                    progress: 0.0,
                });
            }
            Ok(CommandRunOutcome::NeedsForceKill) => {
                let _ = tx.send(WorkerMsg::StopNeedsForceKill);
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Log(log_error(lang, &e)));
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success: false,
                    status: t(L10nKey::StatusOpFailed, lang).into(),
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
    let cmd = command::plan_drive_list(state.config.backend, &state.config.tool_path);
    let lang = state.chrome.resolved_lang;
    let control = state.begin_operation(t(L10nKey::StatusListingDrives, lang));
    state.log(&format!("> {}", process::format_command(&cmd)));

    let tx = tx.clone();
    let backend = state.config.backend;
    let tool_path = state.config.tool_path.clone();
    let runner = runner.clone();
    run_backend_command(move || {
        // Shared list path with CLI `run_list_backend` (same runner seam + success rules).
        match crate::orchestration::run_list_backend_with(
            backend,
            &tool_path,
            runner.as_ref(),
            Some(control.as_ref()),
        ) {
            Ok(out) => {
                let combined = out.combined();
                if !combined.is_empty() {
                    let _ = tx.send(WorkerMsg::Log(combined));
                }
                // Parse stdout only (stderr may contain noise).
                let drives = crate::drive::parse_drive_list(&out.stdout);
                let _ = tx.send(WorkerMsg::Log(t_with_args(
                    L10nKey::LogParsedDrivesFromOutput,
                    lang,
                    &[("count", &drives.len().to_string())],
                )));
                let _ = tx.send(WorkerMsg::DrivesListed(drives));
            }
            Err(crate::orchestration::BackendOpError::Cancelled) => {
                let _ = tx.send(WorkerMsg::Log(t(L10nKey::LogOpCancelled, lang).into()));
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success: false,
                    status: t(L10nKey::StatusOpCancelled, lang).into(),
                    progress: 0.0,
                });
            }
            Err(crate::orchestration::BackendOpError::NeedsForceKill) => {
                let _ = tx.send(WorkerMsg::StopNeedsForceKill);
            }
            Err(crate::orchestration::BackendOpError::Failed(e)) => {
                let _ = tx.send(WorkerMsg::Log(log_error(lang, &e)));
                let _ = tx.send(WorkerMsg::OperationComplete {
                    success: false,
                    status: t(L10nKey::StatusDriveListFailed, lang).into(),
                    progress: 0.0,
                });
            }
        }
    });
}

// ponytail: spawn_* share run_backend_command; ProcessRunner kept for test mocks

fn run_backend_command<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(f);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- drain_worker_messages tests ---

    use super::super::state::AppState;

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
        assert!(state.runtime.log_text.contains("hello"));
    }

    #[test]
    fn drain_progress_message() {
        let mut state = AppState::new_no_backend();
        state.runtime.progress_indeterminate = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::Progress(42.0));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(!state.runtime.progress_indeterminate);
        assert!((state.runtime.progress - 42.0).abs() < 0.01);
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
        assert_eq!(state.runtime.status_message, "Working");
        assert!((state.runtime.progress - 50.0).abs() < 0.01);
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
        assert!((state.runtime.progress - 100.0).abs() < 0.01);
    }

    #[test]
    fn drain_probe_complete_success() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 0,
            mt1959: true,
            encrypted_firmware: true,
            identity: None,
            error: None,
        });
        drop(tx);
        let (repaint, attention) = drain_worker_messages(&mut state, &rx);
        assert!(repaint);
        assert!(attention.is_none());
        assert!(!state.runtime.probing);
        assert!(state.drive.drive_mt1959);
        assert!(state.drive.drive_encrypted_firmware);
        assert!(state.drive.drive_probed);
        assert!(state.flash.encrypted_write);
        assert!(state.runtime.log_text.contains("MT1959: true"));
    }

    #[test]
    fn drain_probe_complete_error() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 0,
            mt1959: false,
            encrypted_firmware: false,
            identity: None,
            error: Some("probe failed".into()),
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(!state.runtime.probing);
        assert!(!state.drive.drive_probed);
        assert!(state.runtime.log_text.contains("probe failed"));
        assert!(state.runtime.status_message.contains("failed"));
    }

    #[test]
    fn drain_probe_complete_wrong_drive_idx() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::ProbeComplete {
            drive_idx: 1, // different from selected
            mt1959: true,
            encrypted_firmware: true,
            identity: None,
            error: None,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        // drive_mt1959 should NOT be updated since drive_idx doesn't match
        assert!(!state.drive.drive_mt1959);
        // but probing should still be cleared
        assert!(!state.runtime.probing);
    }

    #[test]
    fn drain_operation_complete_success() {
        let mut state = AppState::new_no_backend();
        state.runtime.busy = true;
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
        assert!(!state.runtime.busy);
        assert!(!state.runtime.progress_indeterminate);
        assert!(state.runtime.log_text.contains("successfully"));
    }

    #[test]
    fn drain_operation_complete_failure() {
        let mut state = AppState::new_no_backend();
        state.runtime.busy = true;
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
        assert!(!state.runtime.busy);
        assert!(!state.runtime.log_text.contains("successfully"));
    }

    #[test]
    fn drain_write_failure_shows_recovery_dialog() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = super::super::OperationMode::Write;
        state.runtime.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::OperationComplete {
            success: false,
            status: "Failed".into(),
            progress: 0.0,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(state.chrome.show_flash_failure_dialog);
    }

    #[test]
    fn drain_read_failure_does_not_show_recovery_dialog() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = super::super::OperationMode::Read;
        state.runtime.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::OperationComplete {
            success: false,
            status: "Failed".into(),
            progress: 0.0,
        });
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(!state.chrome.show_flash_failure_dialog);
    }

    #[test]
    fn drain_operation_complete_partial_progress() {
        let mut state = AppState::new_no_backend();
        state.runtime.busy = true;
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
        state.runtime.busy = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::DrivesListed(vec![test_drive()]));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert_eq!(state.drive.drives.len(), 1);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert!(!state.runtime.busy);
        assert!(!state.runtime.progress_indeterminate);
        assert!(state.runtime.log_text.contains("1 drive(s)"));
    }

    #[test]
    fn drain_drives_listed_empty() {
        let mut state = AppState::new_no_backend();
        state.drive.selected_drive = Some(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(WorkerMsg::DrivesListed(vec![]));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(state.drive.drives.is_empty());
        // selected_drive not changed to None by DrivesListed — only set to Some(0) if was None
    }

    #[test]
    fn drain_drives_listed_preserves_existing_selection() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
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
        assert_eq!(state.drive.selected_drive, Some(0));
        assert_eq!(state.drive.drives.len(), 2);
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
        assert!(state.runtime.log_text.contains("line1"));
        assert!(state.runtime.log_text.contains("line2"));
        assert!((state.runtime.progress - 50.0).abs() < 0.01); // last Status wins
        assert_eq!(state.runtime.status_message, "Working");
    }

    // --- ProcessRunner mock and spawn tests ---

    use crate::process::ProcessRunner;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Drain worker messages until `until` matches or timeout (Windows-safe vs fixed sleep).
    fn collect_worker_msgs(
        rx: &std::sync::mpsc::Receiver<WorkerMsg>,
        until: impl Fn(&WorkerMsg) -> bool,
        timeout: Duration,
    ) -> Vec<WorkerMsg> {
        let deadline = Instant::now() + timeout;
        let mut msgs = Vec::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(msg) => {
                    let done = until(&msg);
                    msgs.push(msg);
                    if done {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        msgs
    }

    fn wait_for_operation_complete(rx: &std::sync::mpsc::Receiver<WorkerMsg>) -> Vec<WorkerMsg> {
        collect_worker_msgs(
            rx,
            |m| {
                matches!(
                    m,
                    WorkerMsg::OperationComplete { .. } | WorkerMsg::StopNeedsForceKill
                )
            },
            Duration::from_secs(3),
        )
    }

    fn wait_for_probe_complete(rx: &std::sync::mpsc::Receiver<WorkerMsg>) -> Vec<WorkerMsg> {
        collect_worker_msgs(
            rx,
            |m| {
                matches!(
                    m,
                    WorkerMsg::ProbeComplete { .. } | WorkerMsg::StopNeedsForceKill
                )
            },
            Duration::from_secs(3),
        )
    }

    fn wait_for_drives_listed(rx: &std::sync::mpsc::Receiver<WorkerMsg>) -> Vec<WorkerMsg> {
        collect_worker_msgs(
            rx,
            |m| {
                matches!(
                    m,
                    WorkerMsg::DrivesListed(_)
                        | WorkerMsg::OperationComplete { .. }
                        | WorkerMsg::StopNeedsForceKill
                )
            },
            Duration::from_secs(3),
        )
    }

    enum MockOutcome {
        Success,
        Fail,
        Cancelled,
        NeedsForceKill,
        ProbeFailed,
    }

    struct MockRunner {
        output: String,
        outcome: MockOutcome,
    }

    impl MockRunner {
        fn success(output: &str) -> Self {
            Self {
                output: output.to_string(),
                outcome: MockOutcome::Success,
            }
        }

        fn failing() -> Self {
            Self {
                output: String::new(),
                outcome: MockOutcome::Fail,
            }
        }

        fn cancelled() -> Self {
            Self {
                output: String::new(),
                outcome: MockOutcome::Cancelled,
            }
        }

        fn needs_force_kill() -> Self {
            Self {
                output: String::new(),
                outcome: MockOutcome::NeedsForceKill,
            }
        }

        fn probe_failed() -> Self {
            Self {
                output: "probe failed".into(),
                outcome: MockOutcome::ProbeFailed,
            }
        }
    }

    impl ProcessRunner for MockRunner {
        fn run_command(
            &self,
            _program: &str,
            _args: &[String],
            _control: Option<&OperationControl>,
        ) -> Result<CommandRunOutcome, String> {
            match self.outcome {
                MockOutcome::Success => Ok(CommandRunOutcome::Completed(make_output(&self.output))),
                MockOutcome::Cancelled => Ok(CommandRunOutcome::Cancelled),
                MockOutcome::NeedsForceKill => Ok(CommandRunOutcome::NeedsForceKill),
                MockOutcome::Fail => Err("mock command failed".into()),
                MockOutcome::ProbeFailed => Ok(CommandRunOutcome::Completed(make_failing_output(
                    &self.output,
                ))),
            }
        }

        fn run_command_streaming(
            &self,
            _program: &str,
            _args: &[String],
            on_line: &dyn Fn(&str),
            _control: Option<&OperationControl>,
        ) -> Result<CommandRunOutcome, String> {
            match self.outcome {
                MockOutcome::Success => {
                    for line in self.output.lines() {
                        on_line(line);
                    }
                    Ok(CommandRunOutcome::Completed(make_output(&self.output)))
                }
                MockOutcome::Cancelled => Ok(CommandRunOutcome::Cancelled),
                MockOutcome::NeedsForceKill => Ok(CommandRunOutcome::NeedsForceKill),
                MockOutcome::Fail => Err("mock streaming failed".into()),
                MockOutcome::ProbeFailed => Ok(CommandRunOutcome::Completed(make_failing_output(
                    &self.output,
                ))),
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

    fn make_failing_output(stdout: &str) -> crate::process::CommandOutput {
        let status = std::process::Command::new("false").status().unwrap();
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
        spawn_probe(&tx, &mut state, 0, &runner);
        // No drive at index 0 → early return, no messages
        drop(tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn spawn_probe_empty_tool_path() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::success(""));
        spawn_probe(&tx, &mut state, 0, &runner);
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
        state.drive.drives.push(test_drive());
        state.config.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> =
            Arc::new(MockRunner::success("Vendor: HL-DT-ST\nProduct: BU40N\n"));
        spawn_probe(&tx, &mut state, 0, &runner);
        // Wait for thread to finish
        let messages = wait_for_probe_complete(&rx);
        drop(tx);
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
        state.drive.drives.push(test_drive());
        state.config.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::failing());
        spawn_probe(&tx, &mut state, 0, &runner);
        let messages = wait_for_probe_complete(&rx);
        drop(tx);
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
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        let messages = wait_for_operation_complete(&rx);
        drop(tx);
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
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        let messages = wait_for_operation_complete(&rx);
        drop(tx);
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
        assert!(state.runtime.busy);
        let messages = wait_for_drives_listed(&rx);
        drop(tx);
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
    fn drain_stop_needs_force_kill_sets_dialog() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(WorkerMsg::StopNeedsForceKill).unwrap();
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert_eq!(
            state.runtime.stop_dialog,
            super::super::state::StopDialog::ConfirmForceKill
        );
    }

    #[test]
    fn spawn_streaming_command_cancelled() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::cancelled());
        let cmd = crate::command::Command {
            program: "/bin/echo".into(),
            args: vec![],
        };
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        let messages = wait_for_operation_complete(&rx);
        drop(tx);
        let last = messages.last().unwrap();
        assert!(matches!(
            last,
            WorkerMsg::OperationComplete { success: false, .. }
        ));
    }

    #[test]
    fn spawn_streaming_command_non_success_output() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::probe_failed());
        let cmd = crate::command::Command {
            program: "/bin/echo".into(),
            args: vec![],
        };
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        let messages = wait_for_operation_complete(&rx);
        drop(tx);
        assert!(matches!(
            messages.last(),
            Some(WorkerMsg::OperationComplete { success: false, .. })
        ));
    }

    #[test]
    fn spawn_streaming_command_needs_force_kill() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::needs_force_kill());
        let cmd = crate::command::Command {
            program: "/bin/echo".into(),
            args: vec![],
        };
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut state = AppState::new_no_backend();
        drain_worker_messages(&mut state, &rx);
        assert_eq!(
            state.runtime.stop_dialog,
            super::super::state::StopDialog::ConfirmForceKill
        );
    }

    #[test]
    fn poll_waiting_backend_stop_finishes_when_child_gone() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        state.runtime.waiting_for_backend_stop = true;
        poll_worker(&mut state, &std::sync::mpsc::channel().1, None);
        assert!(!state.runtime.busy);
        assert!(!state.runtime.waiting_for_backend_stop);
        assert!(state.runtime.active_operation.is_none());
        assert_eq!(
            state.runtime.status_message,
            t(L10nKey::StatusOpCancelled, state.chrome.resolved_lang)
        );
    }

    #[test]
    fn poll_waiting_backend_stop_keeps_busy_while_child_running() {
        use std::process::Command;
        use std::time::Duration;

        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let control = Arc::new(OperationControl::new());
        control.register_child_for_test(child);
        let mut state = AppState::new_no_backend();
        state.runtime.busy = true;
        state.runtime.active_operation = Some(control.clone());
        state.runtime.waiting_for_backend_stop = true;

        poll_worker(&mut state, &std::sync::mpsc::channel().1, None);
        assert!(state.runtime.busy);
        assert!(state.runtime.waiting_for_backend_stop);

        control.request_force_kill();
        std::thread::sleep(Duration::from_millis(50));
        poll_worker(&mut state, &std::sync::mpsc::channel().1, None);
        assert!(!state.runtime.busy);
    }

    #[test]
    fn poll_probe_backend_stop_noop_when_busy() {
        let mut state = AppState::new_no_backend();
        let _ = state.begin_operation("running");
        state.runtime.probe_control = Some(Arc::new(OperationControl::new()));
        state.runtime.probing = true;
        assert!(!super::poll_probe_backend_stop(&mut state));
        assert!(state.runtime.probing);
    }

    #[test]
    fn poll_probe_backend_stop_noop_without_probe() {
        let mut state = AppState::new_no_backend();
        assert!(!super::poll_probe_backend_stop(&mut state));
    }

    #[test]
    #[cfg(unix)]
    fn poll_probe_backend_stop_waits_while_child_running() {
        use std::process::Command;

        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let control = Arc::new(OperationControl::new());
        control.register_child_for_test(child);
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing_drive = Some(0);
        state.runtime.probe_control = Some(control.clone());
        state.runtime.probing = true;

        assert!(!super::poll_probe_backend_stop(&mut state));
        assert!(state.runtime.probing);

        control.request_force_kill();
        control.reap_registered_child();
        assert!(super::poll_probe_backend_stop(&mut state));
        assert!(!state.runtime.probing);
        assert!(state.runtime.probe_control.is_none());
        assert_eq!(state.drive.last_probed_drive, Some(0));
        assert!(!state.drive.drive_probed);
    }

    #[test]
    #[cfg(unix)]
    fn poll_probe_backend_stop_finishes_when_child_exited() {
        use std::process::Command;
        use std::time::Duration;

        let child = Command::new("true").spawn().unwrap();
        let control = Arc::new(OperationControl::new());
        control.register_child_for_test(child);
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.drive.selected_drive = Some(0);
        state.runtime.probing_drive = Some(0);
        state.runtime.probe_control = Some(control);
        state.runtime.probing = true;
        std::thread::sleep(Duration::from_millis(50));

        assert!(super::poll_probe_backend_stop(&mut state));
        assert!(!state.runtime.probing);
        assert!(state.runtime.probe_control.is_none());
        assert_eq!(state.drive.last_probed_drive, Some(0));
    }

    #[test]
    #[cfg(unix)]
    fn poll_worker_finishes_probe_when_child_exited() {
        use std::process::Command;
        use std::time::Duration;

        let child = Command::new("true").spawn().unwrap();
        let control = Arc::new(OperationControl::new());
        control.register_child_for_test(child);
        let mut state = AppState::new_no_backend();
        state.runtime.probe_control = Some(control);
        state.runtime.probing = true;
        std::thread::sleep(Duration::from_millis(50));

        poll_worker(&mut state, &std::sync::mpsc::channel().1, None);
        assert!(!state.runtime.probing);
        assert!(state.runtime.probe_control.is_none());
    }

    #[test]
    fn poll_worker_without_context_drains_only() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(WorkerMsg::Status {
            message: "working".into(),
            progress: 25.0,
        })
        .unwrap();
        drop(tx);
        poll_worker(&mut state, &rx, None);
        assert!((state.runtime.progress - 25.0).abs() < 0.01);
    }

    #[test]
    fn poll_worker_with_context_drains_messages() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(WorkerMsg::Status {
            message: "working".into(),
            progress: 50.0,
        })
        .unwrap();
        drop(tx);
        let ctx = egui::Context::default();
        poll_worker(&mut state, &rx, Some(&ctx));
        assert_eq!(state.runtime.progress, 50.0);
    }

    #[test]
    fn poll_worker_with_context_requests_attention_on_success() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(WorkerMsg::OperationComplete {
            success: true,
            status: "done".into(),
            progress: 100.0,
        })
        .unwrap();
        drop(tx);
        let ctx = egui::Context::default();
        poll_worker(&mut state, &rx, Some(&ctx));
        assert_eq!(state.runtime.progress, 100.0);
    }

    #[test]
    fn poll_worker_with_context_requests_critical_attention_on_write_failure() {
        let mut state = AppState::new_no_backend();
        state.operation_mode = super::super::OperationMode::Write;
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(WorkerMsg::OperationComplete {
            success: false,
            status: "failed".into(),
            progress: 0.0,
        })
        .unwrap();
        drop(tx);
        let ctx = egui::Context::default();
        poll_worker(&mut state, &rx, Some(&ctx));
        assert!(state.chrome.show_flash_failure_dialog);
    }

    #[test]
    fn spawn_probe_cancelled() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.config.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::cancelled());
        spawn_probe(&tx, &mut state, 0, &runner);
        let messages = wait_for_probe_complete(&rx);
        drop(tx);
        assert!(matches!(
            messages.last(),
            Some(WorkerMsg::ProbeComplete { error: Some(_), .. })
        ));
    }

    #[test]
    fn spawn_probe_needs_force_kill() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.config.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::needs_force_kill());
        spawn_probe(&tx, &mut state, 0, &runner);
        assert!(state.runtime.probe_control.is_some());
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert!(state.runtime.probe_control.is_some());
        assert_eq!(
            state.runtime.stop_dialog,
            super::super::state::StopDialog::ConfirmForceKill
        );
    }

    #[test]
    fn spawn_probe_non_success_output() {
        let mut state = AppState::new_no_backend();
        state.drive.drives.push(test_drive());
        state.config.tool_path = "/usr/bin/sdftool".into();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::probe_failed());
        spawn_probe(&tx, &mut state, 0, &runner);
        let messages = wait_for_probe_complete(&rx);
        drop(tx);
        assert!(matches!(
            messages.last(),
            Some(WorkerMsg::ProbeComplete {
                error: Some(_),
                identity: None,
                ..
            })
        ));
    }

    #[test]
    fn spawn_list_drives_cancelled() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::cancelled());
        spawn_list_drives(&tx, &mut state, &runner);
        let messages = wait_for_drives_listed(&rx);
        drop(tx);
        assert!(matches!(
            messages.last(),
            Some(WorkerMsg::OperationComplete { success: false, .. })
        ));
    }

    #[test]
    fn spawn_list_drives_needs_force_kill() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::needs_force_kill());
        spawn_list_drives(&tx, &mut state, &runner);
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        drain_worker_messages(&mut state, &rx);
        assert_eq!(
            state.runtime.stop_dialog,
            super::super::state::StopDialog::ConfirmForceKill
        );
    }

    #[test]
    fn spawn_streaming_command_with_progress_line() {
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::success("PRGV:50,100,0\n"));
        let cmd = crate::command::Command {
            program: "/bin/echo".into(),
            args: vec![],
        };
        spawn_streaming_command(
            &tx,
            cmd,
            "Testing",
            &runner,
            Language::English,
            Arc::new(OperationControl::new()),
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(tx);
        let mut saw_progress = false;
        while let Ok(msg) = rx.try_recv() {
            if let WorkerMsg::Progress(p) = msg {
                assert!((p - 50.0).abs() < 0.1);
                saw_progress = true;
            }
        }
        assert!(saw_progress);
    }

    #[test]
    fn spawn_list_drives_fails() {
        let mut state = AppState::new_no_backend();
        let (tx, rx) = std::sync::mpsc::channel();
        let runner: Arc<dyn ProcessRunner> = Arc::new(MockRunner::failing());
        spawn_list_drives(&tx, &mut state, &runner);
        let messages = wait_for_drives_listed(&rx);
        drop(tx);
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
