mod about;
mod dialogs;
mod main_panel;
mod settings;

use eframe::egui;

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
