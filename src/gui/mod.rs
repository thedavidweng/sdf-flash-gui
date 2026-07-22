pub mod file_dialog;
mod ops;
mod start_gate;
mod state;
mod validation;
mod views;
mod workers;

pub use crate::drive::find_sdf_bin;

use eframe::egui;

use crate::process_runner::NativeRunner;
use file_dialog::NativeDialog;
use state::{AppState, PersistedSettings, StopDialog, SETTINGS_STORAGE_KEY};
use views::{
    handle_global_shortcuts, show_about_window, show_flash_failure_dialog, show_force_kill_dialog,
    show_main_ui, show_quit_confirmation_dialog, show_settings_window,
    show_stop_confirmation_dialog,
};
use workers::{spawn_list_drives, spawn_probe, WorkerMsg};

const WINDOW_WIDTH: f32 = 480.0;
const WINDOW_HEIGHT: f32 = 720.0;
const WINDOW_MIN_WIDTH: f32 = 400.0;
const WINDOW_MIN_HEIGHT: f32 = 620.0;

pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Spacing constants ──────────────────────────────────────────────
/// Between tightly coupled elements (label ↔ input, checkbox ↔ checkbox).
pub(crate) const GAP_TINY: f32 = 3.0;
/// Between related elements (input ↔ hint, button ↔ button).
pub(crate) const GAP_SMALL: f32 = 6.0;
/// Between loosely related groups inside the same section.
pub(crate) const GAP_MEDIUM: f32 = 12.0;
/// Outer padding of the main panel (egui `Margin` uses i8).
pub(crate) const PANEL_MARGIN: i8 = 10;

pub(crate) fn button_text_size(ui: &egui::Ui) -> f32 {
    ui.style()
        .text_styles
        .get(&egui::TextStyle::Button)
        .map(|font| font.size)
        .unwrap_or(14.0)
}

pub(crate) fn body_text_size(ui: &egui::Ui) -> f32 {
    ui.style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|font| font.size)
        .unwrap_or(14.0)
}

pub(crate) fn toolbar_icon_button(ui: &egui::Ui, glyph: &'static str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(glyph).size(button_text_size(ui)))
}

pub(crate) fn icon_button(
    ui: &egui::Ui,
    glyph: &'static str,
    label: &str,
) -> egui::Button<'static> {
    egui::Button::new(icon_rich(ui, glyph, label, egui::TextStyle::Button))
}

pub(crate) fn icon_rich(
    ui: &egui::Ui,
    glyph: &'static str,
    text: &str,
    style: egui::TextStyle,
) -> egui::RichText {
    let size = ui
        .style()
        .text_styles
        .get(&style)
        .map(|font| font.size)
        .unwrap_or(14.0);
    egui::RichText::new(format!("{glyph}  {text}")).size(size)
}

pub(crate) fn combo_label_text(ui: &egui::Ui, text: &str) -> egui::RichText {
    egui::RichText::new(text).size(body_text_size(ui))
}

pub(crate) fn section_heading(ui: &mut egui::Ui, glyph: &'static str, text: &str) {
    ui.add_space(GAP_SMALL);
    ui.heading(icon_rich(ui, glyph, text, egui::TextStyle::Heading));
    ui.separator();
    ui.add_space(GAP_TINY);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Read,
    Write,
    Recover,
}

/// Window/dock icon embedded at compile time (same asset as packager `icons`).
pub(crate) fn window_icon() -> std::sync::Arc<egui::IconData> {
    use std::sync::{Arc, OnceLock};

    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon.png"))
            .expect("bundled app icon must be valid PNG");
        Arc::new(icon)
    })
    .clone()
}

fn apply_app_text_styles(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};
    use std::collections::BTreeMap;

    let text_styles: BTreeMap<TextStyle, FontId> = [
        (
            TextStyle::Small,
            FontId::new(10.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.all_styles_mut(move |style| {
        style.text_styles = text_styles.clone();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 3.0);
    });
}

pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT])
            .with_resizable(true)
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        crate::branding::DISPLAY_NAME,
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            apply_app_text_styles(&cc.egui_ctx);

            let persisted = cc
                .storage
                .and_then(|s| eframe::get_value::<PersistedSettings>(s, SETTINGS_STORAGE_KEY));
            let state = AppState::with_persisted(persisted.as_ref());
            cc.egui_ctx.set_theme(state.chrome.theme.to_egui());

            Ok(Box::new(App::new(state)))
        }),
    )
}

struct App {
    state: AppState,
    worker_rx: std::sync::mpsc::Receiver<WorkerMsg>,
    worker_tx: std::sync::mpsc::Sender<WorkerMsg>,
    runner: std::sync::Arc<dyn crate::process::ProcessRunner>,
}

