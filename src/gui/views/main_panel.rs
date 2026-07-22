use crate::branding::MAKEMKV_DOWNLOAD_URL;
use crate::command;
use crate::firmware_db::FlashDirection;
use crate::gui::file_dialog::{FileDialog, NativeDialog};
use crate::gui::ops;
use crate::gui::state::{AppState, ThemeChoice};
use crate::gui::workers::{spawn_list_drives, WorkerMsg};
use crate::i18n::{t, t_with_args, L10nKey, Language};
use crate::platform::{self, DriveFormFactor};
use crate::process;

use eframe::egui;
use egui_phosphor::regular as icon;
use std::sync::mpsc;

use super::super::{
    button_text_size, icon_button, icon_rich, section_heading, OperationMode, GAP_MEDIUM,
    GAP_SMALL, GAP_TINY,
};

pub fn show_main_ui(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
    runner: &std::sync::Arc<dyn process::ProcessRunner>,
) {
    let backend_ok = ops::backend_configured(state);
    let now = ctx.input(|i| i.time);
    let settings_nudge = ops::settings_nudge_active(state.chrome.settings_nudge_until, now);
    let settings_highlight = ops::settings_nudge_highlight(state.chrome.settings_nudge_until, now);
    if settings_nudge {
        ctx.request_repaint();
    } else if state.chrome.settings_nudge_until.is_some() {
        state.chrome.settings_nudge_until = None;
    }

    let mut settings_btn_rect = egui::Rect::NOTHING;
    let mut get_makemkv_link_rect = egui::Rect::NOTHING;

    ui.horizontal(|ui| {
        let refresh_text = t(L10nKey::TooltipRefresh, state.chrome.resolved_lang);
        let mut refresh_hint = if cfg!(target_os = "macos") {
            format!("{refresh_text} (⌘R)")
        } else {
            format!("{refresh_text} (Ctrl+R)")
        };
        if state.drive.drives.is_empty() {
            refresh_hint = format!(
                "{refresh_hint}\n{}",
                t(L10nKey::HelpEmptyDrives, state.chrome.resolved_lang)
            );
        }
        let refresh_resp = ui.add_enabled(
            backend_ok && !state.runtime.busy && !state.runtime.probing,
            super::super::toolbar_icon_button(ui, icon::ARROW_CLOCKWISE),
        );
        refresh_resp.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                refresh_resp.enabled(),
                refresh_text,
            )
        });
        if refresh_resp.on_hover_text(refresh_hint).clicked() {
            // Backend `-l` is the source of truth (macOS USB optical uses
            // /IOBDServices/… paths that OS enumeration cannot produce).
            spawn_list_drives(worker_tx, state, runner, true);
        }
        let settings_text = t(L10nKey::TooltipSettings, state.chrome.resolved_lang);
        let settings_hint = if cfg!(target_os = "macos") {
            format!("{settings_text} (⌘,)")
        } else {
            format!("{settings_text} (Ctrl+,)")
        };
        // Soft white fill only — no stroke / size change (layout-stable attention pulse).
        let mut settings_btn = super::super::toolbar_icon_button(ui, icon::GEAR);
        if settings_highlight > 0.0 {
            // Cap alpha so the gear stays readable; strength already eases in/out.
            let alpha = (settings_highlight * 150.0).clamp(0.0, 150.0) as u8;
            settings_btn = settings_btn.fill(egui::Color32::from_white_alpha(alpha));
        }
        let settings_resp = ui.add(settings_btn);
        settings_btn_rect = settings_resp.rect;
        // Soft outer glow: fixed-width stroke drawn *outside* the widget (no layout shift).
        if settings_highlight > 0.05 {
            let glow_alpha = (settings_highlight * 200.0).clamp(0.0, 200.0) as u8;
            ui.painter().rect_stroke(
                settings_resp.rect,
                ui.visuals().widgets.inactive.corner_radius,
                egui::Stroke::new(1.5_f32, egui::Color32::from_white_alpha(glow_alpha)),
                egui::StrokeKind::Outside,
            );
        }
        settings_resp.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                settings_resp.enabled(),
                settings_text,
            )
        });
        if settings_resp.on_hover_text(settings_hint).clicked() {
            state.chrome.show_settings = true;
            state.chrome.settings_nudge_until = None;
        }
        let about_text = t(L10nKey::TooltipAbout, state.chrome.resolved_lang);
        let about_hint = if cfg!(target_os = "macos") {
            format!("{about_text} (⌘I)")
        } else {
            format!("{about_text} (Ctrl+I)")
        };
        let about_resp = ui.add(super::super::toolbar_icon_button(ui, icon::INFO));
        about_resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, about_resp.enabled(), about_text)
        });
        if about_resp.on_hover_text(about_hint).clicked() {
            state.chrome.show_about = true;
        }
        let quit_text = t(L10nKey::MenuQuit, state.chrome.resolved_lang);
        let quit_hint = if cfg!(target_os = "macos") {
            format!("{quit_text} (⌘Q)")
        } else {
            format!("{quit_text} (Alt+F4)")
        };
        let quit_resp = ui.add(super::super::toolbar_icon_button(ui, icon::X));
        quit_resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, quit_resp.enabled(), quit_text)
        });
        if quit_resp.on_hover_text(quit_hint).clicked() {
            ops::request_app_quit(ctx, state);
        }
        if state.runtime.busy || state.runtime.probing {
            ui.spinner();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lang = state.chrome.resolved_lang;
            let current = state.chrome.theme;
            if ui
                .selectable_label(
                    current == ThemeChoice::Light,
                    icon_rich(
                        ui,
                        icon::SUN,
                        t(L10nKey::ThemeLight, lang),
                        egui::TextStyle::Body,
                    ),
                )
                .clicked()
            {
                state.chrome.theme = ThemeChoice::Light;
                ctx.set_theme(egui::ThemePreference::Light);
            }
            if ui
                .selectable_label(
                    current == ThemeChoice::Dark,
                    icon_rich(
                        ui,
                        icon::MOON,
                        t(L10nKey::ThemeDark, lang),
                        egui::TextStyle::Body,
                    ),
                )
                .clicked()
            {
                state.chrome.theme = ThemeChoice::Dark;
                ctx.set_theme(egui::ThemePreference::Dark);
            }
            if ui
                .selectable_label(
                    current == ThemeChoice::System,
                    icon_rich(
                        ui,
                        icon::DESKTOP,
                        t(L10nKey::ThemeSystem, lang),
                        egui::TextStyle::Body,
                    ),
                )
                .clicked()
            {
                state.chrome.theme = ThemeChoice::System;
                ctx.set_theme(egui::ThemePreference::System);
            }
        });
    });

    if !backend_ok {
        ui.add_space(GAP_SMALL);
        ui.horizontal(|ui| {
            ui.colored_label(
                ui.visuals().error_fg_color,
                icon_rich(
                    ui,
                    icon::WARNING,
                    t(L10nKey::BannerNoBackend, state.chrome.resolved_lang),
                    egui::TextStyle::Body,
                ),
            );
            let link_resp = ui.hyperlink_to(
                t(L10nKey::LinkGetMakeMkv, state.chrome.resolved_lang),
                MAKEMKV_DOWNLOAD_URL,
            );
            get_makemkv_link_rect = link_resp.rect;
        });
    }

    // Operational controls require a configured backend and idle runtime.
    let controls_enabled = backend_ok && !state.runtime.busy && !state.runtime.probing;

    ui.add_enabled_ui(controls_enabled, |ui| {
        section_heading(
            ui,
            icon::HARD_DRIVE,
            t(L10nKey::TitleDriveProperties, state.chrome.resolved_lang),
        );
        ui.add_space(GAP_TINY);
        ui.label(t(L10nKey::LabelDevice, state.chrome.resolved_lang));
        let no_drives_msg = t(L10nKey::StatusNoDrives, state.chrome.resolved_lang);
        let selected_label = state
            .selected_drive()
            .map(ops::drive_label)
            .unwrap_or_else(|| no_drives_msg.to_string());
        let combo_resp = egui::ComboBox::from_id_salt("drive_selector")
            .selected_text(&selected_label)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                if state.drive.drives.is_empty() {
                    ui.label(no_drives_msg);
                } else {
                    for (i, drive) in state.drive.drives.iter().enumerate() {
                        ui.selectable_value(
                            &mut state.drive.selected_drive,
                            Some(i),
                            ops::drive_label(drive),
                        );
                    }
                }
            });
        if state.drive.drives.is_empty() {
            combo_resp
                .response
                .on_hover_text(t(L10nKey::HelpEmptyDrives, state.chrome.resolved_lang));
        }
        if let Some(d) = state.selected_drive().cloned() {
            ui.add_space(GAP_TINY);
            let lang = state.chrome.resolved_lang;
            let na = t(L10nKey::LabelNotAvailable, lang);
            let weak_color = ui.visuals().weak_text_color();
            ui.columns(2, |cols| {
                let prop = |cols: &mut [egui::Ui], label_key: L10nKey, value: &str| {
                    cols[0].label(t(label_key, lang));
                    if value.is_empty() {
                        cols[1].weak(na);
                    } else {
                        cols[1].label(value);
                    }
                };

                prop(cols, L10nKey::LabelManufacturer, &d.vendor);
                prop(cols, L10nKey::LabelProduct, &d.product);
                prop(cols, L10nKey::LabelRevision, &d.revision);
                prop(cols, L10nKey::LabelSerial, &d.serial);
                let date = d.firmware_date_display();
                prop(cols, L10nKey::LabelFirmwareDate, &date);

                cols[0].label(t(L10nKey::LabelMt1959Platform, lang));
                status_indicator(
                    &mut cols[1],
                    state.runtime.probing,
                    state.drive.drive_probed,
                    state.drive.drive_mt1959,
                );

                cols[0].label(t(L10nKey::LabelEncryptedFirmware, lang));
                status_indicator(
                    &mut cols[1],
                    state.runtime.probing,
                    state.drive.drive_probed,
                    state.drive.drive_encrypted_firmware,
                );

                cols[0].label(t(L10nKey::LabelLibreDrive, lang));
                if state.runtime.probing {
                    cols[1].add(egui::Spinner::new());
                } else if !state.drive.drive_probed {
                    cols[1].weak("…");
                } else {
                    let (text_key, color_ok) = match state.drive.drive_libredrive {
                        crate::command::LibreDriveStatus::Enabled => {
                            (L10nKey::LibreDriveEnabled, true)
                        }
                        crate::command::LibreDriveStatus::PossibleNotEnabled => {
                            (L10nKey::LibreDrivePossible, true)
                        }
                        crate::command::LibreDriveStatus::NotAvailable => {
                            (L10nKey::LibreDriveNotAvailable, false)
                        }
                        crate::command::LibreDriveStatus::Unknown => {
                            (L10nKey::LibreDriveUnknown, false)
                        }
                    };
                    let color = if color_ok {
                        egui::Color32::from_rgb(120, 200, 120)
                    } else {
                        weak_color
                    };
                    cols[1].colored_label(color, t(text_key, lang));
                }

                cols[0].label(t(L10nKey::LabelSdfVersion, lang));
                if state.runtime.probing {
                    cols[1].add(egui::Spinner::new());
                } else if !state.drive.drive_probed {
                    cols[1].weak("…");
                } else if let Some(v) = &state.drive.drive_sdf_version {
                    cols[1].label(v);
                } else {
                    cols[1].weak("—");
                }
            });
        }

        ui.add_space(GAP_MEDIUM);

        section_heading(
            ui,
            icon::DISC,
            t(L10nKey::SectionOperation, state.chrome.resolved_lang),
        );
        let prev = state.operation_mode;
        let mode_label = match state.operation_mode {
            OperationMode::Read => icon_rich(
                ui,
                icon::DOWNLOAD_SIMPLE,
                t(L10nKey::TabRead, state.chrome.resolved_lang),
                egui::TextStyle::Body,
            ),
            OperationMode::Write => icon_rich(
                ui,
                icon::UPLOAD_SIMPLE,
                t(L10nKey::TabWrite, state.chrome.resolved_lang),
                egui::TextStyle::Body,
            ),
            OperationMode::Recover => icon_rich(
                ui,
                icon::FIRST_AID,
                t(L10nKey::TabRecover, state.chrome.resolved_lang),
                egui::TextStyle::Body,
            )
            .color(ui.visuals().error_fg_color),
        };
        egui::ComboBox::from_id_salt("operation_mode")
            .selected_text(mode_label)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.operation_mode,
                    OperationMode::Read,
                    icon_rich(
                        ui,
                        icon::DOWNLOAD_SIMPLE,
                        t(L10nKey::TabRead, state.chrome.resolved_lang),
                        egui::TextStyle::Body,
                    ),
                );
                ui.selectable_value(
                    &mut state.operation_mode,
                    OperationMode::Write,
                    icon_rich(
                        ui,
                        icon::UPLOAD_SIMPLE,
                        t(L10nKey::TabWrite, state.chrome.resolved_lang),
                        egui::TextStyle::Body,
                    ),
                );
                ui.selectable_value(
                    &mut state.operation_mode,
                    OperationMode::Recover,
                    icon_rich(
                        ui,
                        icon::FIRST_AID,
                        t(L10nKey::TabRecover, state.chrome.resolved_lang),
                        egui::TextStyle::Body,
                    )
                    .color(ui.visuals().error_fg_color),
                );
            });

        if state.operation_mode != prev {
            ops::on_operation_mode_changed(state, state.operation_mode);
        }

        let write_mode = state.operation_mode == OperationMode::Write;
        if write_mode {
            ui.add_space(GAP_TINY);
            ui.checkbox(
                &mut state.flash.include_boot_loader,
                t(L10nKey::OptionBootloader, state.chrome.resolved_lang),
            );
            ui.checkbox(
                &mut state.flash.encrypted_write,
                t(L10nKey::OptionEncrypted, state.chrome.resolved_lang),
            );
            if state.flash.encrypted_write && state.flash.include_boot_loader {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    t(L10nKey::WarnCannotCombine, state.chrome.resolved_lang),
                );
            }
            ui.add_space(GAP_TINY);
            ui.checkbox(
                &mut state.flash.dry_run_only,
                t(L10nKey::OptionDryRunOnly, state.chrome.resolved_lang),
            );
        }

        if state.operation_mode != OperationMode::Read {
            ui.add_space(GAP_SMALL);
            show_firmware_selector(ui, state, &NativeDialog);
        }

        show_mode_specific_options(ui, state, &NativeDialog);
    });

    ui.add_space(GAP_MEDIUM);

    section_heading(
        ui,
        icon::PULSE,
        t(L10nKey::SectionStatus, state.chrome.resolved_lang),
    );
    ui.add_space(GAP_TINY);
    if state.runtime.busy {
        if matches!(
            state.operation_mode,
            OperationMode::Write | OperationMode::Recover
        ) {
            ui.label(
                egui::RichText::new(t(L10nKey::HintFlashNoCancel, state.chrome.resolved_lang))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
            ui.add_space(GAP_TINY);
        }
        let status = format!("{}…", state.runtime.status_message.trim_end_matches('…'));
        if state.runtime.progress_indeterminate && state.runtime.progress <= 0.0 {
            ui.add(egui::ProgressBar::new(0.0).animate(true).text(status));
        } else {
            ui.add(
                egui::ProgressBar::new(state.runtime.progress / 100.0)
                    .show_percentage()
                    .text(status),
            );
        }
    } else {
        ui.add(
            egui::ProgressBar::new(0.0)
                .fill(egui::Color32::TRANSPARENT)
                .text(t(L10nKey::StatusReadyText, state.chrome.resolved_lang)),
        );
    }

    ui.add_space(GAP_SMALL);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.runtime.busy {
                let stop_text = t(L10nKey::TooltipStop, state.chrome.resolved_lang);
                if ui
                    .add(icon_button(
                        ui,
                        icon::STOP,
                        t(L10nKey::BtnStop, state.chrome.resolved_lang),
                    ))
                    .on_hover_text(stop_text)
                    .clicked()
                {
                    ops::request_stop(state);
                }
            } else {
                let start_enabled = ops::can_start(state);
                let start_text = t(L10nKey::TooltipStartEnabled, state.chrome.resolved_lang);
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
                        .add(icon_button(
                            ui,
                            icon::PLAY,
                            t(L10nKey::BtnStart, state.chrome.resolved_lang),
                        ))
                        .on_disabled_hover_text(hover)
                        .clicked()
                    {
                        ops::execute_start(state, worker_tx, &NativeDialog, runner);
                    }
                });
            }
        });
    });

    ui.add_space(GAP_TINY);
    let log_height = ui.available_height() - 20.0;
    let log_height = log_height.max(40.0);

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(log_height)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if state.runtime.log_text.is_empty() {
                ui.weak(t(L10nKey::LogReady, state.chrome.resolved_lang));
            } else {
                ui.label(
                    egui::RichText::new(&state.runtime.log_text)
                        .monospace()
                        .size(11.0),
                );
            }
        });

    let drive_count = state.drive.drives.len();
    let status_text = if drive_count == 0 {
        t(L10nKey::StatusNoDrivesFound, state.chrome.resolved_lang).to_string()
    } else if drive_count == 1 {
        t(L10nKey::StatusOneDriveFound, state.chrome.resolved_lang).to_string()
    } else {
        t_with_args(
            L10nKey::StatusDrivesFound,
            state.chrome.resolved_lang,
            &[("count", &drive_count.to_string())],
        )
    };
    ui.label(
        icon_rich(ui, icon::HARD_DRIVES, &status_text, egui::TextStyle::Body)
            .small()
            .weak(),
    );

    // Low-intrusion first-run: no modal. Clicks outside Settings / Get MakeMKV pulse the gear.
    if !backend_ok && ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(pos) = ctx.pointer_interact_pos() {
            let on_settings = settings_btn_rect.contains(pos);
            let on_get_makemkv = get_makemkv_link_rect.contains(pos);
            if ops::click_should_nudge_settings(backend_ok, on_settings || on_get_makemkv) {
                state.chrome.settings_nudge_until = Some(now + ops::SETTINGS_NUDGE_SECONDS);
            }
        }
    }
}

