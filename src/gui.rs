// egui-based GUI for SDFtool Flasher — stock egui native style.

use crate::command::{self, Backend, Command, Operation, PlanRequest};
use crate::drive::{self, Drive};
use crate::flash;
use crate::i18n::{self, t, t_with_args, L10nKey, Language};
use crate::manifest;
use crate::sdf;

use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const WINDOW_WIDTH: f32 = 380.0;
const WINDOW_HEIGHT: f32 = 640.0;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Signal colors for pass/fail indicators.
const COLOR_OK: egui::Color32 = egui::Color32::from_rgb(80, 200, 120);
const COLOR_FAIL: egui::Color32 = egui::Color32::from_rgb(220, 80, 80);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationMode {
    Read,
    Write,
    Recover,
}

#[derive(Debug)]
enum WorkerMsg {
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

struct AppState {
    show_settings: bool,
    show_about: bool,

    backend: Backend,
    tool_path: String,
    sdf_path: String,
    auto_detected: bool,

    drives: Vec<Drive>,
    selected_drive: Option<usize>,
    last_probed_drive: Option<usize>,

    drive_mt1959: bool,
    drive_encrypted_firmware: bool,
    drive_probed: bool,

    operation_mode: OperationMode,
    include_boot_loader: bool,
    encrypted_write: bool,

    firmware_path: String,
    firmware_candidates: Vec<String>,
    firmware_picker_items: Vec<(String, String)>,
    manifest_path: String,
    manifest: Option<manifest::FirmwareManifest>,
    firmware_data: Option<Vec<u8>>,
    selected_image_id: Option<String>,
    confirmation: String,
    flash_report: Option<flash::FlashReport>,

    recovery_token: String,
    wrong_firmware_path: String,

    status_message: String,
    progress: f32,
    progress_indeterminate: bool,
    busy: bool,
    probing: bool,
    pending_recover_browse: bool,
    log_text: String,
    show_exit_confirmation: bool,
    language: Language,
    resolved_lang: Language,
}

impl AppState {
    fn new() -> Self {
        let (backend, path, auto) = match drive::find_backend() {
            Some((b, p)) => (b, p, true),
            None => (Backend::SdfTool, String::new(), false),
        };

        Self {
            show_settings: false,
            show_about: false,
            backend,
            tool_path: path,
            sdf_path: find_sdf_bin(),
            auto_detected: auto,
            drives: Vec::new(),
            selected_drive: None,
            last_probed_drive: None,
            drive_mt1959: false,
            drive_encrypted_firmware: false,
            drive_probed: false,
            operation_mode: OperationMode::Write,
            include_boot_loader: false,
            encrypted_write: false,
            firmware_path: String::new(),
            firmware_candidates: Vec::new(),
            firmware_picker_items: Vec::new(),
            manifest_path: String::new(),
            manifest: None,
            firmware_data: None,
            selected_image_id: None,
            confirmation: String::new(),
            flash_report: None,
            recovery_token: String::new(),
            wrong_firmware_path: String::new(),
            status_message: "Ready".into(),
            progress: 0.0,
            progress_indeterminate: false,
            busy: false,
            probing: false,
            pending_recover_browse: false,
            log_text: String::new(),
            show_exit_confirmation: false,
            language: Language::Auto,
            resolved_lang: i18n::detect_system_language(),
        }
    }

    fn log(&mut self, msg: &str) {
        if !self.log_text.is_empty() {
            self.log_text.push('\n');
        }
        self.log_text.push_str(msg);
    }

    fn selected_drive(&self) -> Option<&Drive> {
        self.selected_drive.and_then(|i| self.drives.get(i))
    }

    fn set_status(&mut self, msg: impl Into<String>, progress: f32) {
        self.status_message = msg.into();
        self.progress = progress.clamp(0.0, 100.0);
    }
}

pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_resizable(true),
        ..Default::default()
    };
    eframe::run_native(
        "SDF Flash GUI",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(App::new()))
        }),
    )
}

struct App {
    state: AppState,
    worker_rx: Receiver<WorkerMsg>,
    worker_tx: Sender<WorkerMsg>,
}

impl App {
    fn new() -> Self {
        let (worker_tx, worker_rx) = mpsc::channel();
        let mut state = AppState::new();
        state.drives = drive::enumerate_drives();
        if state.drives.is_empty() {
            state.log("No optical drives detected.");
            state.set_status("No optical drives detected", 0.0);
        } else {
            state.log(&format!("Found {} drive(s).", state.drives.len()));
            state.selected_drive = Some(0);
        }
        Self {
            state,
            worker_rx,
            worker_tx,
        }
    }
}

fn handle_global_shortcuts(
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &Sender<WorkerMsg>,
) {
    ctx.input_mut(|i| {
        // Toggle settings: Cmd+, (Ctrl+,)
        let settings_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma);
        if i.consume_shortcut(&settings_shortcut) {
            state.show_settings = !state.show_settings;
        }

        // Refresh drives: Cmd+R (Ctrl+R)
        let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::R);
        if i.consume_shortcut(&refresh_shortcut) && !state.busy && !state.probing {
            refresh_drives(state);
        }

        // Close window: Cmd+W (Ctrl+W)
        let close_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
        if i.consume_shortcut(&close_shortcut) {
            if state.busy {
                state.show_exit_confirmation = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // Start flash: Cmd+Enter (Ctrl+Enter) or Enter
        let start_cmd_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
        let start_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter);

        if (i.consume_shortcut(&start_cmd_shortcut) || i.consume_shortcut(&start_shortcut))
            && !state.busy
            && !state.probing
            && can_start(state)
        {
            execute_start(state, worker_tx);
        }
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        poll_worker(self, ctx);
        handle_global_shortcuts(ctx, &mut self.state, &self.worker_tx);

        if ctx.input(|i| i.viewport().close_requested()) && self.state.busy {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.state.show_exit_confirmation = true;
        }

        if !self.state.busy
            && !self.state.probing
            && self.state.selected_drive != self.state.last_probed_drive
        {
            if let Some(idx) = self.state.selected_drive {
                spawn_probe(&self.worker_tx, &self.state, idx);
                self.state.probing = true;
            }
        }

        if self.state.pending_recover_browse {
            self.state.pending_recover_browse = false;
            prompt_recovery_wrong_firmware(&mut self.state);
        }

        let panel_frame =
            egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::same(6));
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.state.show_exit_confirmation, |ui| {
                    show_main_ui(ui, ctx, frame, &mut self.state, &self.worker_tx);
                });
            });

        if self.state.show_settings {
            show_settings_window(ctx, &mut self.state, &self.worker_tx);
        }

        if self.state.show_about {
            show_about_window(ctx, &mut self.state);
        }

        if self.state.show_exit_confirmation {
            egui::Window::new(t(L10nKey::TitleExitWarning, self.state.resolved_lang))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(t(
                                L10nKey::LabelExitWarningMsg,
                                self.state.resolved_lang,
                            ))
                            .color(COLOR_FAIL)
                            .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(t(L10nKey::LabelExitWarningDesc, self.state.resolved_lang));
                        ui.label(t(L10nKey::LabelExitWarningAsk, self.state.resolved_lang));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(40.0);
                            if ui
                                .button(t(L10nKey::BtnNoCancel, self.state.resolved_lang))
                                .clicked()
                            {
                                self.state.show_exit_confirmation = false;
                            }
                            ui.add_space(20.0);
                            if ui
                                .button(t(L10nKey::BtnYesForce, self.state.resolved_lang))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });
                });
        }
    }
}

