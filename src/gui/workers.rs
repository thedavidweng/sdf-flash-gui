// Worker message types, dispatch, and thread spawning.

use crate::command::{self, Command};
use crate::drive::Drive;
use crate::process;

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

/// Drain the worker channel and apply each message to app state.
pub fn poll_worker(
    state: &mut AppState,
    worker_rx: &std::sync::mpsc::Receiver<WorkerMsg>,
    ctx: &egui::Context,
) {
    let mut repaint = false;
    while let Ok(msg) = worker_rx.try_recv() {
        repaint = true;
        match msg {
            WorkerMsg::Log(line) => state.log(&line),
            WorkerMsg::Progress(p) => {
                state.progress_indeterminate = false;
                state.progress = p;
            }
            WorkerMsg::Status { message, progress } => {
                state.status_message = message;
                state.progress = progress.clamp(0.0, 100.0);
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

                let attention_type = if is_success {
                    egui::UserAttentionType::Informational
                } else {
                    egui::UserAttentionType::Critical
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(attention_type));
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
            }
        }
    }
    if repaint {
        ctx.request_repaint();
    }
}

pub fn spawn_probe(tx: &Sender<WorkerMsg>, state: &AppState, drive_idx: usize) {
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
        match process::run_command(&cmd.program, &cmd.args) {
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

pub fn spawn_streaming_command(tx: &Sender<WorkerMsg>, cmd: Command, initial_status: &str) {
    let tx = tx.clone();
    let program = cmd.program;
    let args = cmd.args;
    let initial_status = initial_status.to_string();

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
        let result = process::run_command_streaming(&program, &args, |line| {
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

pub fn spawn_list_drives(tx: &Sender<WorkerMsg>, state: &mut AppState) {
    let cmd = command::plan_drive_list(state.backend, &state.tool_path);
    state.begin_operation("Listing drives");
    state.log(&format!("> {}", process::format_command(&cmd)));

    let tx = tx.clone();
    let program = cmd.program;
    let args = cmd.args;
    thread::spawn(move || match process::run_command(&program, &args) {
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
