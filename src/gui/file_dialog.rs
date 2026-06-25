// Abstraction over native file dialogs for testability.

use std::path::PathBuf;

/// Trait for file/folder picker operations.
pub trait FileDialog {
    fn pick_folder(&self) -> Option<PathBuf>;
    fn pick_file(&self) -> Option<PathBuf>;
    fn pick_file_with_title(
        &self,
        title: &str,
        filter_name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf>;
}

/// Production implementation using rfd (native dialogs).
pub struct NativeDialog;

impl FileDialog for NativeDialog {
    fn pick_folder(&self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    fn pick_file(&self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_file()
    }

    fn pick_file_with_title(
        &self,
        title: &str,
        filter_name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter_name, extensions)
            .pick_file()
    }
}
