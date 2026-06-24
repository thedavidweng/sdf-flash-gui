// Application state — owns all data the GUI tracks.

use crate::command::Backend;
use crate::drive::{self, Drive};
use crate::flash;
use crate::i18n::{self, Language};
use crate::manifest;

use super::OperationMode;

pub struct AppState {
    pub show_settings: bool,
    pub show_about: bool,

    pub backend: Backend,
    pub tool_path: String,
    pub sdf_path: String,
    pub auto_detected: bool,

    pub drives: Vec<Drive>,
    pub selected_drive: Option<usize>,
    pub last_probed_drive: Option<usize>,

    pub drive_mt1959: bool,
    pub drive_encrypted_firmware: bool,
    pub drive_probed: bool,

    pub operation_mode: OperationMode,
    pub include_boot_loader: bool,
    pub encrypted_write: bool,

    pub firmware_path: String,
    pub firmware_candidates: Vec<String>,
    pub firmware_picker_items: Vec<(String, String)>,
    pub manifest_path: String,
    pub manifest: Option<manifest::FirmwareManifest>,
    pub firmware_data: Option<Vec<u8>>,
    pub selected_image_id: Option<String>,
    pub confirmation: String,
    pub flash_report: Option<flash::FlashReport>,

    pub recovery_token: String,
    pub wrong_firmware_path: String,

    pub status_message: String,
    pub progress: f32,
    pub progress_indeterminate: bool,
    pub busy: bool,
    pub probing: bool,
    pub pending_recover_browse: bool,
    pub log_text: String,
    pub show_exit_confirmation: bool,
    pub language: Language,
    pub resolved_lang: Language,
}

impl AppState {
    pub fn new() -> Self {
        let (backend, path, auto) = match drive::find_backend() {
            Some((b, p)) => (b, p, true),
            None => (Backend::SdfTool, String::new(), false),
        };

        Self {
            show_settings: false,
            show_about: false,
            backend,
            tool_path: path,
            sdf_path: find_sdf_bin(),
            auto_detected: auto,
            drives: Vec::new(),
            selected_drive: None,
            last_probed_drive: None,
            drive_mt1959: false,
            drive_encrypted_firmware: false,
            drive_probed: false,
            operation_mode: OperationMode::Write,
            include_boot_loader: false,
            encrypted_write: false,
            firmware_path: String::new(),
            firmware_candidates: Vec::new(),
            firmware_picker_items: Vec::new(),
            manifest_path: String::new(),
            manifest: None,
            firmware_data: None,
            selected_image_id: None,
            confirmation: String::new(),
            flash_report: None,
            recovery_token: String::new(),
            wrong_firmware_path: String::new(),
            status_message: "Ready".into(),
            progress: 0.0,
            progress_indeterminate: false,
            busy: false,
            probing: false,
            pending_recover_browse: false,
            log_text: String::new(),
            show_exit_confirmation: false,
            language: Language::Auto,
            resolved_lang: i18n::detect_system_language(),
        }
    }

    pub fn log(&mut self, msg: &str) {
        if !self.log_text.is_empty() {
            self.log_text.push('\n');
        }
        self.log_text.push_str(msg);
    }

    pub fn selected_drive(&self) -> Option<&Drive> {
        self.selected_drive.and_then(|i| self.drives.get(i))
    }

    pub fn set_status(&mut self, msg: impl Into<String>, progress: f32) {
        self.status_message = msg.into();
        self.progress = progress.clamp(0.0, 100.0);
    }

    pub fn begin_operation(&mut self, status: &str) {
        self.busy = true;
        self.progress_indeterminate = true;
        self.progress = 0.0;
        self.set_status(status, 0.0);
    }
}

pub fn find_sdf_bin() -> String {
    let candidates = ["./sdf.bin", "../sdf.bin", "/usr/share/sdftool/sdf.bin"];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let paths = [
            format!("{home}/.MakeMKV/sdf.bin"),
            "/Library/MakeMKV/sdf.bin".to_string(),
            "/opt/homebrew/share/sdftool/sdf.bin".to_string(),
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                return p.clone();
            }
        }
    }

    String::new()
}