// ── Worker message handling ─────────────────────────────────────────

fn poll_worker(app: &mut App, ctx: &egui::Context) {
    let mut repaint = false;
    while let Ok(msg) = app.worker_rx.try_recv() {
        repaint = true;
        match msg {
            WorkerMsg::Log(line) => app.state.log(&line),
            WorkerMsg::Progress(p) => {
                app.state.progress_indeterminate = false;
                app.state.progress = p;
            }
            WorkerMsg::Status { message, progress } => {
                app.state.status_message = message;
                app.state.progress = progress.clamp(0.0, 100.0);
            }
            WorkerMsg::ProbeComplete {
                drive_idx,
                mt1959,
                encrypted_firmware,
                error,
            } => {
                if app.state.selected_drive == Some(drive_idx) {
                    app.state.drive_probed = error.is_none();
                    app.state.drive_mt1959 = mt1959;
                    app.state.drive_encrypted_firmware = encrypted_firmware;
                    if error.is_none() {
                        app.state.encrypted_write = encrypted_firmware;
                    }
                    app.state.last_probed_drive = Some(drive_idx);
                }
                app.state.probing = false;
                if let Some(err) = error {
                    app.state.log(&format!("ERROR: {err}"));
                    app.state.set_status("Drive probe failed", 0.0);
                } else {
                    app.state.log(&format!(
                        "MT1959: {mt1959} | Encrypted FW: {encrypted_firmware}"
                    ));
                    app.state.set_status("Ready", 0.0);
                }
            }
            WorkerMsg::OperationComplete {
                success,
                status,
                progress,
            } => {
                app.state.busy = false;
                app.state.progress_indeterminate = false;
                app.state.set_status(status, progress);
                let is_success = success && progress >= 100.0;
                if is_success {
                    app.state.log("Operation completed successfully.");
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
                app.state.drives = drives;
                app.state.last_probed_drive = None;
                if app.state.selected_drive.is_none() && count > 0 {
                    app.state.selected_drive = Some(0);
                }
                app.state.busy = false;
                app.state.set_status("Ready", 0.0);
                app.state.log(&format!("Found {count} drive(s)."));
            }
        }
    }
    if repaint {
        ctx.request_repaint();
    }
}

// ── Worker thread spawning (unchanged business logic) ───────────────

fn spawn_probe(tx: &Sender<WorkerMsg>, state: &AppState, drive_idx: usize) {
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
        let _ = tx.send(WorkerMsg::Log(format!("> {}", format_command(&cmd))));
        match drive::run_command(&cmd.program, &cmd.args) {
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

fn spawn_streaming_command(tx: &Sender<WorkerMsg>, cmd: Command, initial_status: &str) {
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
        format_command(&Command {
            program: program.clone(),
            args: args.clone(),
        })
    )));

    thread::spawn(move || {
        let result = drive::run_command_streaming(&program, &args, |line| {
            let _ = tx.send(WorkerMsg::Log(line.to_string()));
            if let Some(p) = drive::parse_progress_percent(line) {
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

// ── UI rendering ────────────────────────────────────────────────────

fn show_main_ui(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    frame: &mut eframe::Frame,
    state: &mut AppState,
    worker_tx: &Sender<WorkerMsg>,
) {
    // ── Toolbar ──
    ui.horizontal(|ui| {
        let refresh_text = t(L10nKey::TooltipRefresh, state.resolved_lang);
        let refresh_hint = if cfg!(target_os = "macos") {
            format!("{refresh_text} (⌘R)")
        } else {
            format!("{refresh_text} (Ctrl+R)")
        };
        if ui
            .add_enabled(!state.busy && !state.probing, egui::Button::new("⟳"))
            .on_hover_text(refresh_hint)
            .clicked()
        {
            refresh_drives(state);
        }
        let settings_text = t(L10nKey::TooltipSettings, state.resolved_lang);
        let settings_hint = if cfg!(target_os = "macos") {
            format!("{settings_text} (⌘,)")
        } else {
            format!("{settings_text} (Ctrl+,)")
        };
        if ui.button("⚙").on_hover_text(settings_hint).clicked() {
            state.show_settings = true;
        }
        if ui
            .button("ℹ")
            .on_hover_text(t(L10nKey::TooltipAbout, state.resolved_lang))
            .clicked()
        {
            state.show_about = true;
        }
        if state.busy || state.probing {
            ui.spinner();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::widgets::global_theme_preference_buttons(ui);
        });
    });

    // ── Drive Properties ──
    section_heading(ui, t(L10nKey::TitleDriveProperties, state.resolved_lang));
    ui.add_space(2.0);
    ui.label(t(L10nKey::LabelDevice, state.resolved_lang));
    let no_drives_msg = t(L10nKey::StatusNoDrives, state.resolved_lang);
    let selected_label = state
        .selected_drive()
        .map(drive_label)
        .unwrap_or_else(|| no_drives_msg.to_string());
    egui::ComboBox::from_id_salt("drive_selector")
        .selected_text(&selected_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if state.drives.is_empty() {
                ui.label(no_drives_msg);
            } else {
                for (i, drive) in state.drives.iter().enumerate() {
                    ui.selectable_value(&mut state.selected_drive, Some(i), drive_label(drive));
                }
            }
        });
    ui.add_space(2.0);
    let probed = state.drive_probed && !state.probing;
    egui::Grid::new("drive_status")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label("MT1959 Platform:");
            status_indicator(ui, probed, state.drive_mt1959);
            ui.end_row();

            ui.label("Encrypted Firmware:");
            status_indicator(ui, probed, state.drive_encrypted_firmware);
            ui.end_row();
        });

    ui.add_space(6.0);

    // ── Operation Options ──
    section_heading(ui, t(L10nKey::SectionOperation, state.resolved_lang));
    ui.add_space(2.0);
    ui.label(t(L10nKey::SectionOperation, state.resolved_lang));
    let prev = state.operation_mode;
    let mode_label = match state.operation_mode {
        OperationMode::Read => t(L10nKey::TabRead, state.resolved_lang),
        OperationMode::Write => t(L10nKey::TabWrite, state.resolved_lang),
        OperationMode::Recover => t(L10nKey::TabRecover, state.resolved_lang),
    };
    egui::ComboBox::from_id_salt("operation_mode")
        .selected_text(mode_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut state.operation_mode,
                OperationMode::Read,
                t(L10nKey::TabRead, state.resolved_lang),
            );
            ui.selectable_value(
                &mut state.operation_mode,
                OperationMode::Write,
                t(L10nKey::TabWrite, state.resolved_lang),
            );
            ui.selectable_value(
                &mut state.operation_mode,
                OperationMode::Recover,
                egui::RichText::new(t(L10nKey::TabRecover, state.resolved_lang)).color(COLOR_FAIL),
            );
        });

    if state.operation_mode != prev {
        on_operation_mode_changed(state, state.operation_mode);
    }

    let write_mode = state.operation_mode == OperationMode::Write;
    if write_mode {
        ui.add_space(2.0);
        ui.checkbox(
            &mut state.include_boot_loader,
            t(L10nKey::OptionBootloader, state.resolved_lang),
        );
        ui.checkbox(
            &mut state.encrypted_write,
            t(L10nKey::OptionEncrypted, state.resolved_lang),
        );
        if state.encrypted_write && state.include_boot_loader {
            ui.colored_label(COLOR_FAIL, "⚠ Cannot combine encrypted + boot-loader");
        }
    }

    if state.operation_mode != OperationMode::Read {
        ui.add_space(4.0);
        show_firmware_selector(ui, state);
    }

    // ── Mode-specific options ──
    show_mode_specific_options(ui, state);

    ui.add_space(6.0);

    // ── Status ──
    section_heading(ui, "Status");
    ui.add_space(2.0);
    if state.busy {
        let status = format!("{}…", state.status_message.trim_end_matches('…'));
        if state.progress_indeterminate && state.progress <= 0.0 {
            ui.add(egui::ProgressBar::new(0.0).animate(true).text(status));
        } else {
            ui.add(
                egui::ProgressBar::new(state.progress / 100.0)
                    .show_percentage()
                    .text(status),
            );
        }
    } else {
        // Idle: draw an inset bar with no fill, matching egui ProgressBar height
        let desired_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
        let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let rounding = egui::CornerRadius::same(2);
        ui.painter()
            .rect_filled(rect, rounding, ui.visuals().extreme_bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "READY",
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );
    }

    // ── Action buttons ──
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let btn_size = egui::vec2(90.0, 28.0);
            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(t(L10nKey::BtnClose, state.resolved_lang)),
                )
                .clicked()
            {
                if state.busy {
                    state.show_exit_confirmation = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let _ = frame;
            }
            let start_enabled = can_start(state);
            let start_text = t(L10nKey::TooltipStartEnabled, state.resolved_lang);
            let start_hint = if cfg!(target_os = "macos") {
                format!("{start_text} (Enter / ⌘Enter)")
            } else {
                format!("{start_text} (Enter / Ctrl+Enter)")
            };
            let hover = if !start_enabled {
                start_disabled_reason(state)
            } else {
                start_hint
            };

            ui.add_enabled_ui(start_enabled, |ui| {
                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(t(L10nKey::BtnStart, state.resolved_lang)),
                    )
                    .on_disabled_hover_text(hover)
                    .clicked()
                {
                    execute_start(state, worker_tx);
                }
            });
        });
    });

    // ── Log (fills remaining space) ──
    ui.add_space(2.0);
    // Reserve space for the status bar at the very bottom
    let log_height = ui.available_height() - 20.0;
    let log_height = log_height.max(40.0);

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(log_height)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if state.log_text.is_empty() {
                ui.label(egui::RichText::new("Ready.").weak().monospace().size(11.0));
            } else {
                ui.label(egui::RichText::new(&state.log_text).monospace().size(11.0));
            }
        });

    // ── Status bar (drive count) ──
    let drive_count = state.drives.len();
    let status_text = if drive_count == 0 {
        "No drives found".to_string()
    } else {
        format!("{drive_count} drive(s) found")
    };
    ui.label(egui::RichText::new(status_text).small().weak());
}

