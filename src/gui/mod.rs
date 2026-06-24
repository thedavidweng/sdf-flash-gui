// egui-based GUI for SDFtool Flasher — stock egui native style.
//
// This module owns the UI rendering and the eframe::App lifecycle.
// Business logic lives in ops.rs, workers in workers.rs, state in state.rs.

mod ops;
mod state;
mod validation;
mod workers;

use crate::command::{self, Backend};
use crate::drive;
use crate::i18n::{self, t, t_with_args, L10nKey, Language};
use crate::sdf;

use eframe::egui;
use std::sync::mpsc;

use state::AppState;
use workers::{spawn_list_drives, spawn_probe, WorkerMsg};

const WINDOW_WIDTH: f32 = 380.0;
const WINDOW_HEIGHT: f32 = 640.0;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Read,
    Write,
    Recover,
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
    worker_rx: mpsc::Receiver<WorkerMsg>,
    worker_tx: mpsc::Sender<WorkerMsg>,
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
    worker_tx: &mpsc::Sender<WorkerMsg>,
) {
    ctx.input_mut(|i| {
        let settings_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma);
        if i.consume_shortcut(&settings_shortcut) {
            state.show_settings = !state.show_settings;
        }

        let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::R);
        if i.consume_shortcut(&refresh_shortcut) && !state.busy && !state.probing {
            ops::refresh_drives(state);
        }

        let close_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
        if i.consume_shortcut(&close_shortcut) {
            if state.busy {
                state.show_exit_confirmation = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let start_cmd_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
        let start_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter);

        if (i.consume_shortcut(&start_cmd_shortcut) || i.consume_shortcut(&start_shortcut))
            && !state.busy
            && !state.probing
            && ops::can_start(state)
        {
            ops::execute_start(state, worker_tx);
        }
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        workers::poll_worker(&mut self.state, &self.worker_rx, ctx);
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
            ops::prompt_recovery_wrong_firmware(&mut self.state);
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
                            .color(ui.visuals().error_fg_color)
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

// ── UI rendering ────────────────────────────────────────────────────

fn show_main_ui(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    frame: &mut eframe::Frame,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
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
            ops::refresh_drives(state);
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
        .map(ops::drive_label)
        .unwrap_or_else(|| no_drives_msg.to_string());
    egui::ComboBox::from_id_salt("drive_selector")
        .selected_text(&selected_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if state.drives.is_empty() {
                ui.label(no_drives_msg);
            } else {
                for (i, drive) in state.drives.iter().enumerate() {
                    ui.selectable_value(
                        &mut state.selected_drive,
                        Some(i),
                        ops::drive_label(drive),
                    );
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
                egui::RichText::new(t(L10nKey::TabRecover, state.resolved_lang))
                    .color(ui.visuals().error_fg_color),
            );
        });

    if state.operation_mode != prev {
        ops::on_operation_mode_changed(state, state.operation_mode);
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
            ui.colored_label(
                ui.visuals().error_fg_color,
                "⚠ Cannot combine encrypted + boot-loader",
            );
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
        ui.add(
            egui::ProgressBar::new(0.0)
                .fill(egui::Color32::TRANSPARENT)
                .text("READY"),
        );
    }

    // ── Action buttons ──
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(t(L10nKey::BtnClose, state.resolved_lang))
                .clicked()
            {
                if state.busy {
                    state.show_exit_confirmation = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let _ = frame;
            }
            let start_enabled = ops::can_start(state);
            let start_text = t(L10nKey::TooltipStartEnabled, state.resolved_lang);
            let start_hint = if cfg!(target_os = "macos") {
                format!("{start_text} (Enter / ⌘Enter)")
            } else {
                format!("{start_text} (Enter / Ctrl+Enter)")
            };
            let hover = if !start_enabled {
                ops::start_disabled_reason(state)
            } else {
                start_hint
            };

            ui.add_enabled_ui(start_enabled, |ui| {
                if ui
                    .button(t(L10nKey::BtnStart, state.resolved_lang))
                    .on_disabled_hover_text(hover)
                    .clicked()
                {
                    ops::execute_start(state, worker_tx);
                }
            });
        });
    });

    // ── Log (fills remaining space) ──
    ui.add_space(2.0);
    let log_height = ui.available_height() - 20.0;
    let log_height = log_height.max(40.0);

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(log_height)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if state.log_text.is_empty() {
                ui.weak("Ready.");
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
        ui.weak("…");
    } else if ok {
        ui.colored_label(ui.visuals().hyperlink_color, "✓");
    } else {
        ui.colored_label(ui.visuals().error_fg_color, "✗");
    }
}

