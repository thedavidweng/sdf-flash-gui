use crate::gui::file_dialog::NativeDialog;
use crate::gui::ops;
use crate::gui::state::{AppState, StopDialog};
use crate::gui::workers::WorkerMsg;
use crate::i18n::{t, L10nKey};
use crate::process;

use eframe::egui;
use egui_phosphor::regular as icon;
use std::sync::mpsc;

use super::super::icon_button;

pub fn handle_global_shortcuts(
    ctx: &egui::Context,
    state: &mut AppState,
    worker_tx: &mpsc::Sender<WorkerMsg>,
    runner: &std::sync::Arc<dyn process::ProcessRunner>,
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
        if i.consume_shortcut(&refresh_shortcut)
            && ops::backend_configured(state)
            && !state.runtime.busy
            && !state.runtime.probing
        {
            crate::gui::workers::spawn_list_drives(worker_tx, state, runner, true);
        }

        for quit_shortcut in [
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W),
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q),
            egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::F4),
        ] {
            if i.consume_shortcut(&quit_shortcut) {
                ops::request_app_quit(ctx, state);
            }
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

fn centered_modal(ctx: &egui::Context, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical_centered(add_contents);
        });
}

fn warning_header(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(format!("{}  {}", icon::WARNING, text))
            .color(ui.visuals().error_fg_color)
            .strong(),
    );
}

pub fn show_quit_confirmation_dialog(ctx: &egui::Context, state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    centered_modal(ctx, t(L10nKey::TitleExitWarning, lang), |ui| {
        warning_header(ui, t(L10nKey::LabelExitWarningMsg, lang));
        ui.add_space(8.0);
        ui.label(t(L10nKey::LabelExitWarningDesc, lang));
        ui.label(t(L10nKey::LabelExitWarningAsk, lang));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            if ui
                .add(icon_button(ui, icon::X, t(L10nKey::BtnNoCancel, lang)))
                .clicked()
            {
                state.chrome.show_quit_confirmation = false;
            }
            ui.add_space(20.0);
            if ui
                .add(icon_button(
                    ui,
                    icon::WARNING,
                    t(L10nKey::BtnYesForce, lang),
                ))
                .clicked()
            {
                ops::confirm_force_quit_exit(ctx, state);
            }
        });
    });
}

pub fn show_stop_confirmation_dialog(ctx: &egui::Context, state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    centered_modal(ctx, t(L10nKey::TitleStopWarning, lang), |ui| {
        warning_header(ui, t(L10nKey::LabelStopWarningMsg, lang));
        ui.add_space(8.0);
        ui.label(t(L10nKey::LabelStopWarningDesc, lang));
        ui.label(t(L10nKey::LabelStopWarningAsk, lang));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            if ui
                .add(icon_button(ui, icon::X, t(L10nKey::BtnStopNo, lang)))
                .clicked()
            {
                state.runtime.stop_dialog = StopDialog::None;
            }
            ui.add_space(20.0);
            if ui
                .add(icon_button(ui, icon::STOP, t(L10nKey::BtnStopYes, lang)))
                .clicked()
            {
                ops::confirm_graceful_stop(state);
            }
        });
    });
}

pub fn show_flash_failure_dialog(ctx: &egui::Context, state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    centered_modal(ctx, t(L10nKey::TitleFlashFailure, lang), |ui| {
        warning_header(ui, t(L10nKey::LabelFlashFailureMsg, lang));
        ui.add_space(8.0);
        ui.label(t(L10nKey::LabelFlashFailureStep1, lang));
        ui.label(t(L10nKey::LabelFlashFailureStep2, lang));
        ui.label(t(L10nKey::LabelFlashFailureStep3, lang));
        ui.add_space(12.0);
        if ui
            .add(icon_button(
                ui,
                icon::CHECK,
                t(L10nKey::BtnFlashFailureDismiss, lang),
            ))
            .clicked()
        {
            state.chrome.show_flash_failure_dialog = false;
        }
    });
}

pub fn show_force_kill_dialog(ctx: &egui::Context, state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    centered_modal(ctx, t(L10nKey::TitleForceKillWarning, lang), |ui| {
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
            warning_header(ui, t(L10nKey::LabelForceKillMsg, lang));
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
}
