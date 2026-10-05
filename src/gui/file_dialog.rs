use std::path::{Path, PathBuf};

/// Trait for file/folder picker operations.
pub trait FileDialog {
    fn pick_folder(&self) -> Option<PathBuf>;
    fn pick_file_with_title(
        &self,
        title: &str,
        filter_name: &str,
        extensions: &[&str],
        initial_dir: Option<&Path>,
    ) -> Option<PathBuf>;
}

/// Production implementation using rfd (native dialogs).
pub struct NativeDialog;

/// Browsers expose no synchronous file picker, so the web build never returns a path.
#[cfg(target_arch = "wasm32")]
impl FileDialog for NativeDialog {
    fn pick_folder(&self) -> Option<PathBuf> {
        None
    }

    fn pick_file_with_title(
        &self,
        _title: &str,
        _filter_name: &str,
        _extensions: &[&str],
        _initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl FileDialog for NativeDialog {
    fn pick_folder(&self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    fn pick_file_with_title(
        &self,
        title: &str,
        filter_name: &str,
        extensions: &[&str],
        initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter_name, extensions);
        if let Some(dir) = initial_dir {
            dialog = dialog.set_directory(dir);
        }
        dialog.pick_file()
    }
}