fn status_indicator(ui: &mut egui::Ui, probed: bool, ok: bool) {
    if !probed {
        ui.label(egui::RichText::new("…").weak());
    } else if ok {
        ui.colored_label(COLOR_OK, "✓");
    } else {
        ui.colored_label(COLOR_FAIL, "✗");
    }
}

fn show_firmware_selector(ui: &mut egui::Ui, state: &mut AppState) {
    let firmware_img_text = t(L10nKey::SectionFirmwareImage, state.resolved_lang);
    let select_placeholder = format!("{}…", firmware_img_text);
    let selected = if state.firmware_path.is_empty() {
        select_placeholder
    } else {
        std::path::Path::new(&state.firmware_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| state.firmware_path.clone())
    };

    ui.label(format!("{}:", firmware_img_text));
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(t(L10nKey::BtnBrowse, state.resolved_lang))
                .clicked()
            {
                browse_firmware_file(state);
            }
            // Now the button has taken its space, we can use available_width for the rest.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if state.firmware_picker_items.len() > 1 {
                    let path_before = state.firmware_path.clone();
                    egui::ComboBox::from_id_salt("firmware_picker")
                        .selected_text(&selected)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (name, path) in &state.firmware_picker_items {
                                ui.selectable_value(&mut state.firmware_path, path.clone(), name);
                            }
                        });
                    if state.firmware_path != path_before {
                        let path = state.firmware_path.clone();
                        load_firmware(state, &path);
                    }
                } else {
                    let response = ui
                        .add_sized(
                            [ui.available_width(), ui.spacing().interact_size.y],
                            egui::Label::new(&selected).sense(egui::Sense::click()),
                        )
                        .on_hover_text(&state.firmware_path);
                    if response.clicked() {
                        browse_firmware_file(state);
                    }
                }
            });
        });
    });
    if !state.firmware_path.is_empty() && state.firmware_data.is_none() {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("⚠ Failed to load or invalid firmware file")
                    .color(egui::Color32::from_rgb(220, 50, 50))
                    .small(),
            );
        });
    }
}