fn status_indicator(ui: &mut egui::Ui, probing: bool, probed: bool, ok: bool) {
    let size = button_text_size(ui);
    if probing {
        ui.add(egui::Spinner::new());
    } else if !probed {
        ui.weak("…");
    } else if ok {
        ui.colored_label(
            ui.visuals().hyperlink_color,
            egui::RichText::new(icon::CHECK_CIRCLE).size(size),
        );
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            egui::RichText::new(icon::X_CIRCLE).size(size),
        );
    }
}

fn show_firmware_selector(ui: &mut egui::Ui, state: &mut AppState, dialog: &impl FileDialog) {
    let firmware_img_text = t(L10nKey::SectionFirmwareImage, state.chrome.resolved_lang);

    ui.label(icon_rich(
        ui,
        icon::FLOPPY_DISK,
        &format!("{firmware_img_text}:"),
        egui::TextStyle::Body,
    ));
    let path_before = state.flash.firmware_path.clone();
    let _ = file_picker(
        ui,
        &mut state.flash.firmware_path,
        "Firmware",
        &["bin"],
        state.chrome.resolved_lang,
        dialog,
    );
    if state.flash.firmware_path != path_before {
        if state.flash.firmware_path.is_empty() {
            state.flash.firmware_data = None;
            state.flash.firmware_picker_items.clear();
        } else {
            let path = state.flash.firmware_path.clone();
            ops::load_firmware(state, &path);
        }
    }

    if state.flash.firmware_picker_items.len() > 1 {
        ui.add_space(GAP_TINY);
        let current_name = std::path::Path::new(&state.flash.firmware_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let mut picked: Option<String> = None;
        egui::ComboBox::from_id_salt("firmware_picker")
            .selected_text(&current_name)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (name, path) in &state.flash.firmware_picker_items {
                    if ui
                        .selectable_label(state.flash.firmware_path == *path, name)
                        .clicked()
                    {
                        picked = Some(path.clone());
                    }
                }
            });
        if let Some(path) = picked {
            state.flash.firmware_path = path.clone();
            ops::load_firmware(state, &path);
        }
    }

    if !state.flash.firmware_path.is_empty() && state.flash.firmware_data.is_none() {
        ui.add_space(GAP_TINY);
        ui.colored_label(
            ui.visuals().error_fg_color,
            t(L10nKey::WarnFirmwareLoadFailed, state.chrome.resolved_lang),
        );
    }
}

