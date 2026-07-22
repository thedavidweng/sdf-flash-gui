use crate::command::Backend;
use crate::drive;
use crate::gui::file_dialog::FileDialog;
use crate::gui::state::AppState;
use crate::gui::validation;
use crate::gui::workers::{spawn_list_drives, WorkerMsg};
use crate::i18n::{self, t, t_with_args, L10nKey, Language};
use crate::process;
use crate::sdf;

use eframe::egui;
use egui_phosphor::regular as icon;
use std::sync::mpsc;

use super::super::{icon_button, icon_rich, GAP_MEDIUM, GAP_SMALL, GAP_TINY};
use super::main_panel::file_picker;

#[allow(deprecated)] // viewport has no parent Ui; CentralPanel::show(ctx) is still required
pub fn show_settings_window(
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
    runner: &std::sync::Arc<dyn process::ProcessRunner>,
    dialog: &impl FileDialog,
) {
    let app_ctx = ctx.clone();
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("settings_viewport"),
        egui::ViewportBuilder::default()
            .with_title(t(L10nKey::TitleSettings, state.chrome.resolved_lang))
            .with_inner_size([480.0, 320.0])
            .with_min_inner_size([480.0, 320.0])
            .with_resizable(true),
        |ctx, _class| {
            let close_shortcut =
                egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
            let should_close = ctx
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                || ctx.input_mut(|i| i.consume_shortcut(&close_shortcut));
            if should_close {
                state.chrome.show_settings = false;
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label(t(L10nKey::LabelBackend, state.chrome.resolved_lang));
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut state.config.backend,
                            Backend::SdfTool,
                            t(L10nKey::BackendSdftool, state.chrome.resolved_lang),
                        );
                        ui.selectable_value(
                            &mut state.config.backend,
                            Backend::MakeMkvCon,
                            t(L10nKey::BackendMakeMkv, state.chrome.resolved_lang),
                        );
                    });

                    ui.add_space(GAP_MEDIUM);

                    let error_color = ui.visuals().error_fg_color;
                    let valid_color = ui.visuals().hyperlink_color;
                    let weak_color = ui.visuals().weak_text_color();

                    egui::Grid::new("settings_grid")
                        .num_columns(2)
                        .spacing([GAP_MEDIUM, GAP_MEDIUM])
                        .show(ui, |ui| {
                            ui.label(t(L10nKey::LabelToolPath, state.chrome.resolved_lang));
                            ui.vertical(|ui| {
                                if file_picker(
                                    ui,
                                    &mut state.config.tool_path,
                                    "Executable",
                                    &[],
                                    state.chrome.resolved_lang,
                                    dialog,
                                ) {
                                    state.config.auto_detected = false;
                                    state.drive.last_probed_drive = None;
                                    state.config.tool_detect_failed = false;
                                }
                                ui.add_space(GAP_SMALL);
                                if ui
                                    .add(icon_button(
                                        ui,
                                        icon::MAGNIFYING_GLASS,
                                        t(L10nKey::BtnAutoDetect, state.chrome.resolved_lang),
                                    ))
                                    .clicked()
                                {
                                    if let Some((b, p)) = drive::find_backend(state.config.backend)
                                    {
                                        state.config.backend = b;
                                        state.config.tool_path = p;
                                        state.config.auto_detected = true;
                                        state.config.tool_detect_failed = false;
                                        state.drive.last_probed_drive = None;
                                    } else {
                                        state.config.tool_detect_failed = true;
                                    }
                                }
                                if state.config.auto_detected {
                                    ui.add_space(GAP_TINY);
                                    ui.label(
                                        egui::RichText::new(t(
                                            L10nKey::LabelAutodetected,
                                            state.chrome.resolved_lang,
                                        ))
                                        .small()
                                        .italics()
                                        .color(weak_color),
                                    );
                                }
                            });
                            ui.end_row();

                            ui.label("");
                            if state.config.tool_detect_failed && state.config.tool_path.is_empty()
                            {
                                ui.colored_label(
                                    error_color,
                                    t(L10nKey::StatusNotFound, state.chrome.resolved_lang),
                                );
                            } else if let Err(e) = validation::validate_tool_path(
                                &state.config.tool_path,
                                state.config.backend,
                                state.chrome.resolved_lang,
                            ) {
                                ui.colored_label(error_color, format!("⚠ {e}"));
                            } else {
                                ui.colored_label(
                                    valid_color,
                                    t(L10nKey::StatusPathValid, state.chrome.resolved_lang),
                                );
                            }
                            ui.end_row();

                            ui.label(t(L10nKey::LabelSdfPath, state.chrome.resolved_lang));
                            ui.vertical(|ui| {
                                let _ = file_picker(
                                    ui,
                                    &mut state.config.sdf_path,
                                    "SDF",
                                    &["bin"],
                                    state.chrome.resolved_lang,
                                    dialog,
                                );
                                ui.add_space(GAP_SMALL);
                                if ui
                                    .add(icon_button(
                                        ui,
                                        icon::MAGNIFYING_GLASS,
                                        t(L10nKey::BtnAutoDetect, state.chrome.resolved_lang),
                                    ))
                                    .clicked()
                                {
                                    let found = crate::drive::find_sdf_bin();
                                    state.config.sdf_detect_failed = found.is_empty();
                                    state.config.sdf_path = found;
                                }
                            });
                            ui.end_row();

                            ui.label("");
                            if state.config.sdf_detect_failed && state.config.sdf_path.is_empty() {
                                ui.colored_label(
                                    error_color,
                                    t(L10nKey::StatusNotFound, state.chrome.resolved_lang),
                                );
                            } else if let Err(e) = validation::validate_sdf_path(
                                &state.config.sdf_path,
                                state.chrome.resolved_lang,
                            ) {
                                ui.colored_label(error_color, format!("⚠ {e}"));
                            } else if !state.config.sdf_path.is_empty() {
                                ui.colored_label(
                                    valid_color,
                                    t(L10nKey::StatusPathValid, state.chrome.resolved_lang),
                                );
                            } else {
                                ui.weak(t(L10nKey::StatusOptional, state.chrome.resolved_lang));
                            }
                            ui.end_row();
                        });
                });

                ui.add_space(GAP_SMALL);
                ui.label(icon_rich(
                    ui,
                    icon::GLOBE,
                    t(L10nKey::LabelLanguage, state.chrome.resolved_lang),
                    egui::TextStyle::Body,
                ));
                let prev_lang = state.chrome.language;
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("language_selector")
                        .selected_text(state.chrome.language.display_name())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for lang in Language::ALL {
                                ui.selectable_value(
                                    &mut state.chrome.language,
                                    *lang,
                                    lang.display_name(),
                                );
                            }
                        });
                });
                if state.chrome.language != prev_lang {
                    state.chrome.resolved_lang = i18n::resolve_language(state.chrome.language);
                    app_ctx.request_repaint();
                }

                ui.add_space(GAP_SMALL);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(icon_button(
                                ui,
                                icon::BINARY,
                                t(L10nKey::BtnParseSdf, state.chrome.resolved_lang),
                            ))
                            .clicked()
                        {
                            match std::fs::read(&state.config.sdf_path) {
                                Ok(data) => {
                                    let mut cursor = std::io::Cursor::new(&data);
                                    match sdf::parse_sdf0(&mut cursor) {
                                        Ok(container) => {
                                            state.log(&sdf::format_container_log(
                                                &container,
                                                state.chrome.resolved_lang,
                                            ));
                                        }
                                        Err(e) => state.log(&i18n::log_error(
                                            state.chrome.resolved_lang,
                                            &e.to_string(),
                                        )),
                                    }
                                }
                                Err(e) => state.log(&t_with_args(
                                    L10nKey::LogSdfReadFailed,
                                    state.chrome.resolved_lang,
                                    &[("error", &e.to_string())],
                                )),
                            }
                        }

                        if ui
                            .add_enabled(
                                !state.runtime.busy,
                                icon_button(
                                    ui,
                                    icon::LIST,
                                    t(L10nKey::BtnListDrives, state.chrome.resolved_lang),
                                ),
                            )
                            .clicked()
                        {
                            spawn_list_drives(worker_tx, state, runner, true);
                        }
                    });
                });
            });
        },
    );
}