fn show_mode_specific_options(ui: &mut egui::Ui, state: &mut AppState) {
    match state.operation_mode {
        OperationMode::Read => {}
        OperationMode::Write => {
            ui.add_space(4.0);
            ui.label(t(L10nKey::SectionManifest, state.resolved_lang));
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(t(L10nKey::BtnBrowse, state.resolved_lang))
                        .clicked()
                    {
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            load_manifest(state, &file.to_string_lossy());
                        }
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.manifest_path)
                                .desired_width(ui.available_width()),
                        );
                    });
                });
            });

            if let Some(manifest) = &state.manifest {
                if manifest.firmware_images.len() > 1 {
                    ui.add_space(2.0);
                    ui.label(t(L10nKey::LabelImageId, state.resolved_lang));
                    let select_img_text =
                        format!("{}…", t(L10nKey::LabelImageId, state.resolved_lang));
                    egui::ComboBox::from_id_salt("image_selector")
                        .selected_text(
                            state
                                .selected_image_id
                                .as_deref()
                                .unwrap_or(&select_img_text),
                        )
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for img in &manifest.firmware_images {
                                let label = format!(
                                    "{} · {} ({})",
                                    img.image_id, img.target_version, img.filename
                                );
                                ui.selectable_value(
                                    &mut state.selected_image_id,
                                    Some(img.image_id.clone()),
                                    label,
                                );
                            }
                        });
                }
            }

            if let Some(drive) = state.selected_drive() {
                let required = command::required_flash_confirmation(&drive.device);
                ui.add_space(4.0);
                ui.label(t_with_args(
                    L10nKey::LabelTypeToConfirm,
                    state.resolved_lang,
                    &[("required", &required)],
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut state.confirmation)
                        .desired_width(ui.available_width()),
                );
            }

            ui.add_space(4.0);
            let can_validate = state.firmware_data.is_some() && state.manifest.is_some();
            if ui
                .add_enabled(can_validate, egui::Button::new("Validate flash plan"))
                .clicked()
            {
                validate_flash(state);
            }

            if let Some(report) = &state.flash_report {
                ui.separator();
                let color = if report.would_execute {
                    COLOR_OK
                } else {
                    COLOR_FAIL
                };
                ui.colored_label(color, &report.summary);
                egui::Grid::new("checks")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        check_row(ui, "Model match", report.checks.model_match);
                        check_row(ui, "Revision check", report.checks.revision_check);
                        check_row(ui, "Image checksum", report.checks.image_checksum);
                        check_row(ui, "Signature present", report.checks.signature_present);
                        check_row(ui, "User confirmed", report.checks.user_confirmed);
                    });
            }
        }
        OperationMode::Recover => {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Token:");
                let available_width = ui.available_width() - 40.0;
                ui.add(
                    egui::TextEdit::singleline(&mut state.recovery_token)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(available_width),
                );
                let token_color = if state.recovery_token.len() == 16 {
                    COLOR_OK
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.label(
                    egui::RichText::new(format!("{}/16", state.recovery_token.len()))
                        .small()
                        .monospace()
                        .color(token_color),
                );
            });

            ui.add_space(2.0);
            ui.label(t(L10nKey::LabelWrongFw, state.resolved_lang));
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(t(L10nKey::BtnExtract, state.resolved_lang))
                        .clicked()
                    {
                        extract_recovery_token_from_wrong_firmware(state);
                    }
                    if ui
                        .button(t(L10nKey::BtnBrowse, state.resolved_lang))
                        .clicked()
                    {
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("Firmware", &["bin"])
                            .pick_file()
                        {
                            state.wrong_firmware_path = file.to_string_lossy().to_string();
                            extract_recovery_token_from_wrong_firmware(state);
                        }
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.wrong_firmware_path)
                                .desired_width(ui.available_width()),
                        );
                    });
                });
            });

            if let Some(drive) = state.selected_drive() {
                let required = command::required_flash_confirmation(&drive.device);
                ui.add_space(4.0);
                ui.label(t_with_args(
                    L10nKey::LabelTypeToConfirm,
                    state.resolved_lang,
                    &[("required", &required)],
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut state.confirmation)
                        .desired_width(ui.available_width()),
                );
            }
        }
    }
}

fn check_row(ui: &mut egui::Ui, label: &str, pass: bool) {
    let (icon, color) = if pass {
        ("✓", COLOR_OK)
    } else {
        ("✗", COLOR_FAIL)
    };
    ui.colored_label(color, icon);
    ui.label(label);
    ui.end_row();
}

fn show_about_window(ctx: &egui::Context, state: &mut AppState) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("about_viewport"),
        egui::ViewportBuilder::default()
            .with_title(t(L10nKey::TooltipAbout, state.resolved_lang))
            .with_inner_size([320.0, 180.0])
            .with_resizable(false),
        |ctx, _class| {
            let close_shortcut =
                egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
            let should_close = ctx
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                || ctx.input_mut(|i| i.consume_shortcut(&close_shortcut));
            if should_close {
                state.show_about = false;
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("SDF Flash GUI");
                    ui.add_space(8.0);
                    ui.label(t(L10nKey::AboutDescription, state.resolved_lang));
                    ui.label(t(L10nKey::AboutBuiltWith, state.resolved_lang));
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(t(
                            L10nKey::AboutAcknowledgementsTitle,
                            state.resolved_lang,
                        ))
                        .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "MakeMKV {}",
                            t(L10nKey::AboutBackendAckText, state.resolved_lang)
                        ))
                        .small(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "MartyMcNuts {}",
                            t(L10nKey::AboutCreatorAckText, state.resolved_lang)
                        ))
                        .small(),
                    );
                    ui.add_space(8.0);
                    ui.hyperlink_to(
                        egui::RichText::new("GitHub Repository").small(),
                        "https://github.com/thedavidweng/sdf-flash-gui",
                    );
                    ui.label(
                        egui::RichText::new(format!("Version {}", APP_VERSION))
                            .small()
                            .weak(),
                    );
                });
            });
        },
    );
}