fn show_firmware_selector(ui: &mut egui::Ui, state: &mut AppState) {
    let firmware_img_text = t(L10nKey::SectionFirmwareImage, state.resolved_lang);
    let selected = if state.firmware_path.is_empty() {
        "Select firmware .bin…".to_string()
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
                ops::browse_firmware_file(state);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if state.firmware_picker_items.len() > 1 {
                    let path_before = state.firmware_path.clone();
                    egui::ComboBox::from_id_salt("firmware_picker")
                        .selected_text(&selected)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (name, path) in &state.firmware_picker_items {
                                ui.selectable_value(
                                    &mut state.firmware_path,
                                    path.clone(),
                                    name,
                                );
                            }
                        });
                    if state.firmware_path != path_before {
                        let path = state.firmware_path.clone();
                        ops::load_firmware(state, &path);
                    }
                } else {
                    let response = ui
                        .add_sized(
                            [ui.available_width(), ui.spacing().interact_size.y],
                            egui::Label::new(&selected).sense(egui::Sense::click()),
                        )
                        .on_hover_text(&state.firmware_path);
                    if response.clicked() {
                        ops::browse_firmware_file(state);
                    }
                }
            });
        });
    });
    if !state.firmware_path.is_empty() && state.firmware_data.is_none() {
        ui.add_space(2.0);
        ui.colored_label(
            ui.visuals().error_fg_color,
            "⚠ Failed to load or invalid firmware file",
        );
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
                            ops::load_manifest(state, &file.to_string_lossy());
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
                ops::validate_flash(state);
            }

            if let Some(report) = &state.flash_report {
                ui.separator();
                let color = if report.would_execute {
                    ui.visuals().hyperlink_color
                } else {
                    ui.visuals().error_fg_color
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
                    ui.visuals().hyperlink_color
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
                        ops::extract_recovery_token_from_wrong_firmware(state);
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
                            ops::extract_recovery_token_from_wrong_firmware(state);
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
    if pass {
        ui.colored_label(ui.visuals().hyperlink_color, "✓");
    } else {
        ui.colored_label(ui.visuals().error_fg_color, "✗");
    }
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
                    ui.add_space(4.0);
                    ui.label(t(L10nKey::AboutDescription, state.resolved_lang));
                    ui.label(t(L10nKey::AboutBuiltWith, state.resolved_lang));
                });
                ui.separator();
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.strong(t(L10nKey::AboutAcknowledgementsTitle, state.resolved_lang));
                        ui.small(format!(
                            "MakeMKV {}",
                            t(L10nKey::AboutBackendAckText, state.resolved_lang)
                        ));
                        ui.small(format!(
                            "MartyMcNuts {}",
                            t(L10nKey::AboutCreatorAckText, state.resolved_lang)
                        ));
                        ui.add_space(4.0);
                        ui.hyperlink_to(
                            "GitHub Repository",
                            "https://github.com/thedavidweng/sdf-flash-gui",
                        );
                        ui.weak(format!("Version {}", APP_VERSION));
                    });
                });
            });
        },
    );
}

// ── Settings window ─────────────────────────────────────────────────

