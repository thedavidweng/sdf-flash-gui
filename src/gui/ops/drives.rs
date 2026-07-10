//! Drive list refresh (apply lives on AppState).
use crate::drive;
use crate::gui::state::AppState;
use crate::i18n::{t_with_args, L10nKey};

pub fn refresh_drives(state: &mut AppState) {
    let lang = state.chrome.resolved_lang;
    let drives = drive::enumerate_drives();
    let count = drives.len();
    state.apply_drive_list(drives);
    state.log(&t_with_args(
        L10nKey::StatusDrivesFound,
        lang,
        &[("count", &count.to_string())],
    ));
}