// ── Settings window ─────────────────────────────────────────────────

fn validate_tool_path(path: &str, backend: Backend) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Path is empty".to_string());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if !p.is_file() {
        return Err("Path is not a file".to_string());
    }
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    match backend {
        Backend::SdfTool => {
            if !file_name.contains("sdftool") {
                return Err("Filename must contain 'sdftool'".to_string());
            }
        }
        Backend::MakeMkvCon => {
            if !file_name.contains("makemkvcon") && !file_name.contains("makemkv") {
                return Err("Filename must contain 'makemkvcon' or 'makemkv'".to_string());
            }
        }
    }
    Ok(())
}

fn validate_sdf_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Ok(());
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if !p.is_file() {
        return Err("Path is not a file".to_string());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "bin" {
        return Err("File extension must be '.bin'".to_string());
    }
    Ok(())
}

fn show_settings_window(ctx: &egui::Context, state: &mut AppState, worker_tx: &Sender<WorkerMsg>) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("settings_viewport"),
        egui::ViewportBuilder::default()
            .with_title(t(L10nKey::TitleSettings, state.resolved_lang))
            .with_inner_size([480.0, 310.0])
            .with_resizable(false),
        |ctx, _class| {
            let close_shortcut =
                egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
            let should_close = ctx
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                || ctx.input_mut(|i| i.consume_shortcut(&close_shortcut));
            if should_close {
                state.show_settings = false;
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label(t(L10nKey::LabelBackend, state.resolved_lang));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.backend, Backend::SdfTool, "SDFtool");
                        ui.selectable_value(
                            &mut state.backend,
                            Backend::MakeMkvCon,
                            "MakeMKV (makemkvcon)",
                        );
                    });

                    ui.add_space(8.0);

                    egui::Grid::new("settings_grid")
                        .num_columns(2)
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            // Tool path row
                            ui.label(t(L10nKey::LabelToolPath, state.resolved_lang));
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.tool_path)
                                        .desired_width(220.0),
                                );
                                if ui
                                    .button(t(L10nKey::BtnBrowse, state.resolved_lang))
                                    .clicked()
                                {
                                    if let Some(file) = rfd::FileDialog::new().pick_file() {
                                        state.tool_path = file.to_string_lossy().to_string();
                                        state.auto_detected = false;
                                        state.last_probed_drive = None;
                                    }
                                }
                                if ui.button("Auto-detect").clicked() {
                                    if let Some((b, p)) = drive::find_backend() {
                                        state.backend = b;
                                        state.tool_path = p;
                                        state.auto_detected = true;
                                        state.last_probed_drive = None;
                                    }
                                }
                            });
                            ui.end_row();

                            // Tool path validation status
                            ui.label("");
                            if let Err(e) = validate_tool_path(&state.tool_path, state.backend) {
                                ui.label(
                                    egui::RichText::new(format!("⚠ {e}"))
                                        .color(egui::Color32::from_rgb(220, 50, 50))
                                        .small(),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("✓ Path is valid")
                                        .color(egui::Color32::from_rgb(50, 180, 50))
                                        .small(),
                                );
                            }
                            ui.end_row();

                            // sdf.bin row
                            ui.label(t(L10nKey::LabelSdfPath, state.resolved_lang));
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.sdf_path)
                                        .desired_width(220.0),
                                );
                                if ui
                                    .button(t(L10nKey::BtnBrowse, state.resolved_lang))
                                    .clicked()
                                {
                                    if let Some(file) = rfd::FileDialog::new()
                                        .add_filter("SDF", &["bin"])
                                        .pick_file()
                                    {
                                        state.sdf_path = file.to_string_lossy().to_string();
                                    }
                                }
                                if ui.button("Auto-detect").clicked() {
                                    state.sdf_path = find_sdf_bin();
                                }
                            });
                            ui.end_row();

                            // sdf.bin validation status
                            ui.label("");
                            if let Err(e) = validate_sdf_path(&state.sdf_path) {
                                ui.label(
                                    egui::RichText::new(format!("⚠ {e}"))
                                        .color(egui::Color32::from_rgb(220, 50, 50))
                                        .small(),
                                );
                            } else if !state.sdf_path.is_empty() {
                                ui.label(
                                    egui::RichText::new("✓ Path is valid")
                                        .color(egui::Color32::from_rgb(50, 180, 50))
                                        .small(),
                                );
                            } else {
                                ui.label(egui::RichText::new("Optional").small().weak());
                            }
                            ui.end_row();
                        });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(t(L10nKey::LabelLanguage, state.resolved_lang));
                    let prev_lang = state.language;
                    egui::ComboBox::from_id_salt("language_selector")
                        .selected_text(state.language.display_name())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for lang in Language::ALL {
                                ui.selectable_value(
                                    &mut state.language,
                                    *lang,
                                    lang.display_name(),
                                );
                            }
                        });
                    if state.language != prev_lang {
                        state.resolved_lang = i18n::resolve_language(state.language);
                    }
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !state.busy,
                            egui::Button::new(t(L10nKey::BtnListDrives, state.resolved_lang)),
                        )
                        .clicked()
                    {
                        spawn_list_drives(worker_tx, state);
                    }

                    if ui
                        .button(t(L10nKey::BtnParseSdf, state.resolved_lang))
                        .clicked()
                    {
                        match std::fs::read(&state.sdf_path) {
                            Ok(data) => {
                                let mut cursor = std::io::Cursor::new(&data);
                                match sdf::parse_sdf0(&mut cursor) {
                                    Ok(container) => {
                                        let mut info = format!(
                                            "SDF0 v{} | header_size={} | payload_offset={}",
                                            container.header.version,
                                            container.header.header_size,
                                            container.payload.offset,
                                        );
                                        if let Some(v) = &container.metadata.vendor {
                                            info.push_str(&format!("\n  Vendor: {v}"));
                                        }
                                        if let Some(m) = &container.metadata.model {
                                            info.push_str(&format!("\n  Model: {m}"));
                                        }
                                        if let Some(fw) = &container.metadata.firmware_version {
                                            info.push_str(&format!("\n  Firmware: {fw}"));
                                        }
                                        info.push_str(&format!(
                                            "\n  Encrypted: {} | Compressed: {}",
                                            container.payload.encrypted,
                                            container.payload.compressed
                                        ));
                                        for (k, v) in &container.metadata.extra {
                                            info.push_str(&format!("\n  {k}: {v}"));
                                        }
                                        state.log(&info);
                                    }
                                    Err(e) => state.log(&format!("ERROR: {e}")),
                                }
                            }
                            Err(e) => state.log(&format!("ERROR: cannot read sdf.bin: {e}")),
                        }
                    }
                });
            });
        },
    );
}