fn show_settings_window(
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
) {
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

                            ui.label("");
                            if let Err(e) =
                                validation::validate_tool_path(&state.tool_path, state.backend)
                            {
                                ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {e}"));
                            } else {
                                ui.colored_label(ui.visuals().hyperlink_color, "✓ Path is valid");
                            }
                            ui.end_row();

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
                                    state.sdf_path = state::find_sdf_bin();
                                }
                            });
                            ui.end_row();

                            ui.label("");
                            if let Err(e) = validation::validate_sdf_path(&state.sdf_path) {
                                ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {e}"));
                            } else if !state.sdf_path.is_empty() {
                                ui.colored_label(ui.visuals().hyperlink_color, "✓ Path is valid");
                            } else {
                                ui.weak("Optional");
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

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.heading(text);
    ui.separator();
    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let mkv_html = format!("<b>MakeMKV</b> {}", mkv_text);
        assert!(
            html.contains(&mkv_html),
            "Backend ack mismatch in HTML. Expected to find: {}",
            mkv_html
        );

        let marty_html = format!("<b>MartyMcNuts</b> {}", creator_text);
        assert!(
            html.contains(&marty_html),
            "Creator ack mismatch in HTML. Expected to find: {}",
            marty_html
        );

        let link_html = "href=\"https://github.com/thedavidweng/sdf-flash-gui\"";
        assert!(
            html.contains(link_html),
            "GitHub URL link mismatch in HTML. Expected to find: {}",
            link_html
        );
    }

    #[test]
    fn test_validate_tool_path() {
        use super::validation::validate_tool_path;

        assert!(validate_tool_path("", Backend::SdfTool).is_err());
        assert!(validate_tool_path("   ", Backend::SdfTool).is_err());

        let temp_dir = std::env::temp_dir();

        let sdftool_file = temp_dir.join("test_sdftool_temp");
        std::fs::write(&sdftool_file, b"").unwrap();
        assert!(validate_tool_path(&sdftool_file.to_string_lossy(), Backend::SdfTool).is_ok());
        assert!(validate_tool_path(&sdftool_file.to_string_lossy(), Backend::MakeMkvCon).is_err());
        let _ = std::fs::remove_file(&sdftool_file);

        let makemkv_file = temp_dir.join("test_makemkvcon_temp");
        std::fs::write(&makemkv_file, b"").unwrap();
        assert!(validate_tool_path(&makemkv_file.to_string_lossy(), Backend::MakeMkvCon).is_ok());
        assert!(validate_tool_path(&makemkv_file.to_string_lossy(), Backend::SdfTool).is_err());
        let _ = std::fs::remove_file(&makemkv_file);

        let non_existent = temp_dir.join("does-not-exist-sdftool");
        assert!(validate_tool_path(&non_existent.to_string_lossy(), Backend::SdfTool).is_err());
    }

    #[test]
    fn test_validate_sdf_path() {
        use super::validation::validate_sdf_path;

        assert!(validate_sdf_path("").is_ok());
        assert!(validate_sdf_path("  ").is_ok());

        let temp_dir = std::env::temp_dir();

        let bin_file = temp_dir.join("test_sdf_temp.bin");
        std::fs::write(&bin_file, b"").unwrap();
        assert!(validate_sdf_path(&bin_file.to_string_lossy()).is_ok());
        let _ = std::fs::remove_file(&bin_file);

        let txt_file = temp_dir.join("test_sdf_temp.txt");
        std::fs::write(&txt_file, b"").unwrap();
        assert!(validate_sdf_path(&txt_file.to_string_lossy()).is_err());
        let _ = std::fs::remove_file(&txt_file);

        let non_existent = temp_dir.join("does-not-exist.bin");
        assert!(validate_sdf_path(&non_existent.to_string_lossy()).is_err());
    }

    #[test]
    fn test_app_state() {
        use super::state::AppState;
        use crate::drive::Drive;

        let mut state = AppState::new();
        assert_eq!(state.status_message, "Ready");
        assert_eq!(state.progress, 0.0);
        assert!(!state.busy);
        assert!(state.log_text.is_empty());

        state.log("Hello");
        assert_eq!(state.log_text, "Hello");
        state.log("World");
        assert_eq!(state.log_text, "Hello\nWorld");

        state.set_status("Working...", 50.0);
        assert_eq!(state.status_message, "Working...");
        assert_eq!(state.progress, 50.0);

        state.set_status("Overworking...", 120.0);
        assert_eq!(state.progress, 100.0);
        state.set_status("Underworking...", -10.0);
        assert_eq!(state.progress, 0.0);

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

        state.selected_drive = Some(1);
        assert!(state.selected_drive().is_none());
    }
}
