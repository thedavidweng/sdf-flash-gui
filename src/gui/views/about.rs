use crate::branding::MAKEMKV_HOME_URL;
use crate::gui::state::AppState;
use crate::i18n::{t, t_with_args, L10nKey};

use eframe::egui;

use super::super::{APP_VERSION, GAP_SMALL};

#[allow(deprecated)]
pub fn show_about_window(ctx: &egui::Context, state: &mut AppState) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("about_viewport"),
        egui::ViewportBuilder::default()
            .with_title(t(L10nKey::TooltipAbout, state.chrome.resolved_lang))
            .with_inner_size([320.0, 180.0])
            .with_min_inner_size([320.0, 180.0])
            .with_resizable(true),
        |ctx, _class| {
            if super::viewport_close_requested(ctx) {
                state.chrome.show_about = false;
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(t(L10nKey::LabelAppName, state.chrome.resolved_lang));
                    ui.add_space(GAP_SMALL);
                    ui.label(t(L10nKey::AboutDescription, state.chrome.resolved_lang));
                    ui.label(t(L10nKey::AboutBuiltWith, state.chrome.resolved_lang));
                });
                ui.separator();
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.strong(t(
                            L10nKey::AboutAcknowledgementsTitle,
                            state.chrome.resolved_lang,
                        ));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.add(
                                egui::Hyperlink::from_label_and_url(
                                    egui::RichText::new("MakeMKV").small(),
                                    MAKEMKV_HOME_URL,
                                )
                                .open_in_new_tab(true),
                            );
                            ui.small(t(L10nKey::AboutBackendAckText, state.chrome.resolved_lang));
                        });
                        ui.small(format!(
                            "MartyMcNuts {}",
                            t(L10nKey::AboutCreatorAckText, state.chrome.resolved_lang)
                        ));
                        ui.add_space(GAP_SMALL);
                        ui.hyperlink_to(
                            t(L10nKey::LabelGithubRepo, state.chrome.resolved_lang),
                            "https://github.com/thedavidweng/sdf-flash-gui",
                        );
                        ui.weak(t_with_args(
                            L10nKey::LabelVersion,
                            state.chrome.resolved_lang,
                            &[("version", APP_VERSION)],
                        ));
                    });
                });
            });
        },
    );
}