// ── Pure helpers (unchanged business logic) ─────────────────────────

fn on_operation_mode_changed(state: &mut AppState, mode: OperationMode) {
    state.flash_report = None;
    state.confirmation.clear();
    match mode {
        OperationMode::Read => {
            state.set_status("Select output folder when you start", 0.0);
        }
        OperationMode::Write => {
            state.set_status("Load firmware and manifest, then validate", 0.0);
        }
        OperationMode::Recover => {
            state.set_status("Recovery needs boot token from wrong firmware", 0.0);
            state.pending_recover_browse = true;
        }
    }
}

fn prompt_recovery_wrong_firmware(state: &mut AppState) {
    if !state.wrong_firmware_path.is_empty() {
        return;
    }
    state.log("RECOVER: select the wrong firmware file to extract boot token");
    if let Some(file) = rfd::FileDialog::new()
        .set_title("Wrong firmware (for token extraction)")
        .add_filter("Firmware", &["bin"])
        .pick_file()
    {
        state.wrong_firmware_path = file.to_string_lossy().to_string();
        extract_recovery_token_from_wrong_firmware(state);
    }
}

fn extract_recovery_token_from_wrong_firmware(state: &mut AppState) {
    if state.wrong_firmware_path.is_empty() {
        return;
    }
    match std::fs::read(&state.wrong_firmware_path) {
        Ok(data) => match command::extract_recovery_boot_token(&data) {
            Ok(token) => {
                state.recovery_token = token.clone();
                state.log(&format!("Extracted recovery boot token: {token}"));
            }
            Err(e) => state.log(&format!("ERROR: {e}")),
        },
        Err(e) => state.log(&format!(
            "ERROR: cannot read {}: {e}",
            state.wrong_firmware_path
        )),
    }
}

fn browse_firmware_file(state: &mut AppState) {
    let mut dialog = rfd::FileDialog::new().add_filter("Firmware", &["bin"]);
    if !state.firmware_path.is_empty() {
        if let Some(parent) = std::path::Path::new(&state.firmware_path).parent() {
            dialog = dialog.set_directory(parent);
        }
    }
    if let Some(file) = dialog.pick_file() {
        load_firmware(state, &file.to_string_lossy());
    }
}

fn drive_label(drive: &Drive) -> String {
    if drive.vendor.is_empty() {
        drive.device.clone()
    } else {
        format!(
            "{} {} {} {} {}",
            drive.device,
            drive.vendor,
            drive.product,
            drive.revision,
            drive_serial_hint(drive)
        )
        .trim()
        .to_string()
    }
}

fn drive_serial_hint(drive: &Drive) -> String {
    let label = format!("{}_{}_{}", drive.vendor, drive.product, drive.revision);
    label
        .split(['_', '-', ' '])
        .skip(2)
        .collect::<Vec<_>>()
        .join(" ")
}

fn refresh_drives(state: &mut AppState) {
    state.drives = drive::enumerate_drives();
    state.last_probed_drive = None;
    state.log(&format!("Found {} drive(s).", state.drives.len()));
    if state.drives.is_empty() {
        state.selected_drive = None;
        state.set_status("No optical drives detected", 0.0);
    } else if state.selected_drive.is_none() {
        state.selected_drive = Some(0);
        state.set_status("Ready", 0.0);
    }
}

fn load_firmware(state: &mut AppState, path: &str) {
    state.firmware_path = path.to_string();
    state.flash_report = None;
    match std::fs::read(path) {
        Ok(data) => {
            if data.is_empty() {
                state.log(&format!("ERROR: firmware file is empty: {path}"));
                state.firmware_data = None;
            } else {
                state.firmware_data = Some(data);
            }
        }
        Err(e) => {
            state.log(&format!("ERROR: cannot read firmware file {path}: {e}"));
            state.firmware_data = None;
        }
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        state.firmware_candidates = std::fs::read_dir(parent)
            .map(|entries| {
                let mut files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "bin"))
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                files.sort();
                files
            })
            .unwrap_or_default();
    }

    state.firmware_picker_items = state
        .firmware_candidates
        .iter()
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| path.clone());
            (name, path.clone())
        })
        .collect();

    if let Some(data) = &state.firmware_data {
        state.log(&format!(
            "Loaded firmware: {} ({} bytes, sha256 {})",
            path,
            data.len(),
            &flash::sha256_hex(data)[..16]
        ));
    }
}

fn load_manifest(state: &mut AppState, path: &str) {
    state.manifest_path = path.to_string();
    match std::fs::read(path) {
        Ok(data) => match manifest::parse_manifest(&data) {
            Ok(m) => {
                state.log(&format!(
                    "Loaded manifest: {} {} ({} image(s))",
                    m.vendor,
                    m.model,
                    m.firmware_images.len()
                ));
                state.selected_image_id = if m.firmware_images.len() == 1 {
                    Some(m.firmware_images[0].image_id.clone())
                } else {
                    None
                };
                state.manifest = Some(m);
                state.flash_report = None;
            }
            Err(e) => state.log(&format!("ERROR: invalid manifest: {e}")),
        },
        Err(e) => state.log(&format!("ERROR: cannot read manifest: {e}")),
    }
}

fn can_start(state: &AppState) -> bool {
    if state.busy || state.probing || state.selected_drive().is_none() || !state.drive_mt1959 {
        return false;
    }
    if validate_tool_path(&state.tool_path, state.backend).is_err() {
        return false;
    }
    if validate_sdf_path(&state.sdf_path).is_err() {
        return false;
    }
    match state.operation_mode {
        OperationMode::Read => true,
        OperationMode::Write => {
            state.firmware_data.is_some()
                && !state.firmware_path.is_empty()
                && !(state.encrypted_write && state.include_boot_loader)
                && state.flash_report.as_ref().is_some_and(|r| r.would_execute)
        }
        OperationMode::Recover => {
            !state.firmware_path.is_empty()
                && state.recovery_token.len() == 16
                && state.confirmation
                    == state
                        .selected_drive()
                        .map(|d| command::required_flash_confirmation(&d.device))
                        .unwrap_or_default()
        }
    }
}