fn show_mode_specific_options(ui: &mut egui::Ui, state: &mut AppState, dialog: &impl FileDialog) {
    match state.operation_mode {
        OperationMode::Read => {}
        OperationMode::Write => {
            if let Some(drive) = state.selected_drive().cloned() {
                let required = command::required_flash_confirmation(&drive.device);
                ui.add_space(GAP_SMALL);
                show_confirmation_summary(ui, state, &drive);
                show_safety_warnings(ui, state, &drive);
                ui.label(t_with_args(
                    L10nKey::LabelTypeToConfirm,
                    state.chrome.resolved_lang,
                    &[("required", &required)],
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut state.flash.confirmation)
                        .desired_width(ui.available_width()),
                );
            }
        }
        OperationMode::Recover => {
            ui.add_space(GAP_SMALL);
            ui.horizontal(|ui| {
                ui.label(icon_rich(
                    ui,
                    icon::KEY,
                    t(L10nKey::LabelToken, state.chrome.resolved_lang),
                    egui::TextStyle::Body,
                ));
                let available_width = ui.available_width() - 40.0;
                ui.add(
                    egui::TextEdit::singleline(&mut state.flash.recovery_token)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(available_width),
                );
                let token_color = if state.flash.recovery_token.len() == 16 {
                    ui.visuals().hyperlink_color
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.label(
                    egui::RichText::new(t_with_args(
                        L10nKey::LabelTokenLength,
                        state.chrome.resolved_lang,
                        &[("current", &state.flash.recovery_token.len().to_string())],
                    ))
                    .small()
                    .monospace()
                    .color(token_color),
                );
            });

            ui.add_space(GAP_TINY);
            ui.label(t(L10nKey::LabelWrongFw, state.chrome.resolved_lang));
            if file_picker(
                ui,
                &mut state.flash.wrong_firmware_path,
                "Firmware",
                &["bin"],
                state.chrome.resolved_lang,
                dialog,
            ) && !state.flash.wrong_firmware_path.is_empty()
            {
                ops::extract_recovery_token_from_wrong_firmware(state);
            }
            ui.add_space(GAP_SMALL);
            if ui
                .add(icon_button(
                    ui,
                    icon::EXPORT,
                    t(L10nKey::BtnExtract, state.chrome.resolved_lang),
                ))
                .clicked()
            {
                ops::extract_recovery_token_from_wrong_firmware(state);
            }

            if let Some(drive) = state.selected_drive() {
                let required = command::required_flash_confirmation(&drive.device);
                ui.add_space(GAP_SMALL);
                show_confirmation_summary(ui, state, drive);
                ui.label(t_with_args(
                    L10nKey::LabelTypeToConfirm,
                    state.chrome.resolved_lang,
                    &[("required", &required)],
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut state.flash.confirmation)
                        .desired_width(ui.available_width()),
                );
            }
        }
    }
}

fn show_confirmation_summary(ui: &mut egui::Ui, state: &AppState, drive: &crate::drive::Drive) {
    let lang = state.chrome.resolved_lang;
    ui.label(
        egui::RichText::new(t(L10nKey::LabelFlashSummaryTitle, lang))
            .strong()
            .color(ui.visuals().warn_fg_color),
    );
    ui.add_space(GAP_TINY);
    let label = ops::drive_label(drive);
    ui.label(t_with_args(
        L10nKey::LabelFlashSummaryDrive,
        lang,
        &[("label", &label), ("device", &drive.device)],
    ));
    ui.label(t_with_args(
        L10nKey::LabelFlashSummaryFirmware,
        lang,
        &[
            ("file", &ops::firmware_basename(state)),
            ("hash", &ops::firmware_sha_prefix(state)),
        ],
    ));
    ui.label(t_with_args(
        L10nKey::LabelFlashSummaryMode,
        lang,
        &[("mode", &ops::flash_mode_label(state))],
    ));
}

fn show_safety_warnings(ui: &mut egui::Ui, state: &mut AppState, drive: &crate::drive::Drive) {
    let lang = state.chrome.resolved_lang;

    // Warning 1: Platform mismatch with cross-flash confirmation
    let drive_ff = platform::classify_drive(&drive.product);
    let fw_ff = state.flash.firmware_form_factor;
    if drive_ff != DriveFormFactor::Unknown
        && fw_ff != DriveFormFactor::Unknown
        && drive_ff != fw_ff
    {
        ui.add_space(GAP_TINY);
        ui.colored_label(
            ui.visuals().error_fg_color,
            t_with_args(
                L10nKey::WarnPlatformMismatch,
                lang,
                &[("firmware", fw_ff.label()), ("drive", drive_ff.label())],
            ),
        );
        ui.checkbox(
            &mut state.flash.cross_flash_confirmed,
            t(L10nKey::WarnCrossFlashConfirm, lang),
        );
    }

    // Warning 2: Two-step flash guidance
    if platform::needs_two_step_flash(&drive.product) {
        ui.add_space(GAP_TINY);
        ui.colored_label(
            ui.visuals().warn_fg_color,
            t(L10nKey::InfoTwoStepFlash, lang),
        );
    }

    // Warning 3: Version downgrade (from known firmware database)
    if let Some(resolved) = &state.flash.firmware_resolved {
        if let Some(known) = resolved.identification.known {
            let direction = crate::firmware_db::compare_versions(&drive.revision, known.version);
            if direction == FlashDirection::Downgrade {
                ui.add_space(GAP_TINY);
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    t_with_args(
                        L10nKey::WarnFirmwareDowngrade,
                        lang,
                        &[("current", &drive.revision), ("target", known.version)],
                    ),
                );
            }
        }
    }

    // Warning 4: Firmware-drive model match info (from binary analysis or known database)
    if let Some(resolved) = &state.flash.firmware_resolved {
        if let Some(fw_model) = &resolved.model {
            if !drive.product.contains(fw_model.as_str()) && !fw_model.contains(&drive.product) {
                ui.add_space(GAP_TINY);
                ui.colored_label(
                    ui.visuals().weak_text_color(),
                    t_with_args(
                        L10nKey::InfoFirmwareModelMismatch,
                        lang,
                        &[("firmware", fw_model), ("drive", &drive.product)],
                    ),
                );
            }
        }
    }
}

/// Renders a TextEdit + Browse button row. Returns `true` if the path changed.
pub(crate) fn file_picker(
    ui: &mut egui::Ui,
    path: &mut String,
    filter_name: &str,
    extensions: &[&str],
    lang: Language,
    dialog: &impl FileDialog,
) -> bool {
    let initial_dir = std::path::Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(icon_button(
                    ui,
                    icon::FOLDER_OPEN,
                    t(L10nKey::BtnBrowse, lang),
                ))
                .clicked()
            {
                if let Some(file) = dialog.pick_file_with_title(
                    filter_name,
                    filter_name,
                    extensions,
                    initial_dir.as_deref(),
                ) {
                    *path = file.to_string_lossy().to_string();
                    changed = true;
                }
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let before = path.clone();
                let path_hint = if path.is_empty() {
                    String::new()
                } else {
                    path.clone()
                };
                let edit = egui::TextEdit::singleline(path).desired_width(ui.available_width());
                let resp = ui.add(edit);
                if !path_hint.is_empty() {
                    resp.on_hover_text(&path_hint);
                }
                if *path != before {
                    changed = true;
                }
            });
        });
    });
    changed
}
