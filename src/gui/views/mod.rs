mod about;
mod dialogs;
mod main_panel;
mod settings;

use crate::i18n::{t, L10nKey, Language};

use eframe::egui;
use egui_phosphor::regular as icon;

pub use about::show_about_window;
pub use dialogs::{
    handle_global_shortcuts, show_flash_failure_dialog, show_force_kill_dialog,
    show_quit_confirmation_dialog, show_stop_confirmation_dialog,
};
pub use main_panel::show_main_ui;
pub use settings::show_settings_window;

fn viewport_close_requested(ctx: &egui::Context) -> bool {
    let close_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::W);
    ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
        || ctx.input_mut(|i| i.consume_shortcut(&close_shortcut))
}

fn embedded_close_clicked(ui: &mut egui::Ui, class: egui::ViewportClass, lang: Language) -> bool {
    if class != egui::ViewportClass::EmbeddedWindow {
        return false;
    }
    ui.add_space(super::GAP_SMALL);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        ui.add(super::icon_button(ui, icon::X, t(L10nKey::BtnClose, lang)))
            .clicked()
    })
    .inner
}