fn start_disabled_reason(state: &AppState) -> String {
    let lang = state.resolved_lang;
    if state.busy {
        return t(L10nKey::ReasonBusy, lang).to_string();
    }
    if state.probing {
        return t(L10nKey::ReasonProbing, lang).to_string();
    }
    if state.selected_drive().is_none() {
        return t(L10nKey::ReasonNoDrive, lang).to_string();
    }
    if !state.drive_mt1959 {
        return t(L10nKey::ReasonNotMt1959, lang).to_string();
    }
    if let Err(e) = validate_tool_path(&state.tool_path, state.backend) {
        return format!("Invalid tool path: {e}");
    }
    if let Err(e) = validate_sdf_path(&state.sdf_path) {
        return format!("Invalid sdf.bin: {e}");
    }
    match state.operation_mode {
        OperationMode::Read => String::new(),
        OperationMode::Write => {
            if state.firmware_data.is_none() {
                return t(L10nKey::ReasonNoFirmware, lang).to_string();
            }
            if state.encrypted_write && state.include_boot_loader {
                return t(L10nKey::ReasonConflict, lang).to_string();
            }
            t(L10nKey::ReasonRunValidation, lang).to_string()
        }
        OperationMode::Recover => t(L10nKey::ReasonEnterToken, lang).to_string(),
    }
}

fn begin_operation(state: &mut AppState, status: &str) {
    state.busy = true;
    state.progress_indeterminate = true;
    state.progress = 0.0;
    state.set_status(status, 0.0);
}

fn spawn_list_drives(tx: &Sender<WorkerMsg>, state: &mut AppState) {
    let cmd = command::plan_drive_list(state.backend, &state.tool_path);
    begin_operation(state, "Listing drives");
    state.log(&format!("> {}", format_command(&cmd)));

    let tx = tx.clone();
    let program = cmd.program;
    let args = cmd.args;
    thread::spawn(move || match drive::run_command(&program, &args) {
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

fn execute_start(state: &mut AppState, worker_tx: &Sender<WorkerMsg>) {
    match state.operation_mode {
        OperationMode::Read => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            let output_dir = folder.to_string_lossy().to_string();
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: String::new(),
                operation: Operation::Read { output_dir },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Reading firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Reading firmware");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
        OperationMode::Write => {
            validate_flash(state);
            if !state.flash_report.as_ref().is_some_and(|r| r.would_execute) {
                return;
            }
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: state.confirmation.clone(),
                operation: Operation::Write {
                    firmware_path: state.firmware_path.clone(),
                    encrypted: state.encrypted_write,
                    include_boot_loader: state.include_boot_loader,
                },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Writing firmware");
                    spawn_streaming_command(worker_tx, plan.command, "Writing firmware");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
        OperationMode::Recover => {
            let Some(drive) = state.selected_drive() else {
                return;
            };
            let req = PlanRequest {
                backend: state.backend,
                tool_path: state.tool_path.clone(),
                drive: drive.device.clone(),
                drive_is_mt1959: state.drive_mt1959,
                confirmation: state.confirmation.clone(),
                operation: Operation::Recover {
                    firmware_path: state.firmware_path.clone(),
                    recovery_boot_token: state.recovery_token.clone(),
                },
            };
            match command::plan_command(req) {
                Ok(plan) => {
                    begin_operation(state, "Recovering drive");
                    spawn_streaming_command(worker_tx, plan.command, "Recovering drive");
                }
                Err(e) => state.log(&format!("ERROR: {e}")),
            }
        }
    }
}

fn validate_flash(state: &mut AppState) {
    let drive = match state.selected_drive() {
        Some(d) => d,
        None => return,
    };
    let manifest = match &state.manifest {
        Some(m) => m,
        None => {
            state.log("ERROR: load a manifest before validating");
            return;
        }
    };
    let firmware_data = match &state.firmware_data {
        Some(d) => d,
        None => return,
    };

    let image_id = match &state.selected_image_id {
        Some(id) => id.clone(),
        None => {
            state.log("ERROR: select an image before validating");
            return;
        }
    };

    let request = flash::FlashPlanRequest {
        image_id: &image_id,
        current_version: &drive.revision,
        firmware_size: firmware_data.len() as u64,
        firmware_sha256: &flash::sha256_hex(firmware_data),
        signature_present: manifest
            .firmware_images
            .iter()
            .find(|i| i.image_id == image_id)
            .map(|i| i.signature_present)
            .unwrap_or(false),
        user_confirmed: state.confirmation == command::required_flash_confirmation(&drive.device),
    };

    match flash::build_flash_plan(manifest, &drive.into(), request) {
        Ok(plan) => {
            let report = flash::dry_run(&plan);
            state.log(&report.summary);
            state.flash_report = Some(report);
        }
        Err(e) => {
            state.log(&format!("Validation failed: {e}"));
            state.flash_report = None;
        }
    }
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

fn format_command(cmd: &command::Command) -> String {
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

fn find_sdf_bin() -> String {
    let candidates = ["./sdf.bin", "../sdf.bin", "/usr/share/sdftool/sdf.bin"];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let paths = [
            format!("{home}/.MakeMKV/sdf.bin"),
            "/Library/MakeMKV/sdf.bin".to_string(),
            "/opt/homebrew/share/sdftool/sdf.bin".to_string(),
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                return p.clone();
            }
        }
    }

    String::new()
}

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(text).strong().size(15.0));
        ui.add_space(4.0);
        let available_width = ui.available_width();
        if available_width > 0.0 {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(available_width, 1.0), egui::Sense::hover());
            let line_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
            ui.painter()
                .hline(rect.x_range(), rect.center().y, (1.0, line_color));
        }
    });
    ui.add_space(4.0);
}

