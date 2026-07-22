//! Drive list refresh (apply lives on AppState).
use crate::drive;
use crate::gui::state::AppState;

pub fn refresh_drives(state: &mut AppState) {
    let drives = drive::enumerate_drives();
    state.apply_drive_list(drives);
}
