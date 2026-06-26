use crate::gui::file_dialog::NativeDialog;
use crate::gui::ops;
use crate::gui::process_runner;
use crate::gui::state::{AppState, StopDialog};
use crate::gui::workers::WorkerMsg;
use crate::i18n::{t, L10nKey};

use eframe::egui;
use egui_phosphor::regular as icon;
use std::sync::mpsc;

use super::super::icon_button;

pub fn handle_global_shortcuts(
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
    runner: &std::sync::Arc<dyn process_runner::ProcessRunner>,
) {
    if state.chrome.exiting {
        return;
    }
    ctx.input_mut(|i| {
        let settings_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma);
        if i.consume_shortcut(&settings_shortcut) {
            state.chrome.show_settings = !state.chrome.show_settings;
        }

        let about_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::I);
        if i.consume_shortcut(&about_shortcut) {
            state.chrome.show_about = !state.chrome.show_about;
        }

        let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::R);
        if i.consume_shortcut(&refresh_shortcut) && !state.runtime.busy && !state.runtime.probing {
            ops::refresh_drives(state);
        }

        let close_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
        if i.consume_shortcut(&close_shortcut) {
            ops::request_app_quit(ctx, state);
        }

        let quit_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q);
        if i.consume_shortcut(&quit_shortcut) {
            ops::request_app_quit(ctx, state);
        }

        let alt_f4_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::F4);
        if i.consume_shortcut(&alt_f4_shortcut) {
            ops::request_app_quit(ctx, state);
        }

        let start_cmd_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
        let start_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter);

        if (i.consume_shortcut(&start_cmd_shortcut) || i.consume_shortcut(&start_shortcut))
            && !state.runtime.busy
            && !state.runtime.probing
            && ops::can_start(state)
        {
            ops::execute_start(state, worker_tx, &NativeDialog, runner);
        }
    });
}

pub fn show_quit_confirmation_dialog(ctx: &egui::Context, state: &mut AppState) {
    egui::Window::new(t(L10nKey::TitleExitWarning, state.chrome.resolved_lang))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(t(
                        L10nKey::LabelExitWarningMsg,
                        state.chrome.resolved_lang,
                    ))
                    .color(ui.visuals().error_fg_color)
                    .strong(),
                );
                ui.add_space(8.0);
                ui.label(t(L10nKey::LabelExitWarningDesc, state.chrome.resolved_lang));
                ui.label(t(L10nKey::LabelExitWarningAsk, state.chrome.resolved_lang));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::X,
                            t(L10nKey::BtnNoCancel, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        state.chrome.show_quit_confirmation = false;
                    }
                    ui.add_space(20.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::WARNING,
                            t(L10nKey::BtnYesForce, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        ops::confirm_force_quit_exit(ctx, state);
                    }
                });
            });
        });
}

pub fn show_stop_confirmation_dialog(ctx: &egui::Context, state: &mut AppState) {
    egui::Window::new(t(L10nKey::TitleStopWarning, state.chrome.resolved_lang))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(t(
                        L10nKey::LabelStopWarningMsg,
                        state.chrome.resolved_lang,
                    ))
                    .color(ui.visuals().error_fg_color)
                    .strong(),
                );
                ui.add_space(8.0);
                ui.label(t(L10nKey::LabelStopWarningDesc, state.chrome.resolved_lang));
                ui.label(t(L10nKey::LabelStopWarningAsk, state.chrome.resolved_lang));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::X,
                            t(L10nKey::BtnStopNo, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        state.runtime.stop_dialog = StopDialog::None;
                    }
                    ui.add_space(20.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::STOP,
                            t(L10nKey::BtnStopYes, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        ops::confirm_graceful_stop(state);
                    }
                });
            });
        });
}

pub fn show_first_run_dialog(ctx: &egui::Context, state: &mut AppState) {
    egui::Window::new(t(L10nKey::TitleFirstRun, state.chrome.resolved_lang))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(t(L10nKey::LabelFirstRunMsg, state.chrome.resolved_lang));
                ui.add_space(8.0);
                ui.label(t(L10nKey::LabelFirstRunStep1, state.chrome.resolved_lang));
                ui.label(t(L10nKey::LabelFirstRunStep2, state.chrome.resolved_lang));
                ui.label(t(L10nKey::LabelFirstRunStep3, state.chrome.resolved_lang));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(icon_button(
                            ui,
                            icon::X,
                            t(L10nKey::BtnFirstRunDismiss, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        state.chrome.show_first_run_setup = false;
                    }
                    ui.add_space(12.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::GEAR,
                            t(L10nKey::BtnFirstRunOpenSettings, state.chrome.resolved_lang),
                        ))
                        .clicked()
                    {
                        state.chrome.show_first_run_setup = false;
                        state.chrome.show_settings = true;
                    }
                });
            });
        });
}

pub fn show_flash_failure_dialog(ctx: &egui::Context, state: &mut AppState) {
    egui::Window::new(t(L10nKey::TitleFlashFailure, state.chrome.resolved_lang))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(t(
                        L10nKey::LabelFlashFailureMsg,
                        state.chrome.resolved_lang,
                    ))
                    .color(ui.visuals().error_fg_color)
                    .strong(),
                );
                ui.add_space(8.0);
                ui.label(t(
                    L10nKey::LabelFlashFailureStep1,
                    state.chrome.resolved_lang,
                ));
                ui.label(t(
                    L10nKey::LabelFlashFailureStep2,
                    state.chrome.resolved_lang,
                ));
                ui.label(t(
                    L10nKey::LabelFlashFailureStep3,
                    state.chrome.resolved_lang,
                ));
                ui.add_space(12.0);
                if ui
                    .add(icon_button(
                        ui,
                        icon::CHECK,
                        t(L10nKey::BtnFlashFailureDismiss, state.chrome.resolved_lang),
                    ))
                    .clicked()
                {
                    state.chrome.show_flash_failure_dialog = false;
                }
            });
        });
}

pub fn show_force_kill_dialog(ctx: &egui::Context, state: &mut AppState) {
    egui::Window::new(t(
        L10nKey::TitleForceKillWarning,
        state.chrome.resolved_lang,
    ))
    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
    .resizable(false)
    .collapsible(false)
    .show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            let lang = state.chrome.resolved_lang;
            if state.runtime.waiting_for_backend_stop {
                ui.label(
                    egui::RichText::new(t(L10nKey::StatusCancelling, lang))
                        .color(ui.visuals().warn_fg_color)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(t(L10nKey::LabelForceKillDesc, lang));
                ui.label(t(L10nKey::LabelForceKillAsk, lang));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(80.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::PROHIBIT,
                            t(L10nKey::BtnForceKillYes, lang),
                        ))
                        .clicked()
                    {
                        ops::confirm_force_kill(state);
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new(t(L10nKey::LabelForceKillMsg, lang))
                        .color(ui.visuals().error_fg_color)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(t(L10nKey::LabelForceKillDesc, lang));
                ui.label(t(L10nKey::LabelForceKillAsk, lang));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    if ui
                        .add(icon_button(ui, icon::X, t(L10nKey::BtnForceKillNo, lang)))
                        .clicked()
                    {
                        ops::decline_force_kill(state);
                    }
                    ui.add_space(20.0);
                    if ui
                        .add(icon_button(
                            ui,
                            icon::PROHIBIT,
                            t(L10nKey::BtnForceKillYes, lang),
                        ))
                        .clicked()
                    {
                        ops::confirm_force_kill(state);
                    }
                });
            }
        });
    });
}