impl From<&Drive> for manifest::DriveMatch {
    fn from(d: &Drive) -> Self {
        manifest::DriveMatch {
            vendor: d.vendor.clone(),
            model: d.product.clone(),
            revision: d.revision.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{t, L10nKey, Language};

    #[test]
    fn test_credits_html_matches_gui_about() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let credits_path = manifest_dir.join("assets/Credits.html");
        assert!(
            credits_path.exists(),
            "Credits.html does not exist at {:?}",
            credits_path
        );

        let html = std::fs::read_to_string(&credits_path).expect("Failed to read Credits.html");

        // Verify that the HTML content contains the exact descriptions and strings from L10n system
        // dynamically, ensuring the single source of truth is not hardcoded here.
        let desc = t(L10nKey::AboutDescription, Language::English);
        let built_with = t(L10nKey::AboutBuiltWith, Language::English);
        let ack_title = t(L10nKey::AboutAcknowledgementsTitle, Language::English);
        let clean_title = ack_title.trim_end_matches(':');
        let mkv_text = t(L10nKey::AboutBackendAckText, Language::English);
        let creator_text = t(L10nKey::AboutCreatorAckText, Language::English);

        assert!(
            html.contains(desc),
            "Credits.html is missing description: {}",
            desc
        );
        assert!(
            html.contains(built_with),
            "Credits.html is missing built_with: {}",
            built_with
        );
        assert!(
            html.contains(clean_title),
            "Credits.html is missing acknowledgements title: {}",
            clean_title
        );

        // MakeMKV acknowledgement (handles <b> tag wrapper in HTML)
        let mkv_html = format!("<b>MakeMKV</b> {}", mkv_text);
        assert!(
            html.contains(&mkv_html),
            "Backend ack mismatch in HTML. Expected to find: {}",
            mkv_html
        );

        // MartyMcNuts acknowledgement (handles <b> tag wrapper in HTML)
        let marty_html = format!("<b>MartyMcNuts</b> {}", creator_text);
        assert!(
            html.contains(&marty_html),
            "Creator ack mismatch in HTML. Expected to find: {}",
            marty_html
        );

        // GitHub Repository URL link
        let link_html = "href=\"https://github.com/thedavidweng/sdf-flash-gui\"";
        assert!(
            html.contains(link_html),
            "GitHub URL link mismatch in HTML. Expected to find: {}",
            link_html
        );
    }

    #[test]
    fn test_validate_tool_path() {
        use super::{validate_tool_path, Backend};

        // Empty path validation should return an error
        assert!(validate_tool_path("", Backend::SdfTool).is_err());
        assert!(validate_tool_path("   ", Backend::SdfTool).is_err());

        // Create a temporary file to test path validation
        let temp_dir = std::env::temp_dir();

        // Correct filename for SdfTool
        let sdftool_file = temp_dir.join("test_sdftool_temp");
        std::fs::write(&sdftool_file, b"").unwrap();
        assert!(validate_tool_path(&sdftool_file.to_string_lossy(), Backend::SdfTool).is_ok());
        // Wrong backend type check
        assert!(validate_tool_path(&sdftool_file.to_string_lossy(), Backend::MakeMkvCon).is_err());
        let _ = std::fs::remove_file(&sdftool_file);

        // Correct filename for MakeMkvCon
        let makemkv_file = temp_dir.join("test_makemkvcon_temp");
        std::fs::write(&makemkv_file, b"").unwrap();
        assert!(validate_tool_path(&makemkv_file.to_string_lossy(), Backend::MakeMkvCon).is_ok());
        // Wrong backend type check
        assert!(validate_tool_path(&makemkv_file.to_string_lossy(), Backend::SdfTool).is_err());
        let _ = std::fs::remove_file(&makemkv_file);

        // Non-existent path
        let non_existent = temp_dir.join("does-not-exist-sdftool");
        assert!(validate_tool_path(&non_existent.to_string_lossy(), Backend::SdfTool).is_err());
    }

    #[test]
    fn test_validate_sdf_path() {
        use super::validate_sdf_path;

        // Empty sdf.bin path is valid (it is optional)
        assert!(validate_sdf_path("").is_ok());
        assert!(validate_sdf_path("  ").is_ok());

        let temp_dir = std::env::temp_dir();

        // Path is valid if it ends with .bin and exists
        let bin_file = temp_dir.join("test_sdf_temp.bin");
        std::fs::write(&bin_file, b"").unwrap();
        assert!(validate_sdf_path(&bin_file.to_string_lossy()).is_ok());
        let _ = std::fs::remove_file(&bin_file);

        // Path is invalid if it has wrong extension
        let txt_file = temp_dir.join("test_sdf_temp.txt");
        std::fs::write(&txt_file, b"").unwrap();
        assert!(validate_sdf_path(&txt_file.to_string_lossy()).is_err());
        let _ = std::fs::remove_file(&txt_file);

        // Non-existent path
        let non_existent = temp_dir.join("does-not-exist.bin");
        assert!(validate_sdf_path(&non_existent.to_string_lossy()).is_err());
    }

    #[test]
    fn test_app_state() {
        use super::{AppState, Drive};

        let mut state = AppState::new();
        assert_eq!(state.status_message, "Ready");
        assert_eq!(state.progress, 0.0);
        assert!(!state.busy);
        assert!(state.log_text.is_empty());

        // Test logging
        state.log("Hello");
        assert_eq!(state.log_text, "Hello");
        state.log("World");
        assert_eq!(state.log_text, "Hello\nWorld");

        // Test set_status
        state.set_status("Working...", 50.0);
        assert_eq!(state.status_message, "Working...");
        assert_eq!(state.progress, 50.0);

        // Test progress clamping
        state.set_status("Overworking...", 120.0);
        assert_eq!(state.progress, 100.0);
        state.set_status("Underworking...", -10.0);
        assert_eq!(state.progress, 0.0);

        // Test selected_drive
        assert!(state.selected_drive().is_none());
        let mock_drive = Drive {
            device: "/dev/mock_device".to_string(),
            vendor: "MockVendor".to_string(),
            product: "MockProduct".to_string(),
            revision: "1.00".to_string(),
        };
        state.drives.push(mock_drive.clone());
        state.selected_drive = Some(0);

        let selected = state.selected_drive().unwrap();
        assert_eq!(selected.device, "/dev/mock_device");
        assert_eq!(selected.vendor, "MockVendor");
        assert_eq!(selected.product, "MockProduct");
        assert_eq!(selected.revision, "1.00");

        state.selected_drive = Some(1); // out of bounds
        assert!(state.selected_drive().is_none());
    }
}
