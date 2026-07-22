//! Quit / stop / force-kill AppState transitions.
use crate::gui::state::{AppState, StopDialog};
use crate::i18n::{t, L10nKey};

pub(crate) fn begin_app_shutdown(state: &mut AppState) {
    state.chrome.exiting = true;
    state.chrome.show_settings = false;
    state.chrome.show_about = false;
    state.chrome.show_quit_confirmation = false;
    state.runtime.stop_dialog = StopDialog::None;
    if let Some(control) = state.runtime.probe_control.take() {
        control.request_force_kill();
        control.reap_registered_child();
    }
    state.runtime.probing = false;
}

pub(crate) fn close_child_viewports(ctx: &eframe::egui::Context) {
    use eframe::egui::{ViewportCommand, ViewportId};
    ctx.send_viewport_cmd_to(
        ViewportId::from_hash_of("settings_viewport"),
        ViewportCommand::Close,
    );
    ctx.send_viewport_cmd_to(
        ViewportId::from_hash_of("about_viewport"),
        ViewportCommand::Close,
    );
}

pub(crate) fn prepare_app_exit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.chrome.exiting {
        return;
    }
    begin_app_shutdown(state);
    close_child_viewports(ctx);
    ctx.request_repaint();
}

pub fn request_app_quit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.runtime.busy {
        state.chrome.show_quit_confirmation = true;
    } else {
        prepare_app_exit(ctx, state);
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
    }
}

pub fn on_viewport_close_requested(ctx: &eframe::egui::Context, state: &mut AppState) {
    if state.runtime.busy {
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
        state.chrome.show_quit_confirmation = true;
    } else {
        prepare_app_exit(ctx, state);
    }
}

/// Force-kill any running backend and close immediately.
///
/// Clears `busy` before requesting close so `on_viewport_close_requested` does not
/// re-issue `CancelClose` while the worker is still winding down.
pub fn confirm_force_quit_exit(ctx: &eframe::egui::Context, state: &mut AppState) {
    if let Some(control) = state.runtime.probe_control.as_ref() {
        control.request_force_kill();
    }
    if let Some(control) = state.runtime.active_operation.as_ref() {
        control.request_force_kill();
    }
    state.finish_probe();
    state.finish_operation();
    prepare_app_exit(ctx, state);
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
}

pub fn request_stop(state: &mut AppState) {
    if !state.runtime.busy {
        return;
    }
    state.runtime.stop_dialog = if state.runtime.waiting_for_backend_stop {
        StopDialog::ConfirmForceKill
    } else {
        StopDialog::ConfirmStop
    };
}

pub fn confirm_graceful_stop(state: &mut AppState) {
    if let Some(control) = &state.runtime.active_operation {
        control.request_graceful_cancel();
        state.set_status_key(L10nKey::StatusCancelling, state.runtime.progress);
    }
    state.runtime.stop_dialog = StopDialog::None;
}

pub fn confirm_force_kill(state: &mut AppState) {
    if let Some(control) = state.runtime.probe_control.as_ref() {
        control.request_force_kill();
    }
    if let Some(control) = state.runtime.active_operation.as_ref() {
        control.request_force_kill();
    }
    state.log(t(L10nKey::LogOpCancelled, state.chrome.resolved_lang));
    if state.runtime.probe_control.is_some() {
        state.finish_probe_failure();
    }
    if state.runtime.busy {
        state.finish_operation();
        state.set_status_key(L10nKey::StatusOpCancelled, 0.0);
    } else {
        state.runtime.stop_dialog = StopDialog::None;
    }
}

pub fn decline_force_kill(state: &mut AppState) {
    if state.runtime.active_operation.is_some() {
        state.runtime.waiting_for_backend_stop = true;
        state.set_status_key(L10nKey::StatusCancelling, state.runtime.progress);
    }
}