impl App {
    fn new(mut state: AppState) -> Self {
        let (worker_tx, worker_rx) = std::sync::mpsc::channel();
        let runner: std::sync::Arc<dyn crate::process::ProcessRunner> =
            std::sync::Arc::new(NativeRunner);

        if ops::backend_configured(&state) {
            spawn_list_drives(&worker_tx, &mut state, &runner, false);
        } else {
            ops::refresh_drives(&mut state);
        }

        Self {
            state,
            worker_rx,
            worker_tx,
            runner,
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            ops::on_viewport_close_requested(ctx, &mut self.state);
        }

        let ctx_for_worker = if self.state.chrome.exiting {
            None
        } else {
            Some(ctx)
        };
        workers::poll_worker(&mut self.state, &self.worker_rx, ctx_for_worker);

        if self.state.chrome.exiting {
            return;
        }

        handle_global_shortcuts(ctx, &mut self.state, &self.worker_tx, &self.runner);

        if ops::backend_configured(&self.state)
            && !self.state.runtime.busy
            && !self.state.runtime.probing
            && self.state.drive.selected_drive != self.state.drive.last_probed_drive
        {
            if let Some(idx) = self.state.drive.selected_drive {
                spawn_probe(&self.worker_tx, &mut self.state, idx, &self.runner);
                self.state.runtime.probing = true;
            }
        }

        if self.state.flash.pending_recover_browse {
            self.state.flash.pending_recover_browse = false;
            ops::prompt_recovery_wrong_firmware(&mut self.state, &NativeDialog);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.state.chrome.exiting {
            egui::CentralPanel::default().show_inside(ui, |_ui| {});
            return;
        }

        let ctx = ui.ctx().clone();
        let modal_open = self.state.chrome.show_quit_confirmation
            || self.state.chrome.show_flash_failure_dialog
            || self.state.runtime.stop_dialog != StopDialog::None;

        let panel_frame =
            egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::same(PANEL_MARGIN));
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(ui, |ui| {
                ui.add_enabled_ui(!modal_open, |ui| {
                    show_main_ui(ui, &ctx, &mut self.state, &self.worker_tx, &self.runner);
                });
            });

        if self.state.chrome.show_settings {
            self.state.chrome.settings_nudge_until = None;
            show_settings_window(
                &ctx,
                &mut self.state,
                &self.worker_tx,
                &self.runner,
                &NativeDialog,
            );
        }

        if self.state.chrome.show_about {
            show_about_window(&ctx, &mut self.state);
        }

        if self.state.chrome.show_quit_confirmation {
            show_quit_confirmation_dialog(&ctx, &mut self.state);
        }
        if self.state.chrome.show_flash_failure_dialog {
            show_flash_failure_dialog(&ctx, &mut self.state);
        }
        if self.state.runtime.stop_dialog == StopDialog::ConfirmStop {
            show_stop_confirmation_dialog(&ctx, &mut self.state);
        }
        if self.state.runtime.stop_dialog == StopDialog::ConfirmForceKill {
            show_force_kill_dialog(&ctx, &mut self.state);
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let settings = self.state.snapshot_settings();
        eframe::set_value(_storage, SETTINGS_STORAGE_KEY, &settings);
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{t, L10nKey, Language};

    #[test]
    fn window_icon_is_valid_png() {
        let icon = super::window_icon();
        assert!(icon.width > 0);
        assert!(icon.height > 0);
        assert_eq!(
            icon.rgba.len(),
            (icon.width as usize) * (icon.height as usize) * 4
        );
    }

    #[test]
    fn default_window_is_larger_than_minimum() {
        assert!(super::WINDOW_WIDTH > super::WINDOW_MIN_WIDTH);
        assert!(super::WINDOW_HEIGHT > super::WINDOW_MIN_HEIGHT);
        assert!(super::WINDOW_WIDTH >= 480.0);
        assert!(super::WINDOW_HEIGHT >= 720.0);
    }

    #[test]
    fn apply_app_text_styles_raises_body_above_egui_default() {
        let ctx = eframe::egui::Context::default();
        super::apply_app_text_styles(&ctx);
        let body = ctx
            .global_style()
            .text_styles
            .get(&eframe::egui::TextStyle::Body)
            .map(|f| f.size);
        let button = ctx
            .global_style()
            .text_styles
            .get(&eframe::egui::TextStyle::Button)
            .map(|f| f.size);
        let heading = ctx
            .global_style()
            .text_styles
            .get(&eframe::egui::TextStyle::Heading)
            .map(|f| f.size);
        assert_eq!(body, Some(14.0));
        assert_eq!(button, Some(14.0));
        assert_eq!(heading, Some(19.0));
    }

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

        let home = crate::branding::MAKEMKV_HOME_URL;
        assert!(
            html.contains(&format!("href=\"{home}\"")),
            "Credits.html missing MakeMKV home link ({home})"
        );
        assert!(
            html.contains(mkv_text),
            "Credits.html missing backend ack text: {mkv_text}"
        );
        assert!(
            html.contains("<b>MakeMKV</b>"),
            "Credits.html missing MakeMKV brand markup"
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
        use crate::command::Backend;
        use crate::i18n::Language;

        assert!(validate_tool_path("", Backend::SdfTool, Language::English).is_err());
        assert!(validate_tool_path("   ", Backend::SdfTool, Language::English).is_err());

        let temp_dir = std::env::temp_dir();

        let sdftool_file = temp_dir.join("test_sdftool_temp");
        std::fs::write(&sdftool_file, b"").unwrap();
        assert!(validate_tool_path(
            &sdftool_file.to_string_lossy(),
            Backend::SdfTool,
            Language::English
        )
        .is_ok());
        assert!(validate_tool_path(
            &sdftool_file.to_string_lossy(),
            Backend::MakeMkvCon,
            Language::English
        )
        .is_err());
        let _ = std::fs::remove_file(&sdftool_file);

        let makemkv_file = temp_dir.join("test_makemkvcon_temp");
        std::fs::write(&makemkv_file, b"").unwrap();
        assert!(validate_tool_path(
            &makemkv_file.to_string_lossy(),
            Backend::MakeMkvCon,
            Language::English
        )
        .is_ok());
        assert!(validate_tool_path(
            &makemkv_file.to_string_lossy(),
            Backend::SdfTool,
            Language::English
        )
        .is_err());
        let _ = std::fs::remove_file(&makemkv_file);

        let non_existent = temp_dir.join("does-not-exist-sdftool");
        assert!(validate_tool_path(
            &non_existent.to_string_lossy(),
            Backend::SdfTool,
            Language::English
        )
        .is_err());
    }

    #[test]
    fn test_validate_sdf_path() {
        use super::validation::validate_sdf_path;
        use crate::i18n::Language;

        assert!(validate_sdf_path("", Language::English).is_ok());
        assert!(validate_sdf_path("  ", Language::English).is_ok());

        let temp_dir = std::env::temp_dir();

        let bin_file = temp_dir.join("test_sdf_temp.bin");
        std::fs::write(&bin_file, b"").unwrap();
        assert!(validate_sdf_path(&bin_file.to_string_lossy(), Language::English).is_ok());
        let _ = std::fs::remove_file(&bin_file);

        let txt_file = temp_dir.join("test_sdf_temp.txt");
        std::fs::write(&txt_file, b"").unwrap();
        assert!(validate_sdf_path(&txt_file.to_string_lossy(), Language::English).is_err());
        let _ = std::fs::remove_file(&txt_file);

        let non_existent = temp_dir.join("does-not-exist.bin");
        assert!(validate_sdf_path(&non_existent.to_string_lossy(), Language::English).is_err());
    }

    #[test]
    fn test_app_state() {
        use super::state::AppState;
        use crate::drive::Drive;

        let mut state = AppState::with_persisted(None);
        assert_eq!(state.runtime.status_message, "Ready");
        assert_eq!(state.runtime.progress, 0.0);
        assert!(!state.runtime.busy);
        assert!(state.runtime.log_text.is_empty());

        state.log("Hello");
        assert_eq!(state.runtime.log_text, "Hello");
        state.log("World");
        assert_eq!(state.runtime.log_text, "Hello\nWorld");

        state.set_status("Working...", 50.0);
        assert_eq!(state.runtime.status_message, "Working...");
        assert_eq!(state.runtime.progress, 50.0);

        state.set_status("Overworking...", 120.0);
        assert_eq!(state.runtime.progress, 100.0);
        state.set_status("Underworking...", -10.0);
        assert_eq!(state.runtime.progress, 0.0);

        assert!(state.selected_drive().is_none());
        let mock_drive = Drive {
            device: "/dev/mock_device".to_string(),
            vendor: "MockVendor".to_string(),
            product: "MockProduct".to_string(),
            revision: "1.00".to_string(),
            ..Default::default()
        };
        state.drive.drives.push(mock_drive.clone());
        state.drive.selected_drive = Some(0);

        let selected = state.selected_drive().unwrap();
        assert_eq!(selected.device, "/dev/mock_device");
        assert_eq!(selected.vendor, "MockVendor");
        assert_eq!(selected.product, "MockProduct");
        assert_eq!(selected.revision, "1.00");

        state.drive.selected_drive = Some(1);
        assert!(state.selected_drive().is_none());
    }
}
